use crate::{Aggregate, CommandHandler, CqrsContext, EventEnvelope, View, ViewElements};
use http::StatusCode;
use serde::{Deserialize, Serialize};
#[cfg(feature = "utoipa")]
use utoipa::ToSchema;

#[derive(Debug, Clone, thiserror::Error)]
pub enum TestError {
    #[error("Test error {0}")]
    TestError(String),
}
impl From<&str> for TestError {
    fn from(value: &str) -> Self {
        Self::TestError(value.to_string())
    }
}

impl From<TestError> for crate::CqrsError {
    fn from(e: TestError) -> Self {
        crate::CqrsError::user_error(e)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
pub enum CreateCommand {
    Initialize { name: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
pub enum UpdateCommand {
    Increment,
    Decrement,
}

// Événements de test
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
pub enum TestEvent {
    Created { name: String },
    Updated { name: String },
    Incremented,
    Decremented,
}

// Implémentation du trait Event pour TestEvent
impl crate::Event for TestEvent {
    fn event_type(&self) -> String {
        match self {
            TestEvent::Created { .. } => "Created".to_string(),
            TestEvent::Updated { .. } => "Updated".to_string(),
            TestEvent::Incremented => "Incremented".to_string(),
            TestEvent::Decremented => "Decremented".to_string(),
        }
    }
}

// Define a simple aggregate for testing
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TestAggregate {
    id: String,
    counter: i32,
    name: String,
}

#[async_trait::async_trait]
impl Aggregate for TestAggregate {
    const TYPE: &'static str = "TEST";

    type Event = TestEvent;

    type Error = TestError;

    fn aggregate_id(&self) -> String {
        self.id.clone()
    }

    fn with_aggregate_id(self, id: String) -> Self {
        Self { id, ..self }
    }

    fn apply(&mut self, event: Self::Event) -> Result<(), Self::Error> {
        match event {
            TestEvent::Created { name } => self.name = name,
            TestEvent::Updated { name } => self.name = name,
            TestEvent::Incremented => self.counter += 1,
            TestEvent::Decremented => self.counter -= 1,
        }
        Ok(())
    }

    fn error(_status: StatusCode, details: &str) -> Self::Error {
        details.into()
    }
}

#[async_trait::async_trait]
impl CommandHandler for TestAggregate {
    type CreateCommand = CreateCommand;
    type UpdateCommand = UpdateCommand;
    type Services = ();

    async fn handle_create(
        &self,
        command: Self::CreateCommand,
        _services: &Self::Services,
        _context: &CqrsContext,
    ) -> Result<Vec<Self::Event>, Self::Error> {
        match command {
            CreateCommand::Initialize { name } => Ok(vec![TestEvent::Created { name }]),
        }
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
}

// Define a test view
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TestView {
    pub id: String,
    pub name: String,
    pub version: usize,
}

impl View<TestAggregate> for TestView {
    const TYPE: &'static str = "TEST_VIEW";
    const IS_CHILD_OF_AGGREGATE: bool = true;

    fn view_id(event: &EventEnvelope<TestAggregate>) -> String {
        event.aggregate_id.clone()
    }

    fn update(&self, event: &EventEnvelope<TestAggregate>) -> Option<Self> {
        let mut updated = self.clone();
        updated.id = event.aggregate_id.clone();
        updated.version = event.version;

        match &event.payload {
            TestEvent::Created { name } => {
                updated.name = name.clone();
                Some(updated)
            }
            TestEvent::Updated { name } => {
                updated.name = name.clone();
                Some(updated)
            }
            _ => None,
        }
    }
}

impl ViewElements<TestAggregate> for TestView {
    fn aggregate_id(&self) -> String {
        self.id.clone()
    }
}
