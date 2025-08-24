use serde::{Deserialize, Serialize};
use utoipa::IntoParams;

#[derive(Debug, Clone, Serialize, Deserialize, IntoParams)]
pub struct TodoListQuery {
    pub name: Option<String>,
    pub skip: Option<i64>,
    pub limit: Option<i64>,
}
