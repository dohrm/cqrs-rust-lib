use crate::read::storage::HasId;
use crate::Aggregate;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
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

impl<A> HasId for Snapshot<A>
where
    A: Aggregate,
{
    fn field_id() -> &'static str {
        "_id"
    }

    fn id(&self) -> &str {
        self.aggregate_id.as_str()
    }

    fn parent_field_id() -> Option<&'static str> {
        None
    }

    fn parent_id(&self) -> Option<&str> {
        None
    }
}
