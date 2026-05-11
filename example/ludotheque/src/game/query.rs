use cqrs_rust_lib::read::{Query, SortDirection, Sorter};
use serde::{Deserialize, Serialize};
use utoipa::IntoParams;

#[derive(Debug, Clone, Serialize, Deserialize, IntoParams)]
pub struct GameQuery {
    pub category: Option<String>,
    pub available: Option<bool>,
}

impl Query for GameQuery {
    fn default_sort() -> Option<Vec<Sorter>> {
        Some(vec![Sorter {
            field: "title".into(),
            direction: SortDirection::Asc,
        }])
    }
}
