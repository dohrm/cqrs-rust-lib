use crate::{Aggregate, AggregateError, EventEnvelope, Snapshot};
use std::fmt::Debug;

#[async_trait::async_trait]
pub trait Persist<A>: Clone + Debug + Send + Sync
where
    A: Aggregate,
{
    type Session: Send + Sync;

    async fn start_session(&self) -> Result<Self::Session, AggregateError>;
    async fn close_session(&self, session: Self::Session) -> Result<(), AggregateError>;
    async fn fetch_snapshot(
        &self,
        aggregate_id: &str,
    ) -> Result<Option<Snapshot<A>>, AggregateError>;

    async fn fetch_events_from_version(
        &self,
        aggregate_id: &str,
        version: usize,
    ) -> Result<Vec<EventEnvelope<A>>, AggregateError>;

    async fn fetch_all_events(
        &self,
        aggregate_id: &str,
    ) -> Result<Vec<EventEnvelope<A>>, AggregateError>;

    async fn fetch_latest_event(
        &self,
        aggregate: &A,
        session: &Self::Session,
    ) -> Result<Option<EventEnvelope<A>>, AggregateError>;

    async fn save_events(
        &self,
        events: Vec<EventEnvelope<A>>,
        session: Self::Session,
    ) -> Result<Self::Session, AggregateError>;

    async fn save_snapshot(
        &self,
        aggregate: &A,
        version: usize,
        session: Self::Session,
    ) -> Result<Self::Session, AggregateError>;
}
