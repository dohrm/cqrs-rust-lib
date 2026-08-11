use crate::pg::{PgConn, PgPool, SharedClient};
use crate::read::query::Query;
use crate::read::sorter::{SortDirection, Sorter};
use crate::read::storage::{HasId, Storage, StorageError};
use crate::read::Paged;
use crate::{Aggregate, CqrsContext, CqrsError, Snapshot};
use rest_sql::{FieldMapper, IdentityMapper};
use rest_sql_drivers::tokio_postgres::PgCompiler;
use rest_sql_drivers::Driver;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value as JsonValue;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::sync::Arc;
use tokio_postgres::{types::ToSql, Client};

fn map_pg_error<E: std::error::Error + Send + Sync + 'static>(e: E) -> CqrsError {
    CqrsError::database_error(e)
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
    format!(" ORDER BY {}", parts.join(", "))
}

/// Read-side storage backed by a JSONB `data` column.
///
/// Connections are acquired through [`PgPool`], the same abstraction as
/// [`crate::es::postgres::PostgresPersist`]. The default `P = SharedClient`
/// keeps the single-`Arc<Client>` behaviour; pass a real pool via
/// [`Self::with_pool`].
#[derive(Debug, Clone)]
pub struct PostgresStorage<V, Q, M = IdentityMapper, P = SharedClient> {
    _phantom: PhantomData<(V, Q)>,
    pool: P,
    type_name: String,
    table_name: String,
    mapper: M,
}

impl<V, Q> PostgresStorage<V, Q, IdentityMapper, SharedClient> {
    #[must_use]
    pub fn new(client: Arc<Client>, type_name: &str, table_name: &str) -> Self {
        Self::with_mapper(client, type_name, table_name, IdentityMapper)
    }
}

impl<V, Q, M> PostgresStorage<V, Q, M, SharedClient>
where
    M: FieldMapper + Debug + Clone + Send + Sync,
{
    #[must_use]
    pub fn with_mapper(client: Arc<Client>, type_name: &str, table_name: &str, mapper: M) -> Self {
        Self::with_pool_and_mapper(SharedClient(client), type_name, table_name, mapper)
    }
}

impl<V, Q, P> PostgresStorage<V, Q, IdentityMapper, P>
where
    P: PgPool,
{
    #[must_use]
    pub fn with_pool(pool: P, type_name: &str, table_name: &str) -> Self {
        Self::with_pool_and_mapper(pool, type_name, table_name, IdentityMapper)
    }
}

impl<V, Q, M, P> PostgresStorage<V, Q, M, P>
where
    M: FieldMapper + Debug + Clone + Send + Sync,
    P: PgPool,
{
    #[must_use]
    pub fn with_pool_and_mapper(pool: P, type_name: &str, table_name: &str, mapper: M) -> Self {
        Self {
            _phantom: PhantomData,
            pool,
            type_name: type_name.to_string(),
            table_name: table_name.to_string(),
            mapper,
        }
    }
}

impl<V, Q, M, P> PostgresStorage<V, Q, M, P>
where
    V: HasId,
    Q: Query,
    M: FieldMapper + Debug + Clone + Send + Sync,
    P: PgPool,
{
    fn build_filter(
        &self,
        query: &Q,
        parent_id: &Option<String>,
    ) -> Result<(String, Vec<Box<dyn ToSql + Sync + Send>>), CqrsError> {
        let (mut where_sql, mut params): (String, Vec<Box<dyn ToSql + Sync + Send>>) =
            match query.filter() {
                Some(rsql) => PgCompiler::new(self.mapper.clone())
                    .compile(&rsql)
                    .map_err(|e| CqrsError::internal(e.to_string()))?,
                None => (String::new(), vec![]),
            };

        match (V::parent_field_id(), parent_id) {
            (Some(_), Some(pid)) => {
                params.push(Box::new(pid.clone()));
                let n = params.len();
                if where_sql.trim().is_empty() {
                    where_sql = format!("parent_id = ${}", n);
                } else {
                    where_sql = format!("({}) AND parent_id = ${}", where_sql, n);
                }
            }
            (Some(_), None) => {
                return Err(CqrsError::validation(
                    StorageError::MissingParentId.to_string(),
                ));
            }
            _ => {}
        }
        Ok((where_sql, params))
    }
}

cqrs_async_trait! {
impl<V, Q, M, P> Storage<V, Q> for PostgresStorage<V, Q, M, P>
where
    V: Debug + Clone + Default + Serialize + DeserializeOwned + Send + Sync + HasId,
    Q: Clone + Debug + Send + Sync + Query,
    M: FieldMapper + Debug + Clone + Send + Sync,
    P: PgPool,
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
        let (where_sql, params) = self.build_filter(&query, &parent_id)?;
        let where_full = if where_sql.trim().is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", where_sql)
        };

        let pagination = query.pagination().unwrap_or_default();
        let limit_v = pagination.limit.unwrap_or(20);
        let offset_v = pagination.skip.unwrap_or(0);
        let order_by = sorters_to_order_by(query.sort(), &self.mapper);

        let conn = self.pool.acquire().await?;

        let count_sql = format!(
            "SELECT COUNT(*)::BIGINT AS total FROM {}{}",
            self.table_name, where_full
        );
        let count_params: Vec<&(dyn ToSql + Sync)> = params
            .iter()
            .map(|b| b.as_ref() as &(dyn ToSql + Sync))
            .collect();
        let row = conn
            .client()
            .query_one(&count_sql, &count_params)
            .await
            .map_err(map_pg_error)?;
        let total: i64 = row.try_get::<_, i64>("total").map_err(map_pg_error)?;

        let param_offset = params.len() + 1;
        let select_sql = format!(
            "SELECT data FROM {}{}{} OFFSET ${} LIMIT ${}",
            self.table_name,
            where_full,
            order_by,
            param_offset,
            param_offset + 1
        );
        let mut select_params: Vec<Box<dyn ToSql + Sync + Send>> = params;
        select_params.push(Box::new(offset_v));
        select_params.push(Box::new(limit_v));
        let select_params_ref: Vec<&(dyn ToSql + Sync)> = select_params
            .iter()
            .map(|b| b.as_ref() as &(dyn ToSql + Sync))
            .collect();
        let rows = conn
            .client()
            .query(&select_sql, &select_params_ref)
            .await
            .map_err(map_pg_error)?;
        let mut items: Vec<V> = Vec::with_capacity(rows.len());
        for row in rows {
            let val: JsonValue = row.try_get::<_, JsonValue>("data").map_err(map_pg_error)?;
            let v: V = serde_json::from_value(val).map_err(CqrsError::serialization_error)?;
            items.push(v);
        }
        Ok(Paged::new(items, total, offset_v, limit_v))
    }

    async fn find_by_id(
        &self,
        parent_id: Option<String>,
        id: &str,
        _context: CqrsContext,
    ) -> Result<Option<V>, CqrsError> {
        let mut where_sql = String::from("id = $1");
        let mut params: Vec<&(dyn ToSql + Sync)> = vec![&id];
        if let (Some(_), Some(pid)) = (V::parent_field_id(), parent_id.as_ref()) {
            where_sql.push_str(&format!(" AND parent_id = ${}", params.len() + 1));
            params.push(pid);
        } else if V::parent_field_id().is_some() && parent_id.is_none() {
            return Err(CqrsError::validation(
                StorageError::MissingParentId.to_string(),
            ));
        }
        let sql = format!("SELECT data FROM {} WHERE {}", self.table_name, where_sql);
        let conn = self.pool.acquire().await?;
        let row = conn
            .client()
            .query_opt(&sql, &params)
            .await
            .map_err(map_pg_error)?;
        if let Some(row) = row {
            let val: JsonValue = row.try_get::<_, JsonValue>("data").map_err(map_pg_error)?;
            let v: V = serde_json::from_value(val).map_err(CqrsError::serialization_error)?;
            Ok(Some(v))
        } else {
            Ok(None)
        }
    }

    async fn save(&self, entity: V, _context: CqrsContext) -> Result<(), CqrsError> {
        let id = entity.id().to_string();
        let parent_id = entity.parent_id().map(|s| s.to_string());
        let data = serde_json::to_value(&entity).map_err(CqrsError::serialization_error)?;
        let mut data_obj = data;
        if let Some(obj) = data_obj.as_object_mut() {
            obj.remove(V::field_id());
        }
        if V::parent_field_id().is_some() && parent_id.is_none() {
            return Err(CqrsError::validation(
                StorageError::MissingParentId.to_string(),
            ));
        }
        let sql = format!(
            "INSERT INTO {} (id, parent_id, data) VALUES ($1, $2, $3) \
             ON CONFLICT (id) DO UPDATE SET parent_id = EXCLUDED.parent_id, data = EXCLUDED.data",
            self.table_name
        );
        let conn = self.pool.acquire().await?;
        conn.client()
            .execute(&sql, &[&id, &parent_id, &data_obj])
            .await
            .map_err(map_pg_error)?;
        Ok(())
    }
}
}

#[derive(Debug, Clone)]
pub struct PostgresFromSnapshotStorage<A, Q, M = IdentityMapper, P = SharedClient>
where
    A: Aggregate,
    Q: Debug + Clone + Send + Sync + Query,
    M: FieldMapper + Debug + Clone + Send + Sync,
    P: PgPool,
{
    _phantom: PhantomData<A>,
    inner: Arc<PostgresStorage<Snapshot<A>, Q, M, P>>,
}

impl<A, Q, P> PostgresFromSnapshotStorage<A, Q, IdentityMapper, P>
where
    A: Aggregate,
    Q: Debug + Clone + Send + Sync + Query,
    P: PgPool,
{
    #[must_use]
    pub fn new(inner: Arc<PostgresStorage<Snapshot<A>, Q, IdentityMapper, P>>) -> Self {
        Self {
            _phantom: PhantomData,
            inner,
        }
    }
}

impl<A, Q, M, P> PostgresFromSnapshotStorage<A, Q, M, P>
where
    A: Aggregate,
    Q: Debug + Clone + Send + Sync + Query,
    M: FieldMapper + Debug + Clone + Send + Sync,
    P: PgPool,
{
    #[must_use]
    pub fn with_mapper(inner: Arc<PostgresStorage<Snapshot<A>, Q, M, P>>) -> Self {
        Self {
            _phantom: PhantomData,
            inner,
        }
    }
}

cqrs_async_trait! {
impl<A, Q, M, P> Storage<A, Q> for PostgresFromSnapshotStorage<A, Q, M, P>
where
    A: Aggregate,
    Q: Clone + Debug + Send + Sync + Query,
    M: FieldMapper + Debug + Clone + Send + Sync,
    P: PgPool,
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
        Ok(result.map(|s| s.state))
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
        Err(CqrsError::database_error(StorageError::UnsupportedMethod(
            "SnapshotStorage#save".to_string(),
        )))
    }
}
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    struct Article {
        id: String,
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

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    struct ArticleQuery {}

    impl Query for ArticleQuery {}

    /// A pool that never yields a connection — enough to prove a custom
    /// `PgPool` is plumbed through every `Storage` method without a live
    /// database.
    #[derive(Debug, Clone)]
    struct FailingPool;

    cqrs_async_trait! {
    impl PgPool for FailingPool {
        type Connection = SharedClient;
        async fn acquire(&self) -> Result<Self::Connection, CqrsError> {
            Err(CqrsError::database_error("pool exhausted"))
        }
    }
    }

    fn storage() -> PostgresStorage<Article, ArticleQuery, IdentityMapper, FailingPool> {
        PostgresStorage::with_pool(FailingPool, "article", "articles")
    }

    #[tokio::test]
    async fn custom_pool_is_used_by_filter() {
        let err = storage()
            .filter(None, ArticleQuery::default(), CqrsContext::default())
            .await
            .unwrap_err();
        assert_eq!(err.code, "INFRASTRUCTURE_DATABASE_ERROR");
        assert!(err.message.contains("pool exhausted"));
    }

    #[tokio::test]
    async fn custom_pool_is_used_by_find_by_id() {
        let err = storage()
            .find_by_id(None, "abc", CqrsContext::default())
            .await
            .unwrap_err();
        assert!(err.message.contains("pool exhausted"));
    }

    #[tokio::test]
    async fn custom_pool_is_used_by_save() {
        let err = storage()
            .save(Article::default(), CqrsContext::default())
            .await
            .unwrap_err();
        assert!(err.message.contains("pool exhausted"));
    }

    #[test]
    fn default_type_params_keep_the_arc_client_constructors() {
        // Compile-time check: `new`/`with_mapper` still resolve to
        // `P = SharedClient` without naming the pool type parameter.
        fn _assert(client: Arc<Client>) {
            let _: PostgresStorage<Article, ArticleQuery> =
                PostgresStorage::new(client.clone(), "article", "articles");
            let _: PostgresStorage<Article, ArticleQuery, IdentityMapper> =
                PostgresStorage::with_mapper(client, "article", "articles", IdentityMapper);
        }
    }
}
