use crate::es::storage::EventStoreStorage;
use crate::{Aggregate, AggregateError, EventEnvelope, Snapshot};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, OwnedMutexGuard};

#[derive(Clone, Debug, Default)]
pub struct InMemoryPersist<A>
where
    A: Aggregate,
{
    _phantom: std::marker::PhantomData<A>,
    snapshot: Arc<Mutex<HashMap<String, Snapshot<A>>>>,
    journal: Arc<Mutex<HashMap<String, Vec<EventEnvelope<A>>>>>,
}

impl<A> InMemoryPersist<A>
where
    A: Aggregate,
{
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait::async_trait]
impl<A> EventStoreStorage<A> for InMemoryPersist<A>
where
    A: Aggregate,
{
    type Session = (
        OwnedMutexGuard<HashMap<String, Snapshot<A>>>,
        OwnedMutexGuard<HashMap<String, Vec<EventEnvelope<A>>>>,
    );

    async fn start_session(&self) -> Result<Self::Session, AggregateError> {
        let journal = self.journal.clone().lock_owned().await;
        let snapshot = self.snapshot.clone().lock_owned().await;
        Ok((snapshot, journal))
    }

    async fn close_session(&self, _session: Self::Session) -> Result<(), AggregateError> {
        Ok(())
    }

    async fn fetch_snapshot(
        &self,
        aggregate_id: &str,
    ) -> Result<Option<Snapshot<A>>, AggregateError> {
        let snapshot = self.snapshot.lock().await;
        Ok(snapshot.get(aggregate_id).cloned())
    }

    async fn fetch_events_from_version(
        &self,
        aggregate_id: &str,
        version: usize,
    ) -> Result<Vec<EventEnvelope<A>>, AggregateError> {
        let journal = self.journal.lock().await;
        let items = journal.get(aggregate_id).cloned().unwrap_or_default();
        Ok(items
            .iter()
            .filter(|v| v.version > version)
            .cloned()
            .collect())
    }

    async fn fetch_all_events(
        &self,
        aggregate_id: &str,
    ) -> Result<Vec<EventEnvelope<A>>, AggregateError> {
        let journal = self.journal.lock().await;
        let items = journal.get(aggregate_id).cloned().unwrap_or_default();
        Ok(items)
    }

    async fn fetch_latest_event(
        &self,
        aggregate: &A,
        session: &Self::Session,
    ) -> Result<Option<EventEnvelope<A>>, AggregateError> {
        let events = session
            .1
            .get(aggregate.aggregate_id().as_str())
            .cloned()
            .unwrap_or_default();
        Ok(events.last().cloned())
    }

    async fn save_events(
        &self,
        events: Vec<EventEnvelope<A>>,
        mut session: Self::Session,
    ) -> Result<Self::Session, AggregateError> {
        if events.is_empty() {
            return Ok(session);
        }
        let aggregate_id = events.first().unwrap().aggregate_id.clone();
        session
            .1
            .entry(aggregate_id)
            .and_modify(|val| {
                for e in events.iter() {
                    val.push(e.clone());
                }
            })
            .or_insert(events);
        Ok(session)
    }

    async fn save_snapshot(
        &self,
        aggregate: &A,
        version: usize,
        mut session: Self::Session,
    ) -> Result<Self::Session, AggregateError> {
        session.0.insert(
            aggregate.aggregate_id(),
            Snapshot {
                aggregate_id: aggregate.aggregate_id(),
                state: aggregate.clone(),
                version,
            },
        );
        Ok(session)
    }
}
