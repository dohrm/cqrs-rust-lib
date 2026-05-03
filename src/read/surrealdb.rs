use crate::read::storage::{HasId, Storage, StorageError};
use crate::read::Paged;
use crate::{Aggregate, CqrsContext, CqrsError, Snapshot};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value as JsonValue;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::sync::Arc;
use surrealdb::engine::any::Any;
use surrealdb::Surreal;
use surrealdb_types::SurrealValue;

fn map_surreal_error(e: surrealdb::Error) -> CqrsError {
    CqrsError::database_error(e)
}

#[derive(Debug, Clone)]
pub struct SkipLimit {
    pub skip: Option<i64>,
    pub limit: Option<i64>,
}

impl SkipLimit {
    pub fn new(skip: Option<i64>, limit: Option<i64>) -> Self {
        Self { skip, limit }
    }
}

/// Query builder for SurrealDB read storage.
///
/// Implementations return SurrealQL WHERE fragments with named `$param` placeholders.
/// Param names must not start with `__cqrs_` (reserved for internal use).
pub trait SurrealQueryBuilder<Q>: Debug + Clone + Send + Sync {
    fn to_where(&self, query: &Q, context: &CqrsContext) -> Option<String>;
    fn to_order_by(&self, query: &Q, context: &CqrsContext) -> Option<String>;
    fn to_skip_limit(&self, query: &Q, context: &CqrsContext) -> SkipLimit;
    fn bind_params(&self, query: &Q, context: &CqrsContext) -> Vec<(String, JsonValue)>;
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
pub struct SurrealDBStorage<V, Q, QB> {
    _phantom: PhantomData<(V, Q)>,
    db: Surreal<Any>,
    type_name: String,
    table_name: String,
    query_builder: QB,
}

impl<V, Q, QB> SurrealDBStorage<V, Q, QB>
where
    V: Debug + Clone + Default + Serialize + DeserializeOwned + Send + Sync + HasId,
    Q: Debug + Clone + DeserializeOwned + Send,
    QB: SurrealQueryBuilder<Q>,
{
    #[must_use]
    pub fn new(db: Surreal<Any>, type_name: &str, query_builder: QB, table_name: &str) -> Self {
        Self {
            _phantom: PhantomData,
            db,
            type_name: type_name.to_string(),
            table_name: table_name.to_string(),
            query_builder,
        }
    }

    fn build_where(
        &self,
        base_where: Option<String>,
        parent_id: &Option<String>,
    ) -> Result<String, CqrsError> {
        let mut clauses: Vec<String> = Vec::new();
        if let Some(w) = base_where.filter(|w| !w.trim().is_empty()) {
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
impl<V, Q, QB> Storage<V, Q> for SurrealDBStorage<V, Q, QB>
where
    V: Debug + Clone + Default + Serialize + DeserializeOwned + Send + Sync + HasId,
    Q: Clone + Debug + DeserializeOwned + Send + Sync,
    QB: SurrealQueryBuilder<Q> + Send + Sync,
{
    fn type_name(&self) -> &str {
        &self.type_name
    }

    async fn filter(
        &self,
        parent_id: Option<String>,
        query: Q,
        context: CqrsContext,
    ) -> Result<Paged<V>, CqrsError> {
        let user_where = self.query_builder.to_where(&query, &context);
        let where_clause = self.build_where(user_where, &parent_id)?;
        let order_by = self
            .query_builder
            .to_order_by(&query, &context)
            .map(|s| format!("ORDER BY {}", s))
            .unwrap_or_default();
        let SkipLimit { skip, limit } = self.query_builder.to_skip_limit(&query, &context);
        let limit_v = limit.unwrap_or(20).max(0);
        let offset_v = skip.unwrap_or(0).max(0);
        let extra_params = self.query_builder.bind_params(&query, &context);

        let count_sql = format!(
            "SELECT count() AS cnt FROM {} {} GROUP ALL",
            self.table_name, where_clause
        );
        let mut count_q = self.db.query(count_sql);
        if let Some(pid) = parent_id.as_ref() {
            count_q = count_q.bind(("__cqrs_parent_id", pid.clone()));
        }
        for (k, v) in &extra_params {
            count_q = count_q.bind((k.clone(), v.clone()));
        }
        let mut r = count_q.await.map_err(map_surreal_error)?;
        let counts: Vec<CountRow> = r.take(0).map_err(map_surreal_error)?;
        let total = counts.first().map(|c| c.cnt).unwrap_or(0);

        // SELECT * so that fields referenced in ORDER BY are projected (SurrealDB v3 requirement).
        // Extra fields beyond `data` are silently ignored by serde during DataRow deserialization.
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
        for (k, v) in extra_params {
            select_q = select_q.bind((k, v));
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
        // Store the full entity in `data` (id included) so deserialization is a simple
        // serde_json::from_value without manual id re-injection.
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

/// Wraps `SurrealDBStorage<Snapshot<A>, Q, QB>` and exposes `Storage<A, Q>`, projecting snapshots
/// to their inner state on read. `save` is intentionally unsupported (snapshots are written by
/// the event store, not directly).
#[derive(Debug, Clone)]
pub struct SurrealDBFromSnapshotStorage<A, Q, QB>
where
    A: Aggregate,
    Q: Debug + Clone + DeserializeOwned + Send + Sync,
    QB: SurrealQueryBuilder<Q>,
{
    _phantom: PhantomData<(A, Q, QB)>,
    inner: Arc<SurrealDBStorage<Snapshot<A>, Q, QB>>,
}

impl<A, Q, QB> SurrealDBFromSnapshotStorage<A, Q, QB>
where
    A: Aggregate,
    Q: Debug + Clone + DeserializeOwned + Send + Sync,
    QB: SurrealQueryBuilder<Q>,
{
    #[must_use]
    pub fn new(inner: Arc<SurrealDBStorage<Snapshot<A>, Q, QB>>) -> Self {
        Self {
            _phantom: PhantomData,
            inner,
        }
    }
}

cqrs_async_trait! {
impl<A, Q, QB> Storage<A, Q> for SurrealDBFromSnapshotStorage<A, Q, QB>
where
    A: Aggregate,
    Q: Debug + Clone + DeserializeOwned + Send + Sync,
    QB: SurrealQueryBuilder<Q> + Send + Sync,
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
        Err(CqrsError::internal(StorageError::UnsupportedMethod(
            "SurrealDBFromSnapshotStorage#save".to_string(),
        ).to_string()))
    }
}
}

// ─── Tests ───────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;
    use crate::read::storage::Storage;
    use crate::CqrsContext;
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

    // ── Query builder ────────────────────────────────────────────────────────

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    struct ArticleQuery {
        min_score: Option<i32>,
    }

    #[derive(Debug, Clone)]
    struct ArticleQueryBuilder;

    impl SurrealQueryBuilder<ArticleQuery> for ArticleQueryBuilder {
        fn to_where(&self, query: &ArticleQuery, _ctx: &CqrsContext) -> Option<String> {
            query.min_score.map(|_| "data.score >= $qscore".to_string())
        }

        fn to_order_by(&self, _q: &ArticleQuery, _ctx: &CqrsContext) -> Option<String> {
            Some("data.score ASC".to_string())
        }

        fn to_skip_limit(&self, _q: &ArticleQuery, _ctx: &CqrsContext) -> SkipLimit {
            SkipLimit::new(None, Some(10))
        }

        fn bind_params(
            &self,
            query: &ArticleQuery,
            _ctx: &CqrsContext,
        ) -> Vec<(String, serde_json::Value)> {
            match query.min_score {
                Some(s) => vec![("qscore".into(), serde_json::json!(s))],
                None => vec![],
            }
        }
    }

    // ── Setup helper ─────────────────────────────────────────────────────────

    async fn setup() -> SurrealDBStorage<Article, ArticleQuery, ArticleQueryBuilder> {
        let db = connect("mem://").await.unwrap();
        db.use_ns("test").use_db("test").await.unwrap();
        db.query("DEFINE TABLE IF NOT EXISTS articles SCHEMALESS")
            .await
            .unwrap()
            .check()
            .unwrap();
        SurrealDBStorage::new(db, "article", ArticleQueryBuilder, "articles")
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
    async fn filter_with_min_score_bind_param() {
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

        // page 1, size 2 — ArticleQueryBuilder always returns limit=10 but we override here
        // by using a custom query that limits; for now just verify ordering
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
