use serde::{Deserialize, Serialize};
use utoipa::IntoParams;

#[derive(Debug, Clone, Serialize, Deserialize, IntoParams)]
pub struct GameQuery {
    pub category: Option<String>,
    pub available: Option<bool>,
    pub skip: Option<i64>,
    pub limit: Option<i64>,
}
