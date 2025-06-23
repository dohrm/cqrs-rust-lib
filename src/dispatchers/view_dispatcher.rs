use crate::read::storage::{HasId, Storage};
use crate::{Aggregate, AggregateError, CqrsContext, Dispatcher, EventEnvelope, View};
use serde::de::DeserializeOwned;
use std::fmt::Debug;

pub struct ViewDispatcher<A, V, S, Q> {
    _phantom: std::marker::PhantomData<(A, V, S, Q)>,
    storage: S,
}

impl<A, V, S, Q> ViewDispatcher<A, V, S, Q>
where
    A: Aggregate,
    V: View<A> + HasId,
    Q: Clone + Debug + DeserializeOwned + Send + Sync,
    S: Storage<V, Q>,
{
    pub fn new(storage: S) -> Self {
        Self {
            _phantom: std::marker::PhantomData,
            storage,
        }
    }
}

#[async_trait::async_trait]
impl<A, V, Q, S> Dispatcher<A> for ViewDispatcher<A, V, S, Q>
where
    A: Aggregate,
    V: View<A> + HasId,
    Q: Clone + Debug + DeserializeOwned + Send + Sync,
    S: Storage<V, Q>,
{
    async fn dispatch(
        &self,
        aggregate_id: &str,
        events: &[EventEnvelope<A>],
        context: &CqrsContext,
    ) -> Result<(), AggregateError> {
        for event in events {
            let view_id = V::view_id(event);
            let prev = self
                .storage
                .find_by_id(Some(aggregate_id.to_string()), &view_id, context.clone())
                .await?
                .unwrap_or_else(|| V::default());
            if let Some(next) = prev.update(event) {
                self.storage.save(next, context.clone()).await?;
            }
        }
        Ok(())
    }
}
