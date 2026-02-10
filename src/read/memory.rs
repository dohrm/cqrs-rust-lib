use crate::{Aggregate, CqrsError, EventEnvelope, View};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tracing::debug;

/// A simple in-memory view store that can be used for testing or simple applications.
pub struct InMemoryViewStore<A, V>
where
    A: Aggregate,
    V: View<A>,
{
    views: Arc<Mutex<HashMap<String, V>>>,
    _phantom: std::marker::PhantomData<A>,
}

impl<A, V> InMemoryViewStore<A, V>
where
    A: Aggregate,
    V: View<A>,
{
    /// Creates a new in-memory view store.
    #[must_use]
    pub fn new() -> Self {
        Self {
            views: Arc::new(Mutex::new(HashMap::new())),
            _phantom: std::marker::PhantomData,
        }
    }

    /// Gets a view by its ID.
    pub fn get_view(&self, view_id: &str) -> Option<V> {
        let views = self.views.lock().unwrap();
        views.get(view_id).cloned()
    }

    /// Gets all views in the store.
    pub fn get_all_views(&self) -> HashMap<String, V> {
        let views = self.views.lock().unwrap();
        views.clone()
    }

    /// Updates a view with an event.
    pub fn update_view(&self, event: &EventEnvelope<A>) -> Result<(), CqrsError> {
        debug!("Updating view with event");

        let view_id = V::view_id(event);
        let mut views = self.views.lock().unwrap();

        let view = views.entry(view_id.clone()).or_default();

        if let Some(updated_view) = view.update(event) {
            debug!(view_id = %view_id, "View updated successfully");
            views.insert(view_id, updated_view);
        } else {
            debug!(view_id = %view_id, "View not updated (no changes)");
        }

        Ok(())
    }

    /// Clears all views from the store.
    pub fn clear(&self) {
        let mut views = self.views.lock().unwrap();
        views.clear();
    }
}

impl<A, V> Default for InMemoryViewStore<A, V>
where
    A: Aggregate,
    V: View<A>,
{
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Aggregate, Event, ViewElements};
    use chrono::Utc;
    use http::StatusCode;
    use serde::{Deserialize, Serialize};
    use std::error::Error;
    use std::fmt;

    // Custom error type that implements std::error::Error
    #[derive(Debug, Clone)]
    struct TestError(String);

    impl fmt::Display for TestError {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}", self.0)
        }
    }

    impl Error for TestError {}

    // Simple event for testing
    #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
    #[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
    enum TestEvent {
        Created { name: String },
        Updated { name: String },
    }

    impl Event for TestEvent {
        fn event_type(&self) -> String {
            match self {
                TestEvent::Created { .. } => "Created".to_string(),
                TestEvent::Updated { .. } => "Updated".to_string(),
            }
        }
    }

    // Simple aggregate for testing
    #[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
    struct TestAggregate {
        id: String,
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
            }
            Ok(())
        }

        fn error(_status: StatusCode, details: &str) -> Self::Error {
            TestError(details.to_string())
        }
    }

    // Simple view for testing
    #[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
    struct TestView {
        id: String,
        name: String,
        version: usize,
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
            }
        }
    }

    impl ViewElements<TestAggregate> for TestView {
        fn aggregate_id(&self) -> String {
            self.id.clone()
        }
    }

    #[test]
    fn test_in_memory_view_store() {
        // Create a view store
        let view_store = InMemoryViewStore::<TestAggregate, TestView>::new();

        // Create a test event
        let event = EventEnvelope {
            event_id: "event1".to_string(),
            aggregate_id: "agg1".to_string(),
            version: 1,
            payload: TestEvent::Created {
                name: "Test 1".to_string(),
            },
            metadata: HashMap::new(),
            at: Utc::now(),
        };

        // Update the view with the event
        view_store.update_view(&event).unwrap();

        // Verify the view was created and updated
        let view = view_store.get_view("agg1").unwrap();
        assert_eq!(view.id, "agg1");
        assert_eq!(view.name, "Test 1");
        assert_eq!(view.version, 1);

        // Create another test event
        let event2 = EventEnvelope {
            event_id: "event2".to_string(),
            aggregate_id: "agg1".to_string(),
            version: 2,
            payload: TestEvent::Updated {
                name: "Test 1 Updated".to_string(),
            },
            metadata: HashMap::new(),
            at: Utc::now(),
        };

        // Update the view with the second event
        view_store.update_view(&event2).unwrap();

        // Verify the view was updated
        let updated_view = view_store.get_view("agg1").unwrap();
        assert_eq!(updated_view.id, "agg1");
        assert_eq!(updated_view.name, "Test 1 Updated");
        assert_eq!(updated_view.version, 2);

        // Verify we can get all views
        let all_views = view_store.get_all_views();
        assert_eq!(all_views.len(), 1);
        assert!(all_views.contains_key("agg1"));

        // Clear the views
        view_store.clear();

        // Verify the views were cleared
        assert!(view_store.get_view("agg1").is_none());
    }
}
