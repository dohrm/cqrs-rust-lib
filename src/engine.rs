use crate::context::CqrsContext;
use crate::denormalizer::Dispatcher;
use crate::errors::AggregateError;
use crate::event::Event;
use crate::{Aggregate, DynEventStore, EventEnvelope};
use std::collections::HashMap;
use tracing::{debug, error, info};

/// The `CqrsCommandEngine` struct is a Command Query Responsibility Segregation (CQRS) engine
/// designed to handle commands and communication with an underlying event store and various dispatchers.
/// It acts as the main entry point for command processing and encapsulates the behavior specific to an aggregate.
///
/// # Type Parameters
/// - `A`: The type of the aggregate managed by this CQRS engine.
///   The aggregate represents the domain behavior and state transitions.
/// - `ES`: The type of the event store used to views events related to the aggregate.
///
/// # Bounds
/// - `A`: Must implement the `Aggregate` trait. This ensures the aggregate provides necessary
///   functionality such as validating commands or applying events to mutate its state.
/// - `ES`: Must implement the `EventStore<A>` trait. This ensures the event store works
///   with the specified aggregate type for persisting and retrieving events.
///
/// # Fields
/// - `store: ES`
///   The event store instance used to views and retrieve events associated with the aggregate.
///   It allows the CQRS engine to save and load the aggregate's event stream to/from persistent storage.
///
/// - `dispatchers: Vec<Box<dyn Dispatcher<A>>>`
///   A collection of dispatchers used by the CQRS engine to handle various external interactions such as
///   messaging or integration with other systems. Dispatchers are responsible for forwarding
///   or broadcasting events and can implement custom logic based on the use case.
///
/// - `services: A::Services`
///   A collection of domain-specific services required by the aggregate to perform its business operations.
///   These services are defined within the aggregate's associated types to provide dependencies
///   such as external APIs, configuration, or infrastructure required for executing commands.
///
/// # Usage
/// Typically, the `CqrsCommandEngine` is instantiated with a concrete implementation of an event store,
/// one or more command dispatchers, and the services needed by the aggregate. Once initialized,
/// it can be used to dispatch commands and manage the lifecycle of aggregate instances.
///
/// This struct facilitates the CQRS pattern by separating the responsibility of command handling
/// from querying, while keeping event storage and dispatching modular and configurable.
pub struct CqrsCommandEngine<A>
where
    A: Aggregate + 'static,
{
    store: DynEventStore<A>,
    dispatchers: Vec<Box<dyn Dispatcher<A>>>,
    services: A::Services,
    error_handler: Box<dyn Fn(&AggregateError) + Send + Sync>,
}

impl<A> CqrsCommandEngine<A>
where
    A: Aggregate + 'static,
{
    #[must_use]
    pub fn new(
        store: DynEventStore<A>,
        dispatchers: Vec<Box<dyn Dispatcher<A>>>,
        services: A::Services,
        error_handler: Box<dyn Fn(&AggregateError) + Send + Sync>,
    ) -> Self {
        Self {
            store,
            dispatchers,
            services,
            error_handler,
        }
    }

    pub fn append_dispatcher(&mut self, dispatcher: Box<dyn Dispatcher<A>>) {
        self.dispatchers.push(dispatcher);
    }

    pub async fn execute_create(
        &self,
        command: A::CreateCommand,
        context: &CqrsContext,
    ) -> Result<String, AggregateError> {
        debug!("Executing create command");
        let result = self
            .execute_create_with_metadata(command, HashMap::new(), context)
            .await;
        match &result {
            Ok(id) => info!(aggregate_id = %id, "Aggregate created successfully"),
            Err(e) => error!(error = %e, "Failed to create aggregate"),
        }
        result
    }

    pub async fn execute_update(
        &self,
        aggregate_id: &str,
        command: A::UpdateCommand,
        context: &CqrsContext,
    ) -> Result<(), AggregateError> {
        debug!("Executing update command");
        let result = self
            .execute_update_with_metadata(aggregate_id, command, HashMap::new(), context)
            .await;
        match &result {
            Ok(_) => info!("Aggregate updated successfully"),
            Err(e) => error!(error = %e, "Failed to update aggregate"),
        }
        result
    }

    pub async fn execute_create_with_metadata(
        &self,
        command: A::CreateCommand,
        metadata: HashMap<String, String>,
        context: &CqrsContext,
    ) -> Result<String, AggregateError> {
        debug!("Executing create command with metadata");
        let aggregate_id = context.next_uuid();
        debug!(aggregate_id = %aggregate_id, "Generated new aggregate ID");

        let (aggregate, version) = match self.store.initialize_aggregate(&aggregate_id).await {
            Ok(result) => {
                let (_, v) = &result;
                debug!(version = %v, "Initialized aggregate");
                result
            }
            Err(e) => {
                error!(error = %e, "Failed to initialize aggregate");
                return Err(e);
            }
        };

        let events = match aggregate
            .handle_create(command, &self.services, context)
            .await
        {
            Ok(events) => {
                debug!(
                    event_count = events.len(),
                    "Generated events from create command"
                );
                events
            }
            Err(e) => {
                error!(error = %e, "Failed to handle create command");
                return Err(AggregateError::UserError(e.into()));
            }
        };

        match self
            .process(&aggregate_id, aggregate, version, events, metadata, context)
            .await
        {
            Ok(_) => {
                debug!("Processed events successfully");
            }
            Err(e) => {
                error!(error = %e, "Failed to process events");
                return Err(e);
            }
        }

        info!(aggregate_id = %aggregate_id, "Aggregate created successfully with metadata");
        Ok(aggregate_id)
    }

    async fn handle_events(
        &self,
        aggregate_id: &str,
        events: &[EventEnvelope<A>],
        context: &CqrsContext,
    ) {
        debug!("Handling events for dispatchers");
        let eh = &self.error_handler;
        for (i, dispatcher) in self.dispatchers.iter().enumerate() {
            debug!(dispatcher_index = i, "Dispatching events to dispatcher");
            match dispatcher.dispatch(aggregate_id, events, context).await {
                Ok(_) => debug!(dispatcher_index = i, "Successfully dispatched events"),
                Err(e) => {
                    error!(dispatcher_index = i, error = %e, "Failed to dispatch events");
                    eh(&e);
                }
            };
        }
        debug!("Finished handling events for all dispatchers");
    }

    pub async fn execute_update_with_metadata(
        &self,
        aggregate_id: &str,
        command: A::UpdateCommand,
        metadata: HashMap<String, String>,
        context: &CqrsContext,
    ) -> Result<(), AggregateError> {
        debug!("Executing update command with metadata");

        let (mut aggregate, version) = match self.store.load_aggregate(aggregate_id).await {
            Ok(result) => {
                let (_, v) = &result;
                debug!(version = %v, "Loaded aggregate");
                result
            }
            Err(e) => {
                error!(error = %e, "Failed to load aggregate");
                return Err(e);
            }
        };

        let events = match aggregate
            .handle_update(command, &self.services, context)
            .await
        {
            Ok(events) => {
                debug!(
                    event_count = events.len(),
                    "Generated events from update command"
                );
                events
            }
            Err(e) => {
                error!(error = %e, "Failed to handle update command");
                return Err(AggregateError::UserError(e.into()));
            }
        };

        for event in &events {
            if let Err(e) = aggregate.apply(event.clone()) {
                error!(error = %e, "Failed to apply event to aggregate");
                return Err(AggregateError::UserError(e.into()));
            }
        }
        debug!("Applied events to aggregate");

        let committed_events = match self
            .store
            .commit(events, &aggregate, metadata, version, context)
            .await
        {
            Ok(events) => {
                debug!(event_count = events.len(), "Committed events to store");
                events
            }
            Err(e) => {
                error!(error = %e, "Failed to commit events");
                return Err(e);
            }
        };

        if committed_events.is_empty() {
            debug!("No events committed, returning early");
            return Ok(());
        }

        debug!(
            event_count = committed_events.len(),
            "Dispatching events to handlers"
        );
        self.handle_events(aggregate_id, &committed_events, context)
            .await;

        info!("Aggregate updated successfully with metadata");
        Ok(())
    }

    async fn process(
        &self,
        aggregate_id: &str,
        mut aggregate: A,
        version: usize,
        events: Vec<A::Event>,
        metadata: HashMap<String, String>,
        context: &CqrsContext,
    ) -> Result<(), AggregateError> {
        debug!("Processing events for aggregate");

        for (i, event) in events.iter().enumerate() {
            debug!(
                event_index = i,
                event_type = event.event_type(),
                "Applying event to aggregate"
            );
            match aggregate.apply(event.clone()) {
                Ok(_) => debug!(event_index = i, "Successfully applied event to aggregate"),
                Err(e) => {
                    error!(event_index = i, error = %e, "Failed to apply event to aggregate");
                    return Err(AggregateError::UserError(e.into()));
                }
            }
        }
        debug!("Applied all events to aggregate");

        debug!("Committing events to store");
        let committed_events = match self
            .store
            .commit(events, &aggregate, metadata, version, context)
            .await
        {
            Ok(events) => {
                debug!(
                    event_count = events.len(),
                    "Successfully committed events to store"
                );
                events
            }
            Err(e) => {
                error!(error = %e, "Failed to commit events to store");
                return Err(e);
            }
        };

        if committed_events.is_empty() {
            debug!("No events committed, returning early");
            return Ok(());
        }

        debug!(
            event_count = committed_events.len(),
            "Dispatching committed events to handlers"
        );
        self.handle_events(aggregate_id, &committed_events, context)
            .await;

        debug!("Successfully processed all events");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::es::inmemory::InMemoryPersist;
    use crate::es::EventStoreImpl;
    use crate::testing::{CreateCommand, TestAggregate, TestEvent, UpdateCommand};
    use crate::CqrsCommandEngine;
    use crate::CqrsContext;
    use crate::EventEnvelope;
    use crate::EventStore;
    use futures::StreamExt;

    #[tokio::test]
    async fn test_create_aggregate() {
        // Preparation
        let persist = InMemoryPersist::<TestAggregate>::new();
        let store = EventStoreImpl::new(persist);
        let engine = CqrsCommandEngine::new(store, vec![], (), Box::new(|_e| {}));

        let context = CqrsContext::default();

        // Execution
        let aggregate_id = engine
            .execute_create(
                CreateCommand::Initialize {
                    name: "toto".to_string(),
                },
                &context,
            )
            .await
            .expect("Creation should succeed");

        // Verification
        assert!(!aggregate_id.is_empty(), "Aggregate ID should not be empty");
    }

    #[tokio::test]
    async fn test_update_aggregate() {
        // Preparation
        let persist = InMemoryPersist::<TestAggregate>::new();
        let store = EventStoreImpl::new(persist);
        let engine = CqrsCommandEngine::new(store, vec![], (), Box::new(|_e| {}));

        let context = CqrsContext::default();

        // Create the aggregate
        let aggregate_id = engine
            .execute_create(
                CreateCommand::Initialize {
                    name: "toto".to_string(),
                },
                &context,
            )
            .await
            .expect("Creation should succeed");

        // Execute the update
        engine
            .execute_update(&aggregate_id, UpdateCommand::Increment, &context)
            .await
            .expect("Update should succeed");

        // Verify via stored events
        let event_stream = engine
            .store
            .load_events(&aggregate_id)
            .await
            .expect("Event loading should succeed");

        let events: Vec<EventEnvelope<TestAggregate>> = event_stream
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .expect("Events should be valid");

        assert_eq!(events.len(), 2, "There should be two events");
        assert_eq!(
            events[0].payload,
            TestEvent::Created {
                name: "toto".to_string()
            }
        );
        assert!(matches!(events[1].payload, TestEvent::Incremented));
    }

    #[tokio::test]
    async fn test_multiple_updates() {
        // Preparation
        let persist = InMemoryPersist::<TestAggregate>::new();
        let store = EventStoreImpl::new(persist);
        let engine = CqrsCommandEngine::new(store, vec![], (), Box::new(|_e| {}));

        let context = CqrsContext::default();

        // Create the aggregate
        let aggregate_id = engine
            .execute_create(
                CreateCommand::Initialize {
                    name: "toto".to_string(),
                },
                &context,
            )
            .await
            .expect("Creation should succeed");

        // First update
        engine
            .execute_update(&aggregate_id, UpdateCommand::Increment, &context)
            .await
            .expect("First update should succeed");

        // Second update
        engine
            .execute_update(&aggregate_id, UpdateCommand::Increment, &context)
            .await
            .expect("Second update should succeed");

        // Verification
        let event_stream = engine
            .store
            .load_events(&aggregate_id)
            .await
            .expect("Event loading should succeed");

        let events: Vec<EventEnvelope<TestAggregate>> = event_stream
            .collect::<Vec<_>>()
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>()
            .expect("Events should be valid");

        assert_eq!(events.len(), 3, "There should be three events");
        assert_eq!(
            events[0].payload,
            TestEvent::Created {
                name: "toto".to_string()
            }
        );
        assert!(matches!(events[1].payload, TestEvent::Incremented));
        assert!(matches!(events[2].payload, TestEvent::Incremented));
    }
}
