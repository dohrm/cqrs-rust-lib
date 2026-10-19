use crate::es::storage::{EventStoreStorage, EventStream};
use crate::{
    Aggregate, CqrsContext, CqrsError, EventEnvelope, EventStore, MaybeSend, MaybeSync, Snapshot,
};
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;
use tracing::{debug, error, info};

#[derive(Debug, Clone)]
pub struct EventStoreImpl<A, P>
where
    A: Aggregate + 'static,
    P: EventStoreStorage<A> + MaybeSend + MaybeSync + Clone + Debug + 'static,
{
    _phantom: std::marker::PhantomData<(A, P)>,
    persist: P,
}

impl<A, P> EventStoreImpl<A, P>
where
    A: Aggregate + 'static,
    P: EventStoreStorage<A> + MaybeSend + MaybeSync + Clone + Debug + 'static,
{
    #[must_use]
    pub fn new(persist: P) -> Arc<Self> {
        Arc::new(Self {
            _phantom: Default::default(),
            persist,
        })
    }

    async fn execute_within_session(
        &self,
        session: &mut P::Session,
        events: Vec<A::Event>,
        aggregate: &A,
        metadata: HashMap<String, String>,
        version: usize,
        context: &CqrsContext,
    ) -> Result<Vec<EventEnvelope<A>>, CqrsError> {
        let latest_event = match self.persist.fetch_latest_event(aggregate, session).await {
            Ok(event) => {
                debug!(has_event = event.is_some(), "Fetched latest event");
                event
            }
            Err(e) => {
                error!(error = %e, "Failed to fetch latest event");
                return Err(e);
            }
        };

        let latest_version = latest_event.map(|e| e.version).unwrap_or(0);
        debug!(latest_version = %latest_version, expected_version = %version, "Checking version");

        if version != latest_version {
            error!(latest_version = %latest_version, expected_version = %version, "Version conflict detected");
            return Err(CqrsError::concurrency_error());
        }

        debug!("Creating event envelopes");
        let envelopes = events
            .iter()
            .enumerate()
            .map(|(i, e)| {
                let event_id = context.next_uuid();
                let event_version = version + i + 1;
                debug!(event_id = %event_id, event_version = %event_version, "Creating event envelope");
                EventEnvelope {
                    event_id,
                    aggregate_id: aggregate.aggregate_id(),
                    version: event_version,
                    payload: e.clone(),
                    metadata: metadata.clone(),
                    at: context.now(),
                }
            })
            .collect::<Vec<_>>();

        debug!(event_count = envelopes.len(), "Saving events");
        if let Err(e) = self.persist.save_events(envelopes.clone(), session).await {
            error!(error = %e, "Failed to save events");
            return Err(e);
        }
        debug!("Events saved successfully");

        let next_latest_version = version + envelopes.len();
        debug!(next_version = %next_latest_version, "Saving snapshot");
        if let Err(e) = self
            .persist
            .save_snapshot(aggregate, next_latest_version, session)
            .await
        {
            error!(error = %e, "Failed to save snapshot");
            return Err(e);
        }
        debug!("Snapshot saved successfully");

        Ok(envelopes)
    }
}

cqrs_async_trait! {
impl<A, P> EventStore<A> for EventStoreImpl<A, P>
where
    A: Aggregate + 'static,
    P: EventStoreStorage<A> + MaybeSend + MaybeSync + Clone + Debug + 'static,
{
    async fn load_snapshot(&self, aggregate_id: &str) -> Result<Option<Snapshot<A>>, CqrsError> {
        debug!("Loading snapshot for aggregate");
        match self.persist.fetch_snapshot(aggregate_id).await {
            Ok(Some(snapshot)) => {
                info!(version = %snapshot.version, "Snapshot loaded successfully");
                Ok(Some(snapshot))
            }
            Ok(None) => {
                debug!("No snapshot found for aggregate");
                Ok(None)
            }
            Err(e) => {
                error!(error = %e, "Failed to load snapshot");
                Err(e)
            }
        }
    }

    async fn load_events_from_version(
        &self,
        aggregate_id: &str,
        version: usize,
    ) -> Result<EventStream<A>, CqrsError> {
        debug!("Loading events from version");
        self.persist
            .fetch_events_from_version(aggregate_id, version)
            .await
    }

    async fn load_events(&self, aggregate_id: &str) -> Result<EventStream<A>, CqrsError> {
        debug!("Loading all events for aggregate");
        self.persist.fetch_all_events(aggregate_id).await
    }

    async fn load_events_paged(
        &self,
        aggregate_id: &str,
        page: usize,
        page_size: usize,
    ) -> Result<(Vec<EventEnvelope<A>>, i64), CqrsError> {
        debug!("Loading paged events for aggregate");
        self.persist
            .fetch_events_paged(aggregate_id, page, page_size)
            .await
    }

    async fn commit(
        &self,
        events: Vec<A::Event>,
        aggregate: &A,
        metadata: HashMap<String, String>,
        version: usize,
        context: &CqrsContext,
    ) -> Result<Vec<EventEnvelope<A>>, CqrsError> {
        debug!("Starting commit process");

        // Nothing new on an aggregate that has events: no session, no snapshot rewrite at
        // an unchanged version. At version 0 the snapshot is the aggregate's only trace,
        // so it is still written — or a create would return an id that does not exist.
        if events.is_empty() && version > 0 {
            debug!("No events to commit");
            return Ok(vec![]);
        }

        let mut session = match self.persist.start_session().await {
            Ok(session) => {
                debug!("Session started successfully");
                session
            }
            Err(e) => {
                error!(error = %e, "Failed to start session");
                return Err(e);
            }
        };

        let result = self
            .execute_within_session(&mut session, events, aggregate, metadata, version, context)
            .await;

        match result {
            Ok(events) => {
                debug!("Closing session");
                if let Err(e) = self.persist.close_session(session).await {
                    error!(error = %e, "Failed to close session");
                    return Err(e);
                }
                info!(event_count = events.len(), "Commit completed successfully");
                Ok(events)
            }
            Err(e) => {
                error!(error = %e, "Error during commit, aborting session");
                let _ = self.persist.abort_session(session).await;
                Err(e)
            }
        }
    }
}
}

#[cfg(test)]
mod tests {
    use super::EventStoreImpl;
    use crate::es::inmemory::InMemoryPersist;
    use crate::testing::{TestAggregate, TestEvent};
    use crate::{Aggregate, CqrsContext, EventStore};

    async fn created() -> (
        std::sync::Arc<EventStoreImpl<TestAggregate, InMemoryPersist<TestAggregate>>>,
        TestAggregate,
    ) {
        let store = EventStoreImpl::new(InMemoryPersist::<TestAggregate>::new());
        let (mut aggregate, version) = store.initialize_aggregate("a1").await.unwrap();
        let created = TestEvent::Created { name: "a".into() };
        aggregate.apply(created.clone()).unwrap();
        let context = CqrsContext::default();
        store
            .commit(
                vec![created],
                &aggregate,
                Default::default(),
                version,
                &context,
            )
            .await
            .unwrap();
        (store, aggregate)
    }

    #[tokio::test]
    async fn an_empty_commit_leaves_the_snapshot_untouched() {
        let (store, aggregate) = created().await;
        let mut drifted = aggregate.clone();
        drifted.apply(TestEvent::Incremented).unwrap();

        let committed = store
            .commit(
                vec![],
                &drifted,
                Default::default(),
                1,
                &CqrsContext::default(),
            )
            .await
            .unwrap();

        assert!(committed.is_empty());
        let snapshot = store.load_snapshot("a1").await.unwrap().unwrap();
        assert_eq!((snapshot.state, snapshot.version), (aggregate, 1));
    }

    #[tokio::test]
    async fn an_empty_commit_skips_the_version_check() {
        let (store, aggregate) = created().await;
        let stale = 7;

        let committed = store
            .commit(
                vec![],
                &aggregate,
                Default::default(),
                stale,
                &CqrsContext::default(),
            )
            .await;

        assert!(committed.unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_create_without_events_still_persists_the_aggregate() {
        let store = EventStoreImpl::new(InMemoryPersist::<TestAggregate>::new());
        let (aggregate, version) = store.initialize_aggregate("a1").await.unwrap();

        store
            .commit(
                vec![],
                &aggregate,
                Default::default(),
                version,
                &CqrsContext::default(),
            )
            .await
            .unwrap();

        let (_, version) = store.load_aggregate("a1").await.unwrap();
        assert_eq!(version, 0);
        assert!(store.initialize_aggregate("a1").await.is_err());
    }
}
