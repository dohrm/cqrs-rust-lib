use crate::es::storage::{EventStoreStorage, EventStream};
use crate::{Aggregate, CqrsContext, CqrsError, EventEnvelope, EventStore, Snapshot};
use std::collections::HashMap;
use std::fmt::Debug;
use std::sync::Arc;
use tracing::{debug, error, info};

#[derive(Debug, Clone)]
pub struct EventStoreImpl<A, P>
where
    A: Aggregate + 'static,
    P: EventStoreStorage<A> + Send + Sync + Clone + Debug + 'static,
{
    _phantom: std::marker::PhantomData<(A, P)>,
    persist: P,
}

impl<A, P> EventStoreImpl<A, P>
where
    A: Aggregate + 'static,
    P: EventStoreStorage<A> + Send + Sync + Clone + Debug + 'static,
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

#[async_trait::async_trait]
impl<A, P> EventStore<A> for EventStoreImpl<A, P>
where
    A: Aggregate + 'static,
    P: EventStoreStorage<A> + Send + Sync + Clone + Debug + 'static,
{
    async fn load_snapshot(
        &self,
        aggregate_id: &str,
    ) -> Result<Option<Snapshot<A>>, CqrsError> {
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
