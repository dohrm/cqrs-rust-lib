use crate::game::GameQuery;
use cqrs_rust_lib::read::surrealdb::{SkipLimit, SurrealQueryBuilder};
use cqrs_rust_lib::CqrsContext;
use serde_json::Value as JsonValue;

#[derive(Debug, Clone)]
pub struct GameQueryBuilder;

impl SurrealQueryBuilder<GameQuery> for GameQueryBuilder {
    fn to_where(&self, query: &GameQuery, _context: &CqrsContext) -> Option<String> {
        let mut clauses: Vec<&str> = vec![];
        if query.category.is_some() {
            clauses.push("data.category = $qcat");
        }
        if query.available.is_some() {
            clauses.push("data.available = $qavail");
        }
        if clauses.is_empty() {
            None
        } else {
            Some(clauses.join(" AND "))
        }
    }

    fn to_order_by(&self, _query: &GameQuery, _context: &CqrsContext) -> Option<String> {
        Some("data.title ASC".to_string())
    }

    fn to_skip_limit(&self, query: &GameQuery, _context: &CqrsContext) -> SkipLimit {
        SkipLimit::new(query.skip, query.limit)
    }

    fn bind_params(&self, query: &GameQuery, _context: &CqrsContext) -> Vec<(String, JsonValue)> {
        let mut params = vec![];
        if let Some(cat) = &query.category {
            params.push(("qcat".to_string(), JsonValue::String(cat.clone())));
        }
        if let Some(avail) = query.available {
            params.push(("qavail".to_string(), JsonValue::Bool(avail)));
        }
        params
    }
}
