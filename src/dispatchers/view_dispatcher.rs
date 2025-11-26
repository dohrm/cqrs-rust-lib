use crate::read::storage::{DynStorage, HasId};
use crate::{Aggregate, AggregateError, CqrsContext, Dispatcher, EventEnvelope, View};
use serde::de::DeserializeOwned;
use std::fmt::Debug;

pub struct ViewDispatcher<A, V, Q> {
    _phantom: std::marker::PhantomData<(A, V, Q)>,
    storage: DynStorage<V, Q>,
}

impl<A, V, Q> ViewDispatcher<A, V, Q>
where
    A: Aggregate,
    V: View<A> + HasId,
    Q: Clone + Debug + DeserializeOwned + Send + Sync,
{
    pub fn new(storage: DynStorage<V, Q>) -> Self {
        Self {
            _phantom: std::marker::PhantomData,
            storage,
        }
    }
}

#[async_trait::async_trait]
impl<A, V, Q> Dispatcher<A> for ViewDispatcher<A, V, Q>
where
    A: Aggregate,
    V: View<A> + HasId,
    Q: Clone + Debug + DeserializeOwned + Send + Sync,
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
