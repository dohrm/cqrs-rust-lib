use crate::es::storage::EventStoreStorage;
use crate::{Aggregate, AggregateError, CqrsContext, EventEnvelope, EventStore, Snapshot};
use std::collections::HashMap;
use tracing::{debug, error, info, instrument};

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
    ) -> Result<Vec<EventEnvelope<A>>, AggregateError> {
        debug!("Loading events from version");
        match self
            .persist
            .fetch_events_from_version(aggregate_id, version)
            .await
        {
            Ok(events) => {
                info!(event_count = events.len(), "Events loaded successfully");
                Ok(events)
            }
            Err(e) => {
                error!(error = %e, "Failed to load events from version");
                Err(e)
            }
        }
    }

    async fn load_events(
        &self,
        aggregate_id: &str,
    ) -> Result<Vec<EventEnvelope<A>>, AggregateError> {
        debug!("Loading all events for aggregate");
        match self.persist.fetch_all_events(aggregate_id).await {
            Ok(events) => {
                info!(event_count = events.len(), "All events loaded successfully");
                Ok(events)
            }
            Err(e) => {
                error!(error = %e, "Failed to load all events");
                Err(e)
            }
        }
    }

    async fn commit(
        &self,
        events: Vec<A::Event>,
        aggregate: &A,
        metadata: HashMap<String, String>,
        version: usize,
        context: &CqrsContext,
    ) -> Result<Vec<EventEnvelope<A>>, AggregateError> {
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

        let latest_event = match self.persist.fetch_latest_event(aggregate, &session).await {
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
            return Err(AggregateError::Conflict);
        }

        debug!("Creating event envelopes");
        let events = events
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

        debug!(event_count = events.len(), "Saving events");
        session = match self.persist.save_events(events.clone(), session).await {
            Ok(session) => {
                debug!("Events saved successfully");
                session
            }
            Err(e) => {
                error!(error = %e, "Failed to save events");
                return Err(e);
            }
        };

        let next_latest_version = version + events.len();
        debug!(next_version = %next_latest_version, "Saving snapshot");
        session = match self
            .persist
            .save_snapshot(aggregate, next_latest_version, session)
            .await
        {
            Ok(session) => {
                debug!("Snapshot saved successfully");
                session
            }
            Err(e) => {
                error!(error = %e, "Failed to save snapshot");
                return Err(e);
            }
        };

        debug!("Closing session");
        if let Err(e) = self.persist.close_session(session).await {
            error!(error = %e, "Failed to close session");
            return Err(e);
        }

        info!(event_count = events.len(), "Commit completed successfully");
        Ok(events)
    }
}
