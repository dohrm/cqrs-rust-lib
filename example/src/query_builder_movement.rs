use crate::account::views::MovementQuery;
use cqrs_rust_lib::read::mongodb::{QueryBuilder, SkipLimit};
use cqrs_rust_lib::CqrsContext;
use mongodb::bson::Document;

#[derive(Debug, Clone)]
pub struct QueryBuilderMovement;

impl QueryBuilder<MovementQuery> for QueryBuilderMovement {
    fn to_query(&self, _query: &MovementQuery, _context: &CqrsContext) -> Document {
        
        // if let Some(owner) = &query.owner {
        //     doc.insert("owner", owner);
        // }
        Document::new()
    }

    fn to_skip_limit(&self, query: &MovementQuery, _context: &CqrsContext) -> SkipLimit {
        SkipLimit::new(query.skip.map(|s| s as u64), query.limit)
    }
}
