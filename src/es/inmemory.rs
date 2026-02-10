use crate::es::storage::{EventStoreStorage, EventStream};
use crate::{Aggregate, CqrsError, EventEnvelope, Snapshot};
use futures::lock::{Mutex, OwnedMutexGuard};
use futures::stream;
use std::collections::HashMap;
use std::sync::Arc;

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
    A: Aggregate + 'static,
{
    type Session = (
        OwnedMutexGuard<HashMap<String, Snapshot<A>>>,
        OwnedMutexGuard<HashMap<String, Vec<EventEnvelope<A>>>>,
    );

    async fn start_session(&self) -> Result<Self::Session, CqrsError> {
        let journal = self.journal.clone().lock_owned().await;
        let snapshot = self.snapshot.clone().lock_owned().await;
        Ok((snapshot, journal))
    }

    async fn close_session(&self, _session: Self::Session) -> Result<(), CqrsError> {
        Ok(())
    }

    async fn fetch_snapshot(&self, aggregate_id: &str) -> Result<Option<Snapshot<A>>, CqrsError> {
        let snapshot = self.snapshot.lock().await;
        Ok(snapshot.get(aggregate_id).cloned())
    }

    async fn fetch_events_from_version(
        &self,
        aggregate_id: &str,
        version: usize,
    ) -> Result<EventStream<A>, CqrsError> {
        let journal = self.journal.lock().await;
        let items = journal.get(aggregate_id).cloned().unwrap_or_default();
        let events: Vec<EventEnvelope<A>> =
            items.into_iter().filter(|v| v.version > version).collect();
        Ok(Box::pin(stream::iter(events.into_iter().map(Ok))))
    }

    async fn fetch_all_events(&self, aggregate_id: &str) -> Result<EventStream<A>, CqrsError> {
        let journal = self.journal.lock().await;
        let items = journal.get(aggregate_id).cloned().unwrap_or_default();
        Ok(Box::pin(stream::iter(items.into_iter().map(Ok))))
    }

    async fn fetch_events_paged(
        &self,
        aggregate_id: &str,
        page: usize,
        page_size: usize,
    ) -> Result<(Vec<EventEnvelope<A>>, i64), CqrsError> {
        let journal = self.journal.lock().await;
        let items = journal.get(aggregate_id).cloned().unwrap_or_default();
        let total = items.len() as i64;
        let offset = (page.max(1) - 1) * page_size;
        let events: Vec<EventEnvelope<A>> =
            items.into_iter().skip(offset).take(page_size).collect();
        Ok((events, total))
    }

    async fn fetch_latest_event(
        &self,
        aggregate: &A,
        session: &Self::Session,
    ) -> Result<Option<EventEnvelope<A>>, CqrsError> {
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
        session: &mut Self::Session,
    ) -> Result<(), CqrsError> {
        if events.is_empty() {
            return Ok(());
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
        Ok(())
    }

    async fn save_snapshot(
        &self,
        aggregate: &A,
        version: usize,
        session: &mut Self::Session,
    ) -> Result<(), CqrsError> {
        session.0.insert(
            aggregate.aggregate_id(),
            Snapshot {
                aggregate_id: aggregate.aggregate_id(),
                state: aggregate.clone(),
                version,
            },
        );
        Ok(())
    }
}
