use crate::todolist::query::TodoListQuery;
use cqrs_rust_lib::read::postgres::{QueryBuilder, SkipLimit};
use cqrs_rust_lib::CqrsContext;
use tokio_postgres::types::ToSql;

#[derive(Debug, Clone)]
pub struct QueryBuilderTodoList;

impl QueryBuilder<TodoListQuery> for QueryBuilderTodoList {
    fn to_where(
        &self,
        query: &TodoListQuery,
        _context: &CqrsContext,
    ) -> (String, Vec<Box<dyn ToSql + Sync + Send>>) {
        if let Some(name) = query.name.as_ref() {
            ("name = $1".to_string(), vec![Box::new(name.to_string())])
        } else {
            ("1 = 1".to_string(), vec![])
        }
    }

    fn to_order_by(&self, _query: &TodoListQuery, _context: &CqrsContext) -> Option<String> {
        None
    }

    fn to_skip_limit(&self, query: &TodoListQuery, _context: &CqrsContext) -> SkipLimit {
        SkipLimit {
            skip: query.skip,
            limit: query.limit,
        }
    }
}
