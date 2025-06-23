use crate::{Aggregate, AggregateError, CqrsContext, Dispatcher, EventEnvelope};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tracing::{debug, info, instrument};

/// A simple in-memory dispatcher that stores events in memory.
/// Useful for testing or simple applications.
pub struct InMemoryDispatcher<A: Aggregate> {
    events: Arc<Mutex<HashMap<String, Vec<EventEnvelope<A>>>>>,
}

impl<A: Aggregate> InMemoryDispatcher<A> {
    /// Creates a new in-memory dispatcher.
    #[must_use]
    pub fn new() -> Self {
        Self {
            events: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Gets all events for a specific aggregate.
    pub fn get_events(&self, aggregate_id: &str) -> Vec<EventEnvelope<A>> {
        let events = self.events.lock().unwrap();
        events.get(aggregate_id).cloned().unwrap_or_default()
    }

    /// Gets all events stored in the dispatcher.
    pub fn get_all_events(&self) -> HashMap<String, Vec<EventEnvelope<A>>> {
        let events = self.events.lock().unwrap();
        events.clone()
    }

    /// Clears all events from the dispatcher.
    pub fn clear(&self) {
        let mut events = self.events.lock().unwrap();
        events.clear();
    }
}

impl<A: Aggregate> Default for InMemoryDispatcher<A> {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl<A: Aggregate> Dispatcher<A> for InMemoryDispatcher<A> {
    async fn dispatch(
        &self,
        aggregate_id: &str,
        events: &[EventEnvelope<A>],
        _context: &CqrsContext,
    ) -> Result<(), AggregateError> {
        debug!("Dispatching events to in-memory store");

        let mut store = self.events.lock().unwrap();
        let aggregate_events = store.entry(aggregate_id.to_string()).or_default();

        for event in events {
            debug!(event_id = %event.event_id, version = %event.version, "Adding event to in-memory store");
            aggregate_events.push(event.clone());
        }

        info!(
            event_count = events.len(),
            "Successfully dispatched events to in-memory store"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{TestAggregate, TestEvent};
    use crate::CqrsContext;
    use chrono::Utc;

    #[tokio::test]
    async fn test_in_memory_dispatcher() {
        // Create a dispatcher
        let dispatcher = InMemoryDispatcher::<TestAggregate>::new();

        // Create a context
        let context = CqrsContext::default();

        // Create some test events
        let events = vec![
            EventEnvelope {
                event_id: "event1".to_string(),
                aggregate_id: "agg1".to_string(),
                version: 1,
                payload: TestEvent::Created {
                    name: "toto".to_string(),
                },
                metadata: HashMap::new(),
                at: Utc::now(),
            },
            EventEnvelope {
                event_id: "event2".to_string(),
                aggregate_id: "agg1".to_string(),
                version: 2,
                payload: TestEvent::Updated {
                    name: "toto".to_string(),
                },
                metadata: HashMap::new(),
                at: Utc::now(),
            },
        ];

        // Dispatch the events
        dispatcher
            .dispatch("agg1", &events, &context)
            .await
            .unwrap();

        // Verify the events were stored
        let stored_events = dispatcher.get_events("agg1");
        assert_eq!(stored_events.len(), 2);
        assert_eq!(stored_events[0].event_id, "event1");
        assert_eq!(stored_events[1].event_id, "event2");

        // Verify we can get all events
        let all_events = dispatcher.get_all_events();
        assert_eq!(all_events.len(), 1);
        assert!(all_events.contains_key("agg1"));

        // Clear the events
        dispatcher.clear();

        // Verify the events were cleared
        let stored_events = dispatcher.get_events("agg1");
        assert_eq!(stored_events.len(), 0);
    }
}
