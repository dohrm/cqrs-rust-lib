use crate::read::storage::{HasId, Storage, StorageError};
use crate::read::Paged;
use crate::{Aggregate, CqrsContext, CqrsError, Snapshot};
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value as JsonValue;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::sync::Arc;

#[cfg(feature = "postgres")]
use tokio_postgres::{types::ToSql, Client};

fn map_pg_error<E: std::error::Error + Send + Sync + 'static>(e: E) -> CqrsError {
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

/// QueryBuilder for PostgreSQL that can turn a query into SQL fragments.
pub trait QueryBuilder<Q>: Debug + Clone + Send + Sync {
    /// Returns (where_sql, params), where where_sql does not include the "WHERE" keyword.
    fn to_where(
        &self,
        query: &Q,
        context: &CqrsContext,
    ) -> (String, Vec<Box<dyn ToSql + Sync + Send>>);
    /// Returns ORDER BY clause without the keyword (e.g., "created_at DESC").
    fn to_order_by(&self, query: &Q, context: &CqrsContext) -> Option<String>;
    fn to_skip_limit(&self, query: &Q, context: &CqrsContext) -> SkipLimit;
}

#[derive(Debug, Clone)]
pub struct PostgresStorage<V, Q, QB> {
    _phantom: PhantomData<(V, Q)>,
    client: Arc<Client>,
    type_name: String,
    table_name: String,
    query_builder: QB,
}

impl<V, Q, QB> PostgresStorage<V, Q, QB>
where
    V: Debug + Clone + Default + Serialize + DeserializeOwned + Send + Sync + HasId,
    Q: Debug + Clone + DeserializeOwned + Send,
    QB: QueryBuilder<Q>,
{
    #[must_use]
    pub fn new(client: Arc<Client>, type_name: &str, query_builder: QB, table_name: &str) -> Self {
        Self {
            _phantom: PhantomData,
            client,
            type_name: type_name.to_string(),
            table_name: table_name.to_string(),
            query_builder,
        }
    }

    fn build_parent_clause(
        &self,
        parent_id: &Option<String>,
        params: &mut Vec<Box<dyn ToSql + Sync + Send>>,
    ) -> Result<Option<String>, CqrsError> {
        match (V::parent_field_id(), parent_id) {
            (Some(_), Some(pid)) => {
                params.push(Box::new(pid.clone()));
                Ok(Some(format!("parent_id = ${}", params.len())))
            }
            (Some(_), None) => Err(CqrsError::validation(
                StorageError::MissingParentId.to_string(),
            )),
            _ => Ok(None),
        }
    }
}

#[async_trait::async_trait]
impl<V, Q, QB> Storage<V, Q> for PostgresStorage<V, Q, QB>
where
    V: Debug + Clone + Default + Serialize + DeserializeOwned + Send + Sync + HasId,
    Q: Clone + Debug + DeserializeOwned + Send + Sync,
    QB: QueryBuilder<Q> + Send + Sync,
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
        let (mut where_sql, mut params) = self.query_builder.to_where(&query, &context);
        let parent_clause = self.build_parent_clause(&parent_id, &mut params)?;
        if let Some(pc) = parent_clause {
            if where_sql.trim().is_empty() {
                where_sql = pc;
            } else {
                where_sql = format!("({}) AND {}", where_sql, pc);
            }
        }
        let where_full = if where_sql.trim().is_empty() {
            String::new()
        } else {
            format!(" WHERE {}", where_sql)
        };

        let SkipLimit { skip, limit } = self.query_builder.to_skip_limit(&query, &context);
        let order_by = self
            .query_builder
            .to_order_by(&query, &context)
            .map(|s| format!(" ORDER BY {}", s))
            .unwrap_or_default();
        let limit_v = limit.unwrap_or(20);
        let offset_v = skip.unwrap_or(0);
        let owned_params = params; // keep ownership for boxing

        // total count
        let count_sql = format!(
            "SELECT COUNT(*)::BIGINT AS total FROM {}{}",
            self.table_name, where_full
        );
        let count_params: Vec<&(dyn ToSql + Sync)> = owned_params
            .iter()
            .map(|b| b.as_ref() as &(dyn ToSql + Sync))
            .collect();
        let row = self
            .client
            .query_one(&count_sql, &count_params)
            .await
            .map_err(map_pg_error)?;
        let total: i64 = row.try_get::<_, i64>("total").map_err(map_pg_error)?;

        // page query
        let param_offset = owned_params.len() + 1;
        let select_sql = format!(
            "SELECT data FROM {}{}{} OFFSET ${} LIMIT ${}",
            self.table_name, where_full, order_by, param_offset, param_offset + 1
        );
        let mut select_params: Vec<Box<dyn ToSql + Sync + Send>> = owned_params
            .into_iter()
            .collect();
        select_params.push(Box::new(offset_v));
        select_params.push(Box::new(limit_v));
        let select_params_ref: Vec<&(dyn ToSql + Sync)> = select_params
            .iter()
            .map(|b| b.as_ref() as &(dyn ToSql + Sync))
            .collect();
        let rows = self
            .client
            .query(&select_sql, &select_params_ref)
            .await
            .map_err(map_pg_error)?;
        let mut items: Vec<V> = Vec::with_capacity(rows.len());
        for row in rows {
            let val: JsonValue = row.try_get::<_, JsonValue>("data").map_err(map_pg_error)?;
            let v: V = serde_json::from_value(val)
                .map_err(|e| CqrsError::serialization_error(e))?;
            items.push(v);
        }
        Ok(Paged {
            items,
            total,
            page_size: limit_v,
            page: if limit_v > 0 {
                (offset_v / limit_v).abs()
            } else {
                0
            },
        })
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
        let row = self
            .client
            .query_opt(&sql, &params)
            .await
            .map_err(map_pg_error)?;
        if let Some(row) = row {
            let val: JsonValue = row.try_get::<_, JsonValue>("data").map_err(map_pg_error)?;
            let v: V = serde_json::from_value(val)
                .map_err(|e| CqrsError::serialization_error(e))?;
            Ok(Some(v))
        } else {
            Ok(None)
        }
    }

    async fn save(&self, entity: V, _context: CqrsContext) -> Result<(), CqrsError> {
        let id = entity.id().to_string();
        let parent_id = entity.parent_id().map(|s| s.to_string());
        let data = serde_json::to_value(&entity)
            .map_err(|e| CqrsError::serialization_error(e))?;
        // Remove id key from data if exists (to keep canonical form in data column)
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
        self.client
            .execute(&sql, &[&id, &parent_id, &data_obj])
            .await
            .map_err(map_pg_error)?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct PostgresFromSnapshotStorage<A, Q, QB>
where
    A: Aggregate,
    Q: Debug + Clone + DeserializeOwned + Send + Sync,
    QB: QueryBuilder<Q>,
{
    _phantom: PhantomData<(A, Q, QB)>,
    inner: Arc<PostgresStorage<Snapshot<A>, Q, QB>>,
}

impl<A, Q, QB> PostgresFromSnapshotStorage<A, Q, QB>
where
    A: Aggregate,
    Q: Debug + Clone + DeserializeOwned + Send + Sync,
    QB: QueryBuilder<Q>,
{
    #[must_use]
    pub fn new(inner: Arc<PostgresStorage<Snapshot<A>, Q, QB>>) -> Self {
        Self {
            _phantom: PhantomData,
            inner,
        }
    }
}

#[async_trait::async_trait]
impl<A, Q, QB> Storage<A, Q> for PostgresFromSnapshotStorage<A, Q, QB>
where
    A: Aggregate,
    Q: Debug + Clone + DeserializeOwned + Send + Sync,
    QB: QueryBuilder<Q>,
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
            items: result.items.iter().map(|s| s.state.clone()).collect(),
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
        Err(CqrsError::database_error(
            StorageError::UnsupportedMethod("SnapshotStorage#save".to_string()),
        ))
    }
}
