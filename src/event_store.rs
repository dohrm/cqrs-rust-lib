use crate::errors::CqrsError;
use crate::es::storage::EventStream;
use crate::snapshot::Snapshot;
use crate::{Aggregate, CqrsContext, EventEnvelope};
use futures::StreamExt;
use std::collections::HashMap;
use std::sync::Arc;

pub type DynEventStore<A> = Arc<dyn EventStore<A> + Send + Sync + 'static>;

#[async_trait::async_trait]
pub trait EventStore<A>
where
    A: Aggregate + 'static,
{
    async fn load_snapshot(&self, aggregate_id: &str) -> Result<Option<Snapshot<A>>, CqrsError>;

    async fn load_events_from_version(
        &self,
        aggregate_id: &str,
        version: usize,
    ) -> Result<EventStream<A>, CqrsError>;

    async fn load_events(&self, aggregate_id: &str) -> Result<EventStream<A>, CqrsError>;

    async fn load_events_paged(
        &self,
        aggregate_id: &str,
        page: usize,
        page_size: usize,
    ) -> Result<(Vec<EventEnvelope<A>>, i64), CqrsError>;

    async fn initialize_aggregate(&self, aggregate_id: &str) -> Result<(A, usize), CqrsError> {
        let maybe_snapshot = self.load_snapshot(aggregate_id).await?;
        if maybe_snapshot.is_some() {
            return Err(CqrsError::aggregate_already_exists(aggregate_id));
        }
        Ok((A::default().with_aggregate_id(aggregate_id.to_string()), 0))
    }

    async fn load_aggregate(&self, aggregate_id: &str) -> Result<(A, usize), CqrsError> {
        let maybe_snapshot = self.load_snapshot(aggregate_id).await?;
        if maybe_snapshot.is_none() {
            return Err(CqrsError::aggregate_not_found(aggregate_id));
        }
        let snapshot = maybe_snapshot.unwrap();
        let mut agg = snapshot.state;
        let version = snapshot.version;

        let mut latest_version = version;
        let mut event_stream = self.load_events_from_version(aggregate_id, version).await?;
        while let Some(event) = event_stream.next().await {
            let event = event?;
            agg.apply(event.payload).map_err(CqrsError::user_error)?;
            latest_version = event.version;
        }
        Ok((agg, latest_version))
    }

    async fn commit(
        &self,
        events: Vec<A::Event>,
        aggregate: &A,
        metadata: HashMap<String, String>,
        version: usize,
        context: &CqrsContext,
    ) -> Result<Vec<EventEnvelope<A>>, CqrsError>;
}
