use crate::es::storage::EventStoreStorage;
use crate::{Aggregate, AggregateError, CqrsContext, EventEnvelope, EventStore, Snapshot};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct EventStoreImpl<A, P>
where
    A: Aggregate,
    P: EventStoreStorage<A>,
{
    _phantom: std::marker::PhantomData<(A, P)>,
    persist: P,
}

impl<A, P> EventStoreImpl<A, P>
where
    A: Aggregate,
    P: EventStoreStorage<A>,
{
    #[must_use]
    pub fn new(persist: P) -> Self {
        Self {
            _phantom: Default::default(),
            persist,
        }
    }
}

#[async_trait::async_trait]
impl<A, P> EventStore<A> for EventStoreImpl<A, P>
where
    A: Aggregate,
    P: EventStoreStorage<A>,
{
    async fn load_snapshot(
        &self,
        aggregate_id: &str,
    ) -> Result<Option<Snapshot<A>>, AggregateError> {
        self.persist.fetch_snapshot(aggregate_id).await
    }

    async fn load_events_from_version(
        &self,
        aggregate_id: &str,
        version: usize,
    ) -> Result<Vec<EventEnvelope<A>>, AggregateError> {
        self.persist
            .fetch_events_from_version(aggregate_id, version)
            .await
    }

    async fn load_events(
        &self,
        aggregate_id: &str,
    ) -> Result<Vec<EventEnvelope<A>>, AggregateError> {
        self.persist.fetch_all_events(aggregate_id).await
    }

    async fn commit(
        &self,
        events: Vec<A::Event>,
        aggregate: &A,
        metadata: HashMap<String, String>,
        version: usize,
        context: &CqrsContext,
    ) -> Result<Vec<EventEnvelope<A>>, AggregateError> {
        let mut session = self.persist.start_session().await?;
        let latest_event = self.persist.fetch_latest_event(aggregate, &session).await?;

        let latest_version = latest_event.map(|e| e.version).unwrap_or(0);
        if version != latest_version {
            return Err(AggregateError::Conflict);
        }
        let events = events
            .iter()
            .enumerate()
            .map(|(i, e)| EventEnvelope {
                event_id: context.next_uuid(),
                aggregate_id: aggregate.aggregate_id(),
                version: version + i + 1,
                payload: e.clone(),
                metadata: metadata.clone(),
                at: context.now(),
            })
            .collect::<Vec<_>>();

        session = self.persist.save_events(events.clone(), session).await?;
        let next_latest_version = version + events.len();
        session = self
            .persist
            .save_snapshot(aggregate, next_latest_version, session)
            .await?;
        self.persist.close_session(session).await?;
        Ok(events)
    }
}
