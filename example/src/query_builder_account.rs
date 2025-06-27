use crate::account::AccountQuery;
use cqrs_rust_lib::read::mongodb::{QueryBuilder, SkipLimit};
use cqrs_rust_lib::CqrsContext;
use mongodb::bson::Document;

#[derive(Debug, Clone)]
pub struct QueryBuilderAccount;

impl QueryBuilder<AccountQuery> for QueryBuilderAccount {
    fn to_query(&self, query: &AccountQuery, _context: &CqrsContext) -> Document {
        let mut doc = Document::new();
        if let Some(owner) = &query.owner {
            doc.insert("owner", owner);
        }
        doc
    }

    fn to_skip_limit(&self, query: &AccountQuery, _context: &CqrsContext) -> SkipLimit {
        SkipLimit::new(query.skip.map(|s| s as u64), query.limit)
    }

    fn to_sort(&self, _query: &AccountQuery, _context: &CqrsContext) -> Option<Document> {
        None
    }
}
