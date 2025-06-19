use crate::errors::AggregateError;
use crate::snapshot::Snapshot;
use crate::{Aggregate, CqrsContext, EventEnvelope};
use http::StatusCode;
use std::collections::HashMap;
use std::fmt::Debug;

#[async_trait::async_trait]
pub trait EventStore<A>: Debug + Clone + Sync + Send
where
    A: Aggregate,
{
    async fn load_snapshot(
        &self,
        aggregate_id: &str,
    ) -> Result<Option<Snapshot<A>>, AggregateError>;

    async fn load_events_from_version(
        &self,
        aggregate_id: &str,
        version: usize,
    ) -> Result<Vec<EventEnvelope<A>>, AggregateError>;

    async fn load_events(
        &self,
        aggregate_id: &str,
    ) -> Result<Vec<EventEnvelope<A>>, AggregateError>;

    async fn initialize_aggregate(&self, aggregate_id: &str) -> Result<(A, usize), AggregateError> {
        let maybe_snapshot = self.load_snapshot(aggregate_id).await?;
        if let Some(_snapshot) = maybe_snapshot {
            return Err(AggregateError::UserError(
                A::error(StatusCode::CONFLICT, "Aggregate already exists").into(),
            ));
        }
        Ok((A::default().with_aggregate_id(aggregate_id.to_string()), 0))
    }

    async fn load_aggregate(&self, aggregate_id: &str) -> Result<(A, usize), AggregateError> {
        let maybe_snapshot = self.load_snapshot(aggregate_id).await?;
        if maybe_snapshot.is_none() {
            return Err(AggregateError::UserError(
                A::error(StatusCode::NOT_FOUND, "Aggregate not found").into(),
            ));
        }
        let snapshot = maybe_snapshot.unwrap();
        let mut agg = snapshot.state;
        let version = snapshot.version;

        let mut latest_version = version;
        let latest_events = self.load_events_from_version(aggregate_id, version).await?;
        for event in latest_events {
            agg.apply(event.payload)
                .map_err(|e| AggregateError::UserError(e.into()))?;
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
    ) -> Result<Vec<EventEnvelope<A>>, AggregateError>;
}
