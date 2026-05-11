use crate::read::Paged;
use crate::{CqrsContext, CqrsError, MaybeSend, MaybeSync};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fmt::Debug;
use std::sync::Arc;

// DeserializeOwned is kept for V (entity) but NOT required for Q (query).
// Q's type system contract is Query + Clone + Debug + MaybeSend + MaybeSync.
// Individual backend impls may add DeserializeOwned to Q if they need it,
// but the trait itself does not — allowing non-Deserialize query wrappers
// like CqrsHttpQuery<Q>.

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

#[cfg(not(target_arch = "wasm32"))]
pub type DynStorage<V, Q> = Arc<dyn Storage<V, Q> + Send + Sync>;
#[cfg(target_arch = "wasm32")]
pub type DynStorage<V, Q> = Arc<dyn Storage<V, Q>>;

cqrs_async_trait! {
pub trait Storage<V, Q>
where
    V: Debug + Clone + Default + Serialize + DeserializeOwned + MaybeSend + MaybeSync,
    Q: Clone + Debug + MaybeSend + MaybeSync,
{
    fn type_name(&self) -> &str;
    async fn filter(
        &self,
        parent_id: Option<String>,
        query: Q,
        context: CqrsContext,
    ) -> Result<Paged<V>, CqrsError>;

    async fn find_by_id(
        &self,
        parent_id: Option<String>,
        id: &str,
        context: CqrsContext,
    ) -> Result<Option<V>, CqrsError>;

    async fn save(&self, entity: V, context: CqrsContext) -> Result<(), CqrsError>;
}
}
