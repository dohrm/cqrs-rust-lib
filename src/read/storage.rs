use crate::read::Paged;
use crate::{AggregateError, CqrsContext};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fmt::Debug;

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("Missing parent id")]
    MissingParentId,
    #[error("Invalid parent id")]
    InvalidParentId,
    #[error("Unsupported method : {0}")]
    UnsupportedMethod(String),
}

pub trait HasId {
    fn field_id() -> &'static str;
    fn id(&self) -> &str;
    fn parent_field_id() -> Option<&'static str>;
    fn parent_id(&self) -> Option<&str>;
}

#[async_trait::async_trait]
pub trait Storage<V, Q>: Clone + Debug + Send + Sync
where
    V: Debug + Clone + Default + Serialize + DeserializeOwned + Send + Sync,
    Q: Clone + Debug + DeserializeOwned + Send + Sync,
{
    fn type_name(&self) -> &str;
    async fn filter(
        &self,
        parent_id: Option<String>,
        query: Q,
        context: CqrsContext,
    ) -> Result<Paged<V>, AggregateError>;

    async fn find_by_id(
        &self,
        parent_id: Option<String>,
        id: &str,
        context: CqrsContext,
    ) -> Result<Option<V>, AggregateError>;

    async fn save(&self, entity: V, context: CqrsContext) -> Result<(), AggregateError>;
}
