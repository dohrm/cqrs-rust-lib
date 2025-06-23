use serde::{Deserialize, Serialize};
use utoipa::IntoParams;

#[derive(Debug, Clone, Serialize, Deserialize, IntoParams)]
pub struct AccountQuery {
    pub owner: Option<String>,
    pub skip: Option<i64>,
    pub limit: Option<i64>,
}
