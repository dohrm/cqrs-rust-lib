use crate::Aggregate;
use chrono::{DateTime, Utc};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Debug;

pub trait Event: Debug + Serialize + DeserializeOwned + Clone + PartialEq + Sync + Send {
    fn event_type(&self) -> String;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventEnvelope<A>
where
    A: Aggregate,
{
    #[serde(rename = "_id")]
    pub event_id: String,
    /// The id of the aggregate instance.
    pub aggregate_id: String,
    /// The version number for an aggregate instance.
    pub version: usize,
    /// Event payload.
    pub payload: A::Event,
    /// Additional metadata.
    pub metadata: HashMap<String, String>,
    /// The time when the event was created.
    pub at: DateTime<Utc>,
}
