use crate::Aggregate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot<A>
where
    A: Aggregate,
{
    #[serde(rename = "_id")]
    pub aggregate_id: String,
    #[serde(deserialize_with = "A::deserialize")]
    pub state: A,
    pub version: usize,
}
