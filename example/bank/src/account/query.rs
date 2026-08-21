use cqrs_rust_lib::read::Query;
use serde::{Deserialize, Serialize};
use utoipa::IntoParams;

#[derive(Debug, Clone, Serialize, Deserialize, IntoParams)]
pub struct AccountQuery {
    pub owner: Option<String>,
}

// No default_sort(): unpaged, and the 20 rows returned are an arbitrary subset of the
// matches — declare a sort if the order matters. See Pagination in cqrs_rust_lib.
impl Query for AccountQuery {}
