use crate::pg::{PgConn, PgPool, SharedClient};
use crate::read::page_order::warn_if_page_order_undefined;
use crate::read::query::Query;
use crate::read::sorter::order_by_clause;
use crate::read::storage::{HasId, Storage, StorageError};
use crate::read::Paged;
use crate::{Aggregate, CqrsContext, CqrsError};
use rest_sql::{FieldMapper, IdentityMapper};
use std::borrow::Cow;
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

/// Maps a logical field name onto a JSONB member of the `data` column.
///
/// The snapshot table stores the whole aggregate in `data`, so a filter on `name` has to
/// compile to `data->>'name'`; the default [`IdentityMapper`] would emit `name` and the
/// server would answer `column "name" does not exist`. This is the Postgres counterpart
/// of SurrealDB's `DataPrefixMapper`, and it is the default for
/// [`PostgresFromSnapshotStorage`] for the same reason.
///
/// **Text only.** `->>` yields `text`, while the driver binds a filter value by its own
/// type — an integer as `i64`, a bool as `bool`. tokio-postgres refuses that pair before
/// it reaches the wire, so `counter==3` against a snapshot answers `500`, not a wrong
/// result. Only string comparisons work here.
///
/// That is a property of reading a JSONB blob without knowing its field types, and it is
/// why a snapshot table is a shortcut rather than a read model: project a real view when
/// a filter needs types. Pinned by `snapshot_read_path` in `tests/`.
///
/// Interpolating the field name is safe here because nothing unvalidated reaches a
/// mapper: `_q` names are checked against the query struct and `sort` names against
/// [`Sorter::validated_field`].
#[derive(Debug, Clone, Copy, Default)]
pub struct JsonbDataMapper;

impl FieldMapper for JsonbDataMapper {
    fn map<'a>(&self, field: &'a str) -> Cow<'a, str> {
        Cow::Owned(format!("data->>'{field}'"))
    }
}

/// Compiles a query's RSQL filter into a `WHERE` fragment and its bound parameters.
///
/// Shared by the view storage and the snapshot storage: they read different tables with
/// different key columns, but a filter compiles the same way for both.
fn compile_where<Q, M>(
    query: &Q,
    mapper: &M,
) -> Result<(String, Vec<Box<dyn ToSql + Sync + Send>>), CqrsError>
where
    Q: Query,
    M: FieldMapper + Clone,
{
    match query.filter() {
        Some(rsql) => PgCompiler::new(mapper.clone())
            .compile(&rsql)
            .map_err(|e| CqrsError::internal(e.to_string())),
        None => Ok((String::new(), vec![])),
    }
}

/// Runs the count + page pair that both storages need.
///
/// The two differ in the table they read and in how a row becomes a value; everything
/// between — the `WHERE` assembly, the parameter offsets, the `OFFSET`/`LIMIT` binding —
/// is the same, and was duplicated verbatim before it had two call sites to justify
/// pulling it out.
/// Builds the paged `SELECT`, with the two pagination placeholders numbered after the
/// filter's own parameters.
///
/// Split out because it is the only part of `paged_select` a test can reach without a
/// server, and the placeholder arithmetic is exactly where an off-by-one hides: `LIMIT
/// $n` instead of `$n+1` silently reuses the offset as the limit.
fn page_sql(table: &str, where_full: &str, order_by: &str, filter_params: usize) -> String {
    let offset_placeholder = filter_params + 1;
    format!(
        "SELECT * FROM {}{}{} OFFSET ${} LIMIT ${}",
        table,
        where_full,
        order_by,
        offset_placeholder,
        offset_placeholder + 1
    )
}

struct PagedSelect<'a> {
    table: &'a str,
    where_sql: &'a str,
    params: Vec<Box<dyn ToSql + Sync + Send>>,
    order_by: &'a str,
    offset: i64,
    limit: i64,
}

async fn paged_select<T, P, F>(
    pool: &P,
    select: PagedSelect<'_>,
    decode: F,
) -> Result<Paged<T>, CqrsError>
where
    P: PgPool,
    F: Fn(&tokio_postgres::Row) -> Result<T, CqrsError>,
{
    let PagedSelect {
        table,
        where_sql,
        params,
        order_by,
        offset,
        limit,
    } = select;

    let where_full = if where_sql.trim().is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", where_sql)
    };

    let conn = pool.acquire().await?;

    let count_sql = format!(
        "SELECT COUNT(*)::BIGINT AS total FROM {}{}",
        table, where_full
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

    let select_sql = page_sql(table, &where_full, order_by, params.len());
    let mut select_params = params;
    select_params.push(Box::new(offset));
    select_params.push(Box::new(limit));
    let select_params_ref: Vec<&(dyn ToSql + Sync)> = select_params
        .iter()
        .map(|b| b.as_ref() as &(dyn ToSql + Sync))
        .collect();
    let rows = conn
        .client()
        .query(&select_sql, &select_params_ref)
        .await
        .map_err(map_pg_error)?;

    let mut items: Vec<T> = Vec::with_capacity(rows.len());
    for row in rows {
        items.push(decode(&row)?);
    }
    Ok(Paged::new(items, total, offset, limit))
}

/// Rebuilds a view from a `(id, data)` row.
///
/// `save` strips `V::field_id()` out of the `data` payload — the id lives in its own
/// column, which is the primary key — so the JSON on disk is missing the field the type
/// declares. Putting it back is what makes the round-trip work; without it every read of
/// every view fails with `missing field \`id\``.
///
/// Reinstating it here rather than keeping it in `data` also means rows written before
/// this was fixed read back correctly.
fn row_to_view<V>(id: String, mut data: JsonValue) -> Result<V, CqrsError>
where
    V: DeserializeOwned + HasId,
{
    if let Some(obj) = data.as_object_mut() {
        obj.insert(V::field_id().to_string(), JsonValue::String(id));
    }
    serde_json::from_value(data).map_err(CqrsError::serialization_error)
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
        let (mut where_sql, mut params) = compile_where(query, &self.mapper)?;

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

        let pagination = query.pagination().unwrap_or_default();
        let limit_v = pagination.limit.unwrap_or(20);
        let offset_v = pagination.skip.unwrap_or(0);

        let sort = query.sort();
        warn_if_page_order_undefined(&self.type_name, offset_v, sort.as_deref());
        let order_by = order_by_clause(sort, &self.mapper)?;

        paged_select(
            &self.pool,
            PagedSelect {
                table: &self.table_name,
                where_sql: &where_sql,
                params,
                order_by: &order_by,
                offset: offset_v,
                limit: limit_v,
            },
            |row| {
                let id: String = row.try_get("id").map_err(map_pg_error)?;
                let val: JsonValue = row.try_get::<_, JsonValue>("data").map_err(map_pg_error)?;
                row_to_view(id, val)
            },
        )
        .await
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
        let sql = format!("SELECT id, data FROM {} WHERE {}", self.table_name, where_sql);
        let conn = self.pool.acquire().await?;
        let row = conn
            .client()
            .query_opt(&sql, &params)
            .await
            .map_err(map_pg_error)?;
        if let Some(row) = row {
            let row_id: String = row.try_get("id").map_err(map_pg_error)?;
            let val: JsonValue = row.try_get::<_, JsonValue>("data").map_err(map_pg_error)?;
            Ok(Some(row_to_view(row_id, val)?))
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

/// Rejected when a caller hands a snapshot storage a parent id.
///
/// The other two backends say the same thing; a caller swapping backends should not have
/// to learn a second phrasing for the same refusal.
const NO_PARENT_ON_SNAPSHOT: &str =
    "a snapshot table has no parent column, so a parent id cannot be filtered on";

/// Read-side storage over the event store's **snapshot** table.
///
/// This does not reuse [`PostgresStorage`], and the reason is the schema. A view table is
/// `(id, parent_id, data)` with `data` holding the serialized view; the snapshot table is
/// `(aggregate_id, data, version)` with `data` holding the **bare aggregate** —
/// `PostgresPersist::save_snapshot` writes `serde_json::to_value(aggregate)`, and
/// `fetch_snapshot` rebuilds the `Snapshot` wrapper from the row's own columns. Pointing
/// the view storage at that table asked for a column named `id` and deserialized `data`
/// into a `Snapshot<A>` that was never written there, so every read failed. See #10.
///
/// Reading is therefore direct: `data` is an `A`, the key column is `aggregate_id`, and
/// there is no parent. Writing stays unsupported — the event store owns this table.
#[derive(Debug, Clone)]
pub struct PostgresFromSnapshotStorage<A, Q, M = JsonbDataMapper, P = SharedClient> {
    _phantom: PhantomData<(A, Q)>,
    pool: P,
    snapshot_table: String,
    mapper: M,
}

impl<A, Q> PostgresFromSnapshotStorage<A, Q, JsonbDataMapper, SharedClient> {
    /// `snapshot_table` is what `PostgresPersist::snapshot_table_name()` returns.
    #[must_use]
    pub fn new(client: Arc<Client>, snapshot_table: &str) -> Self {
        Self::with_pool_and_mapper(SharedClient(client), snapshot_table, JsonbDataMapper)
    }
}

impl<A, Q, M> PostgresFromSnapshotStorage<A, Q, M, SharedClient> {
    #[must_use]
    pub fn with_mapper(client: Arc<Client>, snapshot_table: &str, mapper: M) -> Self {
        Self::with_pool_and_mapper(SharedClient(client), snapshot_table, mapper)
    }
}

impl<A, Q, P> PostgresFromSnapshotStorage<A, Q, JsonbDataMapper, P> {
    #[must_use]
    pub fn with_pool(pool: P, snapshot_table: &str) -> Self {
        Self::with_pool_and_mapper(pool, snapshot_table, JsonbDataMapper)
    }
}

impl<A, Q, M, P> PostgresFromSnapshotStorage<A, Q, M, P> {
    #[must_use]
    pub fn with_pool_and_mapper(pool: P, snapshot_table: &str, mapper: M) -> Self {
        Self {
            _phantom: PhantomData,
            pool,
            snapshot_table: snapshot_table.to_string(),
            mapper,
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
        _context: CqrsContext,
    ) -> Result<Paged<A>, CqrsError> {
        if parent_id.is_some() {
            return Err(CqrsError::validation(NO_PARENT_ON_SNAPSHOT));
        }

        let (where_sql, params) = compile_where(&query, &self.mapper)?;

        let pagination = query.pagination().unwrap_or_default();
        let limit_v = pagination.limit.unwrap_or(20);
        let offset_v = pagination.skip.unwrap_or(0);

        let sort = query.sort();
        warn_if_page_order_undefined(A::TYPE, offset_v, sort.as_deref());
        let order_by = order_by_clause(sort, &self.mapper)?;

        paged_select(
            &self.pool,
            PagedSelect {
                table: &self.snapshot_table,
                where_sql: &where_sql,
                params,
                order_by: &order_by,
                offset: offset_v,
                limit: limit_v,
            },
            |row| {
                // `data` is the aggregate itself, not a `Snapshot` wrapper.
                let val: JsonValue = row.try_get::<_, JsonValue>("data").map_err(map_pg_error)?;
                serde_json::from_value(val).map_err(CqrsError::serialization_error)
            },
        )
        .await
    }

    async fn find_by_id(
        &self,
        parent_id: Option<String>,
        id: &str,
        _context: CqrsContext,
    ) -> Result<Option<A>, CqrsError> {
        if parent_id.is_some() {
            return Err(CqrsError::validation(NO_PARENT_ON_SNAPSHOT));
        }

        // `aggregate_id`, not `id`: that is the snapshot table's primary key.
        let sql = format!(
            "SELECT data FROM {} WHERE aggregate_id = $1",
            self.snapshot_table
        );
        let conn = self.pool.acquire().await?;
        let row = conn
            .client()
            .query_opt(&sql, &[&id])
            .await
            .map_err(map_pg_error)?;

        match row {
            Some(row) => {
                let val: JsonValue = row.try_get::<_, JsonValue>("data").map_err(map_pg_error)?;
                Ok(Some(
                    serde_json::from_value(val).map_err(CqrsError::serialization_error)?,
                ))
            }
            None => Ok(None),
        }
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
    use crate::log_capture::{containing, events_of_async};
    use crate::read::sorter::{SortDirection, Sorter};
    use crate::read::Pagination;
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
    fn the_page_placeholders_follow_the_filter_parameters() {
        assert_eq!(
            page_sql("articles", "", "", 0),
            "SELECT * FROM articles OFFSET $1 LIMIT $2",
            "with no filter the window binds $1 and $2"
        );
        assert_eq!(
            page_sql("articles", " WHERE data->>'a' = $1", " ORDER BY id ASC", 1),
            "SELECT * FROM articles WHERE data->>'a' = $1 ORDER BY id ASC OFFSET $2 LIMIT $3",
            "one filter parameter pushes the window to $2 and $3"
        );
        assert_eq!(
            page_sql("t", "", "", 3),
            "SELECT * FROM t OFFSET $4 LIMIT $5",
            "the limit is always one past the offset — reusing the offset would silently \
             page by the wrong amount"
        );
    }

    /// Pure, and therefore the part `cargo mutants` can reach: the integration tests in
    /// `tests/snapshot_read_path.rs` skip without a server, so they assert nothing under
    /// mutation.
    #[test]
    fn the_jsonb_mapper_addresses_the_data_column() {
        assert_eq!(JsonbDataMapper.map("name"), "data->>'name'");
        assert_eq!(JsonbDataMapper.map("created_at"), "data->>'created_at'");
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    struct TitleQuery {
        title: Option<String>,
    }

    impl Query for TitleQuery {}

    #[test]
    fn compile_where_maps_the_field_and_binds_the_value() {
        let (sql, params) = compile_where(
            &TitleQuery {
                title: Some("Catan".into()),
            },
            &JsonbDataMapper,
        )
        .expect("a valid filter compiles");

        assert!(
            sql.contains("data->>'title'"),
            "the mapper has to reach the SQL, got: {sql}"
        );
        assert_eq!(params.len(), 1, "the value is bound, not interpolated");
    }

    #[test]
    fn compile_where_is_empty_when_the_query_filters_nothing() {
        let (sql, params) =
            compile_where(&TitleQuery::default(), &JsonbDataMapper).expect("no filter");
        assert!(sql.is_empty(), "got: {sql}");
        assert!(params.is_empty());
    }

    /// `build_filter` is `compile_where` plus the parent clause, and the parent half is
    /// what this pins.
    #[test]
    fn build_filter_rejects_a_view_that_needs_a_parent_without_one() {
        #[derive(Debug, Clone, Default, Serialize, Deserialize)]
        struct Child {
            id: String,
        }

        impl HasId for Child {
            fn field_id() -> &'static str {
                "id"
            }
            fn id(&self) -> &str {
                &self.id
            }
            fn parent_field_id() -> Option<&'static str> {
                Some("parent_id")
            }
            fn parent_id(&self) -> Option<&str> {
                None
            }
        }

        let storage: PostgresStorage<Child, TitleQuery, IdentityMapper, FailingPool> =
            PostgresStorage::with_pool(FailingPool, "child", "children");

        let err = storage
            .build_filter(&TitleQuery::default(), &None)
            .expect_err("a child view needs its parent id");
        assert_eq!(err.code, "GENERIC_VALIDATION_FAILED");

        let (sql, params) = storage
            .build_filter(&TitleQuery::default(), &Some("p1".into()))
            .expect("with the parent it compiles");
        assert_eq!(sql, "parent_id = $1");
        assert_eq!(params.len(), 1);
    }

    /// A query whose sort is whatever the caller asked for — the untrusted path.
    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    struct SortedQuery(Vec<Sorter>);

    impl Query for SortedQuery {
        fn sort(&self) -> Option<Vec<Sorter>> {
            Some(self.0.clone())
        }
    }

    fn asc(field: &str) -> Sorter {
        Sorter {
            field: field.to_string(),
            direction: SortDirection::Asc,
        }
    }

    /// A query that asks for a later page and declares no sort — the shape #5 is about.
    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    struct SecondPageQuery;

    impl Query for SecondPageQuery {
        fn pagination(&self) -> Option<Pagination> {
            Some(Pagination {
                skip: Some(20),
                limit: Some(20),
            })
        }
    }

    /// The warning is wired into `filter`, not merely available: delete the call and
    /// this fails. No server needed — the warn happens before `pool.acquire()`.
    #[tokio::test]
    async fn filter_warns_when_paging_without_a_sort() {
        // A view name of its own: the warning is once-per-view and process-global.
        let storage: PostgresStorage<Article, SecondPageQuery, IdentityMapper, FailingPool> =
            PostgresStorage::with_pool(FailingPool, "pg_unsorted_view", "articles");

        let events = events_of_async(async {
            let _ = storage
                .filter(None, SecondPageQuery, CqrsContext::default())
                .await;
        })
        .await;

        let ours = containing(&events, "no sort in effect");
        assert_eq!(ours.len(), 1, "exactly one warning, got {events:?}");
        assert!(ours[0].starts_with("WARN "), "{}", ours[0]);
        assert!(
            ours[0].contains("type_name=pg_unsorted_view"),
            "{}",
            ours[0]
        );
        assert!(ours[0].contains("skip=20"), "{}", ours[0]);
    }

    /// And it stays quiet when the view declares a sort — the remedy actually works.
    #[tokio::test]
    async fn filter_is_quiet_when_the_view_declares_a_sort() {
        let storage: PostgresStorage<Article, SortedQuery, IdentityMapper, FailingPool> =
            PostgresStorage::with_pool(FailingPool, "pg_sorted_view", "articles");

        let events = events_of_async(async {
            let _ = storage
                .filter(
                    None,
                    SortedQuery(vec![asc("created_at")]),
                    CqrsContext::default(),
                )
                .await;
        })
        .await;

        assert!(
            containing(&events, "no sort in effect").is_empty(),
            "got {events:?}"
        );
    }

    /// End-to-end through `Storage::filter`, no server: the sort is rejected *before*
    /// `pool.acquire()` runs, so the error is the validation one and not the pool's.
    /// That is what "fails closed" means here — a bad sort never opens a connection.
    #[tokio::test]
    async fn filter_rejects_a_hostile_sort_before_touching_the_database() {
        let storage: PostgresStorage<Article, SortedQuery, IdentityMapper, FailingPool> =
            PostgresStorage::with_pool(FailingPool, "article", "articles");

        let err = storage
            .filter(
                None,
                SortedQuery(vec![asc("1 UNION ALL SELECT data FROM secrets--")]),
                CqrsContext::default(),
            )
            .await
            .unwrap_err();

        assert_eq!(err.code, "GENERIC_VALIDATION_FAILED");
        assert!(
            !err.message.contains("pool exhausted"),
            "the sort must be rejected before the pool is asked for a connection"
        );
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
