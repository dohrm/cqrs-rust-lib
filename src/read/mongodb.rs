use crate::read::storage::{HasId, Storage, StorageError};
use crate::read::Paged;
use crate::{Aggregate, AggregateError, CqrsContext, Snapshot};
use futures::TryStreamExt;
use mongodb::bson::{doc, to_bson, Document};
use mongodb::{bson, Database};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::sync::Arc;

fn map_mongo_error(e: mongodb::error::Error) -> AggregateError {
    AggregateError::DatabaseError(e.into())
}

fn map_bson_error(e: bson::ser::Error) -> AggregateError {
    AggregateError::DatabaseError(e.into())
}

pub struct SkipLimit {
    pub skip: Option<u64>,
    pub limit: Option<i64>,
}

impl SkipLimit {
    pub fn new(skip: Option<u64>, limit: Option<i64>) -> Self {
        Self { skip, limit }
    }
}

pub trait QueryBuilder<Q>: Debug + Clone + Send + Sync {
    fn to_query(&self, query: &Q, context: &CqrsContext) -> Document;
    fn to_skip_limit(&self, query: &Q, context: &CqrsContext) -> SkipLimit;
}

#[derive(Debug, Clone)]
pub struct MongoDbStorage<V, Q, QB> {
    _phantom: PhantomData<(V, Q)>,
    database: Database,
    type_name: String,
    collection_name: String,
    query_builder: QB,
}

impl<V, Q, QB> MongoDbStorage<V, Q, QB>
where
    V: Debug + Clone + Default + Serialize + DeserializeOwned + Send + Sync + HasId,
    Q: Debug + Clone + DeserializeOwned + Send,
    QB: QueryBuilder<Q>,
{
    #[must_use]
    pub fn new(
        database: Database,
        type_name: &str,
        query_builder: QB,
        collection_name: &str,
    ) -> Self {
        Self {
            _phantom: PhantomData::default(),
            database,
            type_name: type_name.to_string(),
            collection_name: collection_name.to_string(),
            query_builder,
        }
    }

    fn parent_id_query(
        &self,
        base_query: Document,
        parent_id: &Option<String>,
    ) -> Result<Document, AggregateError> {
        match (V::parent_field_id(), parent_id) {
            (Some(parent_field_id), Some(parent_id)) => {
                let parent_id_query = doc! {parent_field_id: parent_id};
                Ok(doc! { "$and": [base_query, parent_id_query] })
            }
            (Some(_), None) => Err(AggregateError::UserError(Box::new(
                StorageError::MissingParentId,
            ))),
            _ => Ok(base_query),
        }
    }
}

#[async_trait::async_trait]
impl<V, Q, QB> Storage<V, Q> for MongoDbStorage<V, Q, QB>
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
    ) -> Result<Paged<V>, AggregateError> {
        let collection = self.database.collection::<V>(&self.collection_name);
        let q = self.parent_id_query(self.query_builder.to_query(&query, &context), &parent_id)?;
        let SkipLimit { skip, limit } = self.query_builder.to_skip_limit(&query, &context);
        let total = collection
            .count_documents(q.clone())
            .await
            .map_err(map_mongo_error)?;
        let skip = skip.unwrap_or(0u64);
        let limit = limit.unwrap_or(20i64);
        let cursor = collection
            .find(q.clone())
            .skip(skip)
            .limit(limit)
            .await
            .map_err(map_mongo_error)?;

        let items = cursor.try_collect().await.map_err(map_mongo_error)?;
        Ok(Paged {
            items,
            total: total as i64,
            page_size: limit,
            page: ((skip as i64) / limit).abs(),
        })
    }

    async fn find_by_id(
        &self,
        parent_id: Option<String>,
        id: &str,
        _context: CqrsContext,
    ) -> Result<Option<V>, AggregateError> {
        let collection = self.database.collection::<V>(&self.collection_name);
        collection
            .find_one(self.parent_id_query(doc! {V::field_id(): id}, &parent_id)?)
            .await
            .map_err(map_mongo_error)
    }

    async fn save(&self, entity: V, _context: CqrsContext) -> Result<(), AggregateError> {
        let collection = self.database.collection::<V>(&self.collection_name);
        let id = doc! {V::field_id(): entity.id()};
        let e = if let Some(entity) = to_bson(&entity).map_err(map_bson_error)?.as_document_mut() {
            entity.remove(V::field_id());
            entity.clone()
        } else {
            doc! {}
        };
        collection
            .update_one(
                id,
                doc! {"$set": &e, "$setOnInsert": doc!{V::field_id(): entity.id()}},
            )
            .upsert(true)
            .await
            .map_err(map_mongo_error)?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct MongoDBFromSnapshotStorage<A, Q, QB>
where
    A: Aggregate,
    Q: Debug + Clone + DeserializeOwned + Send + Sync,
    QB: QueryBuilder<Q>,
{
    _phantom: PhantomData<(A, Q, QB)>,
    inner: Arc<MongoDbStorage<Snapshot<A>, Q, QB>>,
}

impl<A, Q, QB> MongoDBFromSnapshotStorage<A, Q, QB>
where
    A: Aggregate,
    Q: Debug + Clone + DeserializeOwned + Send + Sync,
    QB: QueryBuilder<Q>,
{
    #[must_use]
    pub fn new(inner: Arc<MongoDbStorage<Snapshot<A>, Q, QB>>) -> Self {
        Self {
            _phantom: PhantomData::default(),
            inner,
        }
    }
}

#[async_trait::async_trait]
impl<A, Q, QB> Storage<A, Q> for MongoDBFromSnapshotStorage<A, Q, QB>
where
    A: Aggregate,
    Q: Debug + Clone + DeserializeOwned + Send + Sync,
    QB: QueryBuilder<Q>,
{
    fn type_name(&self) -> &str {
        self.inner.type_name()
    }

    async fn filter(
        &self,
        parent_id: Option<String>,
        query: Q,
        context: CqrsContext,
    ) -> Result<Paged<A>, AggregateError> {
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
    ) -> Result<Option<A>, AggregateError> {
        Ok(self
            .inner
            .find_by_id(parent_id, id, context)
            .await?
            .map(|s| s.state))
    }

    async fn save(&self, _entity: A, _context: CqrsContext) -> Result<(), AggregateError> {
        Err(AggregateError::DatabaseError(Box::new(
            StorageError::UnsupportedMethod("SnapshotStorage#save".to_string()),
        )))
    }
}
