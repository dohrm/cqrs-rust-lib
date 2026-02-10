use crate::{Aggregate, CqrsContext, CqrsError, EventEnvelope};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fmt::Debug;

#[async_trait::async_trait]
pub trait Dispatcher<A: Aggregate>: Send + Sync {
    async fn dispatch(
        &self,
        aggregate_id: &str,
        events: &[EventEnvelope<A>],
        context: &CqrsContext,
    ) -> Result<(), CqrsError>;
}

pub trait View<A: Aggregate>:
    Debug + Clone + Default + Serialize + DeserializeOwned + Send + Sync
{
    const TYPE: &'static str;
    const IS_CHILD_OF_AGGREGATE: bool;

    fn view_id(event: &EventEnvelope<A>) -> String;
    fn update(&self, event: &EventEnvelope<A>) -> Option<Self>;
}

pub trait ViewElements<A: Aggregate>: View<A> {
    fn aggregate_id(&self) -> String;
}
