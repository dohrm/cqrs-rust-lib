use cqrs_rust_lib::read::Query;
use serde::{Deserialize, Serialize};
use utoipa::IntoParams;

#[derive(Debug, Clone, Serialize, Deserialize, IntoParams)]
pub struct TodoListQuery {
    pub name: Option<String>,
}

impl Query for TodoListQuery {}
