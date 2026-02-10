use crate::{Aggregate, CqrsError, EventEnvelope, Snapshot};
use futures::stream::Stream;
use std::pin::Pin;

pub type EventStream<A> =
    Pin<Box<dyn Stream<Item = Result<EventEnvelope<A>, CqrsError>> + Send>>;

#[async_trait::async_trait]
pub trait EventStoreStorage<A>
where
    A: Aggregate + 'static,
{
    type Session: Send + Sync;

    async fn start_session(&self) -> Result<Self::Session, CqrsError>;
    async fn close_session(&self, session: Self::Session) -> Result<(), CqrsError>;
    async fn fetch_snapshot(
        &self,
        aggregate_id: &str,
    ) -> Result<Option<Snapshot<A>>, CqrsError>;

    async fn fetch_events_from_version(
        &self,
        aggregate_id: &str,
        version: usize,
    ) -> Result<EventStream<A>, CqrsError>;

    async fn fetch_all_events(&self, aggregate_id: &str) -> Result<EventStream<A>, CqrsError>;

    async fn fetch_events_paged(
        &self,
        aggregate_id: &str,
        page: usize,
        page_size: usize,
    ) -> Result<(Vec<EventEnvelope<A>>, i64), CqrsError>;

    async fn fetch_latest_event(
        &self,
        aggregate: &A,
        session: &Self::Session,
    ) -> Result<Option<EventEnvelope<A>>, CqrsError>;

    async fn save_events(
        &self,
        events: Vec<EventEnvelope<A>>,
        session: Self::Session,
    ) -> Result<Self::Session, CqrsError>;

    async fn save_snapshot(
        &self,
        aggregate: &A,
        version: usize,
        session: Self::Session,
    ) -> Result<Self::Session, CqrsError>;
}
