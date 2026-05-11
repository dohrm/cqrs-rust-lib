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

#[derive(Debug, Clone)]
pub struct PostgresStorage<V, Q, M = IdentityMapper> {
    _phantom: PhantomData<(V, Q)>,
    client: Arc<Client>,
    type_name: String,
    table_name: String,
    mapper: M,
}

impl<V, Q> PostgresStorage<V, Q, IdentityMapper> {
    #[must_use]
    pub fn new(client: Arc<Client>, type_name: &str, table_name: &str) -> Self {
        Self::with_mapper(client, type_name, table_name, IdentityMapper)
    }
}

impl<V, Q, M> PostgresStorage<V, Q, M>
where
    M: FieldMapper + Debug + Clone + Send + Sync,
{
    #[must_use]
    pub fn with_mapper(client: Arc<Client>, type_name: &str, table_name: &str, mapper: M) -> Self {
        Self {
            _phantom: PhantomData,
            client,
            type_name: type_name.to_string(),
            table_name: table_name.to_string(),
            mapper,
        }
    }
}

impl<V, Q, M> PostgresStorage<V, Q, M>
where
    V: HasId,
    Q: Query,
    M: FieldMapper + Debug + Clone + Send + Sync,
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
impl<V, Q, M> Storage<V, Q> for PostgresStorage<V, Q, M>
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

        let count_sql = format!(
            "SELECT COUNT(*)::BIGINT AS total FROM {}{}",
            self.table_name, where_full
        );
        let count_params: Vec<&(dyn ToSql + Sync)> = params
            .iter()
            .map(|b| b.as_ref() as &(dyn ToSql + Sync))
            .collect();
        let row = self
            .client
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
        let rows = self
            .client
            .query(&select_sql, &select_params_ref)
            .await
            .map_err(map_pg_error)?;
        let mut items: Vec<V> = Vec::with_capacity(rows.len());
        for row in rows {
            let val: JsonValue = row.try_get::<_, JsonValue>("data").map_err(map_pg_error)?;
            let v: V = serde_json::from_value(val).map_err(CqrsError::serialization_error)?;
            items.push(v);
        }
        Ok(Paged {
            items,
            total,
            page_size: limit_v,
            page: if limit_v > 0 { (offset_v / limit_v).abs() } else { 0 },
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
        self.client
            .execute(&sql, &[&id, &parent_id, &data_obj])
            .await
            .map_err(map_pg_error)?;
        Ok(())
    }
}
}

#[derive(Debug, Clone)]
pub struct PostgresFromSnapshotStorage<A, Q, M = IdentityMapper>
where
    A: Aggregate,
    Q: Debug + Clone + DeserializeOwned + Send + Sync + Query,
    M: FieldMapper + Debug + Clone + Send + Sync,
{
    _phantom: PhantomData<A>,
    inner: Arc<PostgresStorage<Snapshot<A>, Q, M>>,
}

impl<A, Q> PostgresFromSnapshotStorage<A, Q, IdentityMapper>
where
    A: Aggregate,
    Q: Debug + Clone + DeserializeOwned + Send + Sync + Query,
{
    #[must_use]
    pub fn new(inner: Arc<PostgresStorage<Snapshot<A>, Q, IdentityMapper>>) -> Self {
        Self {
            _phantom: PhantomData,
            inner,
        }
    }
}

impl<A, Q, M> PostgresFromSnapshotStorage<A, Q, M>
where
    A: Aggregate,
    Q: Debug + Clone + DeserializeOwned + Send + Sync + Query,
    M: FieldMapper + Debug + Clone + Send + Sync,
{
    #[must_use]
    pub fn with_mapper(inner: Arc<PostgresStorage<Snapshot<A>, Q, M>>) -> Self {
        Self {
            _phantom: PhantomData,
            inner,
        }
    }
}

cqrs_async_trait! {
impl<A, Q, M> Storage<A, Q> for PostgresFromSnapshotStorage<A, Q, M>
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
        Err(CqrsError::database_error(StorageError::UnsupportedMethod(
            "SnapshotStorage#save".to_string(),
        )))
    }
}
}
