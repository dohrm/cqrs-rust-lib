use crate::read::query::{Pagination, Query};
use crate::read::sorter::{SortDirection, Sorter};
use crate::read::storage::{HasId, Storage, StorageError};
use crate::read::Paged;
use crate::{Aggregate, CqrsContext, CqrsError, Snapshot};
use rest_sql::FieldMapper;
use rest_sql_drivers::surrealdb::SurrealCompiler;
use rest_sql_drivers::Driver;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value as JsonValue;
use std::borrow::Cow;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::sync::Arc;
use surrealdb::engine::any::Any;
use surrealdb::Surreal;
use surrealdb_types::SurrealValue;

fn map_surreal_error(e: surrealdb::Error) -> CqrsError {
    CqrsError::database_error(e)
}

/// Maps field names with a `data.` prefix — used for CQRS views where entities
/// are stored under a `data` field in SurrealDB records.
#[derive(Debug, Clone)]
pub struct DataPrefixMapper;

impl FieldMapper for DataPrefixMapper {
    fn map<'a>(&self, field: &'a str) -> Cow<'a, str> {
        Cow::Owned(format!("data.{}", field))
    }
}

fn sorters_to_order_by(sort: Option<Vec<Sorter>>, mapper: &impl FieldMapper) -> String {
    let sorters = match sort {
        Some(s) if !s.is_empty() => s,
        _ => return String::new(),
    };
    let parts: Vec<String> = sorters
        .iter()
        .map(|s| {
            let field = mapper.map(&s.field);
            let dir = match s.direction {
                SortDirection::Asc => "ASC",
                SortDirection::Desc => "DESC",
            };
            format!("{} {}", field, dir)
        })
        .collect();
    format!("ORDER BY {}", parts.join(", "))
}

#[derive(Debug, serde::Deserialize, SurrealValue)]
struct CountRow {
    cnt: i64,
}

#[derive(Debug, serde::Deserialize, SurrealValue)]
struct DataRow {
    data: JsonValue,
}

#[derive(Debug, Clone)]
pub struct SurrealDBStorage<V, Q, M = DataPrefixMapper> {
    _phantom: PhantomData<(V, Q)>,
    db: Surreal<Any>,
    type_name: String,
    table_name: String,
    mapper: M,
}

impl<V, Q> SurrealDBStorage<V, Q, DataPrefixMapper> {
    #[must_use]
    pub fn new(db: Surreal<Any>, type_name: &str, table_name: &str) -> Self {
        Self::with_mapper(db, type_name, table_name, DataPrefixMapper)
    }
}

impl<V, Q, M> SurrealDBStorage<V, Q, M>
where
    M: FieldMapper + Debug + Clone + Send + Sync,
{
    #[must_use]
    pub fn with_mapper(db: Surreal<Any>, type_name: &str, table_name: &str, mapper: M) -> Self {
        Self {
            _phantom: PhantomData,
            db,
            type_name: type_name.to_string(),
            table_name: table_name.to_string(),
            mapper,
        }
    }

    fn build_where(
        &self,
        user_filter: Option<String>,
        parent_id: &Option<String>,
    ) -> Result<String, CqrsError>
    where
        V: HasId,
    {
        let mut clauses: Vec<String> = Vec::new();
        if let Some(w) = user_filter.filter(|w| !w.trim().is_empty()) {
            clauses.push(format!("({})", w));
        }
        match (V::parent_field_id(), parent_id) {
            (Some(_), Some(_)) => clauses.push("parent_id = $__cqrs_parent_id".to_string()),
            (Some(_), None) => {
                return Err(CqrsError::validation(
                    StorageError::MissingParentId.to_string(),
                ));
            }
            _ => {}
        }
        if clauses.is_empty() {
            Ok(String::new())
        } else {
            Ok(format!("WHERE {}", clauses.join(" AND ")))
        }
    }
}

cqrs_async_trait! {
impl<V, Q, M> Storage<V, Q> for SurrealDBStorage<V, Q, M>
where
    V: Debug + Clone + Default + Serialize + DeserializeOwned + Send + Sync + HasId,
    Q: Clone + Debug + DeserializeOwned + Send + Sync + Query,
    M: FieldMapper + Debug + Clone + Send + Sync,
{
    fn type_name(&self) -> &str {
        &self.type_name
    }

    async fn filter(
        &self,
        parent_id: Option<String>,
        query: Q,
        _context: CqrsContext,
    ) -> Result<Paged<V>, CqrsError> {
        let user_filter = match query.filter() {
            Some(rsql) => Some(
                SurrealCompiler::new(self.mapper.clone())
                    .compile(&rsql)
                    .map_err(|e| CqrsError::internal(e.to_string()))?,
            ),
            None => None,
        };
        let where_clause = self.build_where(user_filter, &parent_id)?;
        let order_by = sorters_to_order_by(query.sort(), &self.mapper);

        let Pagination { skip, limit } = query.pagination().unwrap_or_default();
        let limit_v = limit.unwrap_or(20).max(0);
        let offset_v = skip.unwrap_or(0).max(0);

        let count_sql = format!(
            "SELECT count() AS cnt FROM {} {} GROUP ALL",
            self.table_name, where_clause
        );
        let mut count_q = self.db.query(count_sql);
        if let Some(pid) = parent_id.as_ref() {
            count_q = count_q.bind(("__cqrs_parent_id", pid.clone()));
        }
        let mut r = count_q.await.map_err(map_surreal_error)?;
        let counts: Vec<CountRow> = r.take(0).map_err(map_surreal_error)?;
        let total = counts.first().map(|c| c.cnt).unwrap_or(0);

        // SELECT * so fields referenced in ORDER BY are projected (SurrealDB v3 requirement).
        let select_sql = format!(
            "SELECT * FROM {} {} {} LIMIT $__cqrs_limit START $__cqrs_offset",
            self.table_name, where_clause, order_by
        );
        let mut select_q = self
            .db
            .query(select_sql)
            .bind(("__cqrs_limit", limit_v))
            .bind(("__cqrs_offset", offset_v));
        if let Some(pid) = parent_id.as_ref() {
            select_q = select_q.bind(("__cqrs_parent_id", pid.clone()));
        }
        let mut result = select_q.await.map_err(map_surreal_error)?;
        let rows: Vec<DataRow> = result.take(0).map_err(map_surreal_error)?;
        let mut items: Vec<V> = Vec::with_capacity(rows.len());
        for row in rows {
            let v: V = serde_json::from_value(row.data).map_err(CqrsError::serialization_error)?;
            items.push(v);
        }
        let page = if limit_v > 0 { offset_v / limit_v } else { 0 };
        Ok(Paged {
            items,
            total,
            page_size: limit_v,
            page,
        })
    }

    async fn find_by_id(
        &self,
        parent_id: Option<String>,
        id: &str,
        _context: CqrsContext,
    ) -> Result<Option<V>, CqrsError> {
        let id = id.to_string();
        let table = self.table_name.clone();
        let mut where_clause = String::from("id = type::record($__cqrs_table, $__cqrs_id)");
        match (V::parent_field_id(), parent_id.as_ref()) {
            (Some(_), Some(_)) => where_clause.push_str(" AND parent_id = $__cqrs_parent_id"),
            (Some(_), None) => {
                return Err(CqrsError::validation(
                    StorageError::MissingParentId.to_string(),
                ))
            }
            _ => {}
        }
        let sql = format!("SELECT data FROM {} WHERE {}", self.table_name, where_clause);
        let mut q = self
            .db
            .query(sql)
            .bind(("__cqrs_table", table))
            .bind(("__cqrs_id", id));
        if let Some(pid) = parent_id {
            q = q.bind(("__cqrs_parent_id", pid));
        }
        let mut result = q.await.map_err(map_surreal_error)?;
        let rows: Vec<DataRow> = result.take(0).map_err(map_surreal_error)?;
        match rows.into_iter().next() {
            Some(row) => {
                let v: V =
                    serde_json::from_value(row.data).map_err(CqrsError::serialization_error)?;
                Ok(Some(v))
            }
            None => Ok(None),
        }
    }

    async fn save(&self, entity: V, _context: CqrsContext) -> Result<(), CqrsError> {
        let id = entity.id().to_string();
        let parent_id = entity.parent_id().map(|s| s.to_string());
        let data = serde_json::to_value(&entity).map_err(CqrsError::serialization_error)?;
        if V::parent_field_id().is_some() && parent_id.is_none() {
            return Err(CqrsError::validation(
                StorageError::MissingParentId.to_string(),
            ));
        }
        let table = self.table_name.clone();
        self.db
            .query(
                "UPSERT type::record($__cqrs_table, $__cqrs_id) SET parent_id = $__cqrs_parent, data = $__cqrs_data",
            )
            .bind(("__cqrs_table", table))
            .bind(("__cqrs_id", id))
            .bind(("__cqrs_parent", parent_id))
            .bind(("__cqrs_data", data))
            .await
            .map_err(map_surreal_error)?
            .check()
            .map_err(map_surreal_error)?;
        Ok(())
    }
}
}

/// Wraps `SurrealDBStorage<Snapshot<A>, Q, M>` and exposes `Storage<A, Q>`, projecting snapshots
/// to their inner state on read. `save` is intentionally unsupported.
#[derive(Debug, Clone)]
pub struct SurrealDBFromSnapshotStorage<A, Q, M = DataPrefixMapper>
where
    A: Aggregate,
    Q: Debug + Clone + DeserializeOwned + Send + Sync + Query,
    M: FieldMapper + Debug + Clone + Send + Sync,
{
    _phantom: PhantomData<A>,
    inner: Arc<SurrealDBStorage<Snapshot<A>, Q, M>>,
}

impl<A, Q> SurrealDBFromSnapshotStorage<A, Q, DataPrefixMapper>
where
    A: Aggregate,
    Q: Debug + Clone + DeserializeOwned + Send + Sync + Query,
{
    #[must_use]
    pub fn new(inner: Arc<SurrealDBStorage<Snapshot<A>, Q, DataPrefixMapper>>) -> Self {
        Self {
            _phantom: PhantomData,
            inner,
        }
    }
}

impl<A, Q, M> SurrealDBFromSnapshotStorage<A, Q, M>
where
    A: Aggregate,
    Q: Debug + Clone + DeserializeOwned + Send + Sync + Query,
    M: FieldMapper + Debug + Clone + Send + Sync,
{
    #[must_use]
    pub fn with_mapper(inner: Arc<SurrealDBStorage<Snapshot<A>, Q, M>>) -> Self {
        Self {
            _phantom: PhantomData,
            inner,
        }
    }
}

cqrs_async_trait! {
impl<A, Q, M> Storage<A, Q> for SurrealDBFromSnapshotStorage<A, Q, M>
where
    A: Aggregate,
    Q: Clone + Debug + DeserializeOwned + Send + Sync + Query,
    M: FieldMapper + Debug + Clone + Send + Sync,
{
    fn type_name(&self) -> &str {
        A::TYPE
    }

    async fn filter(
        &self,
        parent_id: Option<String>,
        query: Q,
        context: CqrsContext,
    ) -> Result<Paged<A>, CqrsError> {
        let result = self.inner.filter(parent_id, query, context).await?;
        Ok(Paged {
            items: result.items.into_iter().map(|s| s.state).collect(),
            total: result.total,
            page: result.page,
            page_size: result.page_size,
        })
    }

    async fn find_by_id(
        &self,
        parent_id: Option<String>,
        id: &str,
        context: CqrsContext,
    ) -> Result<Option<A>, CqrsError> {
        Ok(self
            .inner
            .find_by_id(parent_id, id, context)
            .await?
            .map(|s| s.state))
    }

    async fn save(&self, _entity: A, _context: CqrsContext) -> Result<(), CqrsError> {
        Err(CqrsError::internal(
            StorageError::UnsupportedMethod("SurrealDBFromSnapshotStorage#save".to_string())
                .to_string(),
        ))
    }
}
}

// ─── Tests ───────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;
    use crate::read::query::{Pagination, Query};
    use crate::read::storage::Storage;
    use crate::read::Sorter;
    use crate::CqrsContext;
    use rest_sql::{Ast, Constraint, Operator, RestSql, Value};
    use serde::{Deserialize, Serialize};
    use surrealdb::engine::any::connect;

    // ── Test view type ───────────────────────────────────────────────────────

    #[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
    struct Article {
        id: String,
        title: String,
        score: i32,
    }

    impl HasId for Article {
        fn field_id() -> &'static str {
            "id"
        }
        fn id(&self) -> &str {
            &self.id
        }
        fn parent_field_id() -> Option<&'static str> {
            None
        }
        fn parent_id(&self) -> Option<&str> {
            None
        }
    }

    // ── Query ────────────────────────────────────────────────────────────────

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    struct ArticleQuery {
        min_score: Option<i32>,
    }

    impl Query for ArticleQuery {
        fn filter(&self) -> Option<RestSql> {
            let score = self.min_score?;
            RestSql::from_ast(Ast::Constraint(Constraint {
                field: "score".into(),
                operator: Operator::Gte,
                value: Value::Int(score as i64),
            }))
            .ok()
        }

        fn pagination(&self) -> Option<Pagination> {
            Some(Pagination {
                skip: None,
                limit: Some(10),
            })
        }

        fn sort(&self) -> Option<Vec<Sorter>> {
            Some(vec![Sorter {
                field: "score".into(),
                direction: crate::read::sorter::SortDirection::Asc,
            }])
        }
    }

    // ── Setup helper ─────────────────────────────────────────────────────────

    async fn setup() -> SurrealDBStorage<Article, ArticleQuery> {
        let db = connect("mem://").await.unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        db.query("DEFINE TABLE IF NOT EXISTS articles SCHEMALESS")
            .await
            .unwrap()
            .check()
            .unwrap();
        SurrealDBStorage::new(db, "article", "articles")
    }

    fn article(id: &str, title: &str, score: i32) -> Article {
        Article {
            id: id.to_string(),
            title: title.to_string(),
            score,
        }
    }

    // ── Tests ────────────────────────────────────────────────────────────────

    #[tokio::test]
    async fn save_and_find_by_id() {
        let store = setup().await;
        let ctx = CqrsContext::default();

        let a = article("a1", "Hello", 42);
        store.save(a.clone(), ctx.clone()).await.unwrap();

        let found = store.find_by_id(None, "a1", ctx).await.unwrap();
        assert_eq!(found, Some(a));
    }

    #[tokio::test]
    async fn find_by_id_returns_none_when_missing() {
        let store = setup().await;
        let ctx = CqrsContext::default();

        let found = store.find_by_id(None, "nonexistent", ctx).await.unwrap();
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn upsert_replaces_existing_record() {
        let store = setup().await;
        let ctx = CqrsContext::default();

        store
            .save(article("a1", "First", 1), ctx.clone())
            .await
            .unwrap();
        store
            .save(article("a1", "Updated", 99), ctx.clone())
            .await
            .unwrap();

        let found = store.find_by_id(None, "a1", ctx).await.unwrap().unwrap();
        assert_eq!(found.title, "Updated");
        assert_eq!(found.score, 99);
    }

    #[tokio::test]
    async fn filter_returns_all_when_no_where() {
        let store = setup().await;
        let ctx = CqrsContext::default();

        for (id, title, score) in [("a1", "A", 10), ("a2", "B", 20), ("a3", "C", 30)] {
            store
                .save(article(id, title, score), ctx.clone())
                .await
                .unwrap();
        }

        let result = store
            .filter(None, ArticleQuery::default(), ctx)
            .await
            .unwrap();
        assert_eq!(result.total, 3);
        assert_eq!(result.items.len(), 3);
    }

    #[tokio::test]
    async fn filter_with_min_score() {
        let store = setup().await;
        let ctx = CqrsContext::default();

        for (id, title, score) in [("a1", "Low", 5), ("a2", "Mid", 50), ("a3", "High", 100)] {
            store
                .save(article(id, title, score), ctx.clone())
                .await
                .unwrap();
        }

        let result = store
            .filter(
                None,
                ArticleQuery {
                    min_score: Some(50),
                },
                ctx,
            )
            .await
            .unwrap();
        assert_eq!(result.total, 2);
        assert!(result.items.iter().all(|a| a.score >= 50));
    }

    #[tokio::test]
    async fn filter_ordering() {
        let store = setup().await;
        let ctx = CqrsContext::default();

        for i in 1..=5i32 {
            store
                .save(article(&format!("a{i}"), "item", i * 10), ctx.clone())
                .await
                .unwrap();
        }

        let result = store
            .filter(None, ArticleQuery::default(), ctx)
            .await
            .unwrap();
        let scores: Vec<i32> = result.items.iter().map(|a| a.score).collect();
        assert!(
            scores.windows(2).all(|w| w[0] <= w[1]),
            "items should be sorted ascending by score"
        );
    }
}
