use crate::context::CqrsContext;
use crate::denormalizer::Dispatcher;
use crate::errors::AggregateError;
use crate::event_store::EventStore;
use crate::Aggregate;
use std::collections::HashMap;

/// The `CqrsCommandEngine` struct is a Command Query Responsibility Segregation (CQRS) engine
/// designed to handle commands and communication with an underlying event store and various dispatchers.
/// It acts as the main entry point for command processing and encapsulates the behavior specific to an aggregate.
///
/// # Type Parameters
/// - `A`: The type of the aggregate managed by this CQRS engine.
///        The aggregate represents the domain behavior and state transitions.
/// - `ES`: The type of the event store used to persist events related to the aggregate.
///
/// # Bounds
/// - `A`: Must implement the `Aggregate` trait. This ensures the aggregate provides necessary
///         functionality such as validating commands or applying events to mutate its state.
/// - `ES`: Must implement the `EventStore<A>` trait. This ensures the event store works
///          with the specified aggregate type for persisting and retrieving events.
///
/// # Fields
/// - `store: ES`
///   The event store instance used to persist and retrieve events associated with the aggregate.
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
pub struct CqrsCommandEngine<A, ES>
where
    A: Aggregate,
    ES: EventStore<A>,
{
    store: ES,
    dispatchers: Vec<Box<dyn Dispatcher<A>>>,
    services: A::Services,
}

impl<A, ES> CqrsCommandEngine<A, ES>
where
    A: Aggregate,
    ES: EventStore<A>,
{
    #[must_use]
    pub fn new(store: ES, dispatchers: Vec<Box<dyn Dispatcher<A>>>, services: A::Services) -> Self {
        Self {
            store,
            dispatchers,
            services,
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
        self.execute_create_with_metadata(command, HashMap::new(), context)
            .await
    }

    pub async fn execute_update(
        &self,
        aggregate_id: &str,
        command: A::UpdateCommand,
        context: &CqrsContext,
    ) -> Result<(), AggregateError> {
        self.execute_update_with_metadata(aggregate_id, command, HashMap::new(), context)
            .await
    }

    pub async fn execute_create_with_metadata(
        &self,
        command: A::CreateCommand,
        metadata: HashMap<String, String>,
        context: &CqrsContext,
    ) -> Result<String, AggregateError> {
        let aggregate_id = context.next_uuid();
        let (aggregate, version) = self.store.initialize_aggregate(&aggregate_id).await?;
        let events = aggregate
            .handle_create(command, &self.services, context)
            .await
            .map_err(|e| AggregateError::UserError(e.into()))?;
        self.process(&aggregate_id, aggregate, version, events, metadata, context)
            .await?;
        Ok(aggregate_id)
    }

    pub async fn execute_update_with_metadata(
        &self,
        aggregate_id: &str,
        command: A::UpdateCommand,
        metadata: HashMap<String, String>,
        context: &CqrsContext,
    ) -> Result<(), AggregateError> {
        let (mut aggregate, version) = self.store.load_aggregate(aggregate_id).await?;
        let events = aggregate
            .handle_update(command, &self.services, context)
            .await
            .map_err(|e| AggregateError::UserError(e.into()))?;

        for event in &events {
            aggregate
                .apply(event.clone())
                .map_err(|e| AggregateError::UserError(e.into()))?;
        }

        let committed_events = self
            .store
            .commit(events, &aggregate, metadata, version, context)
            .await?;

        if committed_events.is_empty() {
            return Ok(());
        }
        for dispatcher in &self.dispatchers {
            let events_to_dispatch = committed_events.as_slice();
            dispatcher.dispatch(aggregate_id, events_to_dispatch).await;
        }
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
        for event in &events {
            aggregate
                .apply(event.clone())
                .map_err(|e| AggregateError::UserError(e.into()))?;
        }

        let committed_events = self
            .store
            .commit(events, &aggregate, metadata, version, context)
            .await?;

        if committed_events.is_empty() {
            return Ok(());
        }
        for dispatcher in &self.dispatchers {
            let events_to_dispatch = committed_events.as_slice();
            dispatcher.dispatch(aggregate_id, events_to_dispatch).await;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        es::inmemory::InMemoryPersist, es::EventStoreImpl, Aggregate, CqrsCommandEngine,
        CqrsContext, EventStore,
    };
    use http::StatusCode;
    use serde::{Deserialize, Serialize};
    use utoipa::ToSchema;

    // Structure de test pour l'agrégat
    #[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, ToSchema)]
    struct TestAggregate {
        id: String,
        counter: i32,
    }

    // Commandes de test
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
    enum CreateCommand {
        Initialize,
    }

    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
    enum UpdateCommand {
        Increment,
        Decrement,
    }

    // Événements de test
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
    enum TestEvent {
        Created,
        Incremented,
        Decremented,
    }

    // Implémentation du trait Event pour TestEvent
    impl crate::Event for TestEvent {
        fn event_type(&self) -> String {
            match self {
                TestEvent::Created => "Created".to_string(),
                TestEvent::Incremented => "Incremented".to_string(),
                TestEvent::Decremented => "Decremented".to_string(),
            }
        }
    }

    // Implémentation de l'agrégat
    #[async_trait::async_trait]
    impl Aggregate for TestAggregate {
        const TYPE: &'static str = "TEST";

        type CreateCommand = CreateCommand;
        type UpdateCommand = UpdateCommand;
        type Event = TestEvent;
        type Error = std::io::Error;
        type Services = ();

        fn aggregate_id(&self) -> String {
            self.id.clone()
        }

        async fn handle_create(
            &self,
            _command: Self::CreateCommand,
            _services: &Self::Services,
            _context: &CqrsContext,
        ) -> Result<Vec<Self::Event>, Self::Error> {
            Ok(vec![TestEvent::Created])
        }

        async fn handle_update(
            &self,
            command: Self::UpdateCommand,
            _services: &Self::Services,
            _context: &CqrsContext,
        ) -> Result<Vec<Self::Event>, Self::Error> {
            match command {
                UpdateCommand::Increment => Ok(vec![TestEvent::Incremented]),
                UpdateCommand::Decrement => Ok(vec![TestEvent::Decremented]),
            }
        }

        fn apply(&mut self, event: Self::Event) -> Result<(), Self::Error> {
            match event {
                TestEvent::Created => {}
                TestEvent::Incremented => self.counter += 1,
                TestEvent::Decremented => self.counter -= 1,
            }
            Ok(())
        }

        fn with_aggregate_id(self, id: String) -> Self {
            Self { id, ..self }
        }

        fn error(_status: StatusCode, details: &str) -> Self::Error {
            std::io::Error::new(std::io::ErrorKind::Other, details)
        }
    }

    #[tokio::test]
    async fn test_create_aggregate() {
        // Préparation
        let persist = InMemoryPersist::<TestAggregate>::new();
        let store = EventStoreImpl::new(persist);
        let engine = CqrsCommandEngine::new(store, vec![], ());

        let context = CqrsContext::default();

        // Exécution
        let aggregate_id = engine
            .execute_create(CreateCommand::Initialize, &context)
            .await
            .expect("La création devrait réussir");

        // Vérification
        assert!(
            !aggregate_id.is_empty(),
            "L'ID de l'agrégat ne devrait pas être vide"
        );
    }

    #[tokio::test]
    async fn test_update_aggregate() {
        // Préparation
        let persist = InMemoryPersist::<TestAggregate>::new();
        let store = EventStoreImpl::new(persist);
        let engine = CqrsCommandEngine::new(store, vec![], ());

        let context = CqrsContext::default();

        // Création de l'agrégat
        let aggregate_id = engine
            .execute_create(CreateCommand::Initialize, &context)
            .await
            .expect("La création devrait réussir");

        // Exécution de la mise à jour
        engine
            .execute_update(&aggregate_id, UpdateCommand::Increment, &context)
            .await
            .expect("La mise à jour devrait réussir");

        // Vérification via les événements stockés
        let events = engine
            .store
            .load_events(&aggregate_id)
            .await
            .expect("Le chargement des événements devrait réussir");

        assert_eq!(events.len(), 2, "Il devrait y avoir deux événements");
        assert!(matches!(events[0].payload, TestEvent::Created));
        assert!(matches!(events[1].payload, TestEvent::Incremented));
    }

    #[tokio::test]
    async fn test_multiple_updates() {
        // Préparation
        let persist = InMemoryPersist::<TestAggregate>::new();
        let store = EventStoreImpl::new(persist);
        let engine = CqrsCommandEngine::new(store, vec![], ());

        let context = CqrsContext::default();

        // Création de l'agrégat
        let aggregate_id = engine
            .execute_create(CreateCommand::Initialize, &context)
            .await
            .expect("La création devrait réussir");

        // Premier update
        engine
            .execute_update(&aggregate_id, UpdateCommand::Increment, &context)
            .await
            .expect("Premier update devrait réussir");

        // Deuxième update
        engine
            .execute_update(&aggregate_id, UpdateCommand::Increment, &context)
            .await
            .expect("Deuxième update devrait réussir");

        // Vérification
        let events = engine
            .store
            .load_events(&aggregate_id)
            .await
            .expect("Le chargement des événements devrait réussir");

        assert_eq!(events.len(), 3, "Il devrait y avoir trois événements");
        assert!(matches!(events[0].payload, TestEvent::Created));
        assert!(matches!(events[1].payload, TestEvent::Incremented));
        assert!(matches!(events[2].payload, TestEvent::Incremented));
    }
}
