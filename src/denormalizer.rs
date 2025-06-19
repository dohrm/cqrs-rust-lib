use crate::{Aggregate, EventEnvelope};
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::fmt::Debug;

#[async_trait::async_trait]
pub trait Dispatcher<A: Aggregate>: Send + Sync {
    async fn dispatch(&self, aggregate_id: &str, events: &[EventEnvelope<A>]);
}

pub trait View<A: Aggregate>: Debug + Default + Serialize + DeserializeOwned + Send + Sync {
    fn update(&self, event: &EventEnvelope<A>);
}
