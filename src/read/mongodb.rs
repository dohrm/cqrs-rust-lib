use crate::read::query::{Pagination, Query};
use crate::read::sorter::{SortDirection, Sorter};
use crate::read::storage::{HasId, Storage, StorageError};
use crate::read::Paged;
use crate::{Aggregate, CqrsContext, CqrsError, Snapshot};
use futures::TryStreamExt;
use mongodb::bson::{doc, serialize_to_document, Bson, Document};
use mongodb::Database;
use rest_sql::{FieldMapper, IdentityMapper};
use rest_sql_drivers::mongodb::MongoCompiler;
use rest_sql_drivers::Driver;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fmt::Debug;
use std::marker::PhantomData;
use std::sync::Arc;

fn map_mongo_error(e: mongodb::error::Error) -> CqrsError {
    CqrsError::database_error(e)
}

fn map_bson_error(e: mongodb::bson::error::Error) -> CqrsError {
    CqrsError::database_error(e)
}

fn sorters_to_mongo_sort(sort: Option<Vec<Sorter>>, mapper: &impl FieldMapper) -> Option<Document> {
    let sorters = match sort {
        Some(s) if !s.is_empty() => s,
        _ => return None,
    };
    let mut doc = Document::new();
    for s in &sorters {
        let field = mapper.map(&s.field).into_owned();
        let dir: i32 = match s.direction {
            SortDirection::Asc => 1,
            SortDirection::Desc => -1,
        };
        doc.insert(field, Bson::Int32(dir));
    }
    Some(doc)
}

#[derive(Debug, Clone)]
pub struct MongoDbStorage<V, Q, M = IdentityMapper> {
    _phantom: PhantomData<(V, Q)>,
    database: Database,
    type_name: String,
    collection_name: String,
    mapper: M,
}

impl<V, Q> MongoDbStorage<V, Q, IdentityMapper> {
    #[must_use]
    pub fn new(database: Database, type_name: &str, collection_name: &str) -> Self {
        Self::with_mapper(database, type_name, collection_name, IdentityMapper)
    }
}

impl<V, Q, M> MongoDbStorage<V, Q, M>
where
    M: FieldMapper + Debug + Clone + Send + Sync,
{
    #[must_use]
    pub fn with_mapper(
        database: Database,
        type_name: &str,
        collection_name: &str,
        mapper: M,
    ) -> Self {
        Self {
            _phantom: PhantomData,
            database,
            type_name: type_name.to_string(),
            collection_name: collection_name.to_string(),
            mapper,
        }
    }

    fn parent_id_query(
        &self,
        base_query: Document,
        parent_id: &Option<String>,
    ) -> Result<Document, CqrsError>
    where
        V: HasId,
    {
        match (V::parent_field_id(), parent_id) {
            (Some(parent_field_id), Some(parent_id)) => {
                let parent_id_query = doc! {parent_field_id: parent_id};
                Ok(doc! { "$and": [base_query, parent_id_query] })
            }
            (Some(_), None) => Err(CqrsError::validation(
                StorageError::MissingParentId.to_string(),
            )),
            _ => Ok(base_query),
        }
    }
}

cqrs_async_trait! {
impl<V, Q, M> Storage<V, Q> for MongoDbStorage<V, Q, M>
where
    V: Debug + Clone + Default + Serialize + DeserializeOwned + Send + Sync + HasId,
    Q: Clone + Debug + Send + Sync + Query,
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
        let collection = self.database.collection::<V>(&self.collection_name);

        let user_filter = match query.filter() {
            Some(rsql) => MongoCompiler::new(self.mapper.clone())
                .compile(&rsql)
                .unwrap_or_default(),
            None => Document::new(),
        };
        let filter_doc = self.parent_id_query(user_filter, &parent_id)?;
        let sort_doc = sorters_to_mongo_sort(query.sort(), &self.mapper);

        let Pagination { skip, limit } = query.pagination().unwrap_or_default();
        let skip_v = skip.unwrap_or(0).max(0) as u64;
        let limit_v = limit.unwrap_or(20);

        let total = collection
            .count_documents(filter_doc.clone())
            .await
            .map_err(map_mongo_error)?;

        let find = collection
            .find(filter_doc.clone())
            .skip(skip_v)
            .limit(limit_v);
        let cursor = (if let Some(sort) = sort_doc {
            find.sort(sort)
        } else {
            find
        })
        .await
        .map_err(map_mongo_error)?;

        let items = cursor.try_collect().await.map_err(map_mongo_error)?;
        Ok(Paged {
            items,
            total: total as i64,
            page_size: limit_v,
            page: if limit_v > 0 { ((skip_v as i64) / limit_v).abs() } else { 0 },
        })
    }

    async fn find_by_id(
        &self,
        parent_id: Option<String>,
        id: &str,
        _context: CqrsContext,
    ) -> Result<Option<V>, CqrsError> {
        let collection = self.database.collection::<V>(&self.collection_name);
        collection
            .find_one(self.parent_id_query(doc! {V::field_id(): id}, &parent_id)?)
            .await
            .map_err(map_mongo_error)
    }

    async fn save(&self, entity: V, _context: CqrsContext) -> Result<(), CqrsError> {
        let collection = self.database.collection::<V>(&self.collection_name);
        let id = doc! {V::field_id(): entity.id()};
        let mut fields = serialize_to_document(&entity).map_err(map_bson_error)?;
        fields.remove(V::field_id());
        collection
            .update_one(
                id,
                doc! {"$set": &fields, "$setOnInsert": doc!{V::field_id(): entity.id()}},
            )
            .upsert(true)
            .await
            .map_err(map_mongo_error)?;
        Ok(())
    }
}
}

#[derive(Debug, Clone)]
pub struct MongoDBFromSnapshotStorage<A, Q, M = IdentityMapper>
where
    A: Aggregate,
    Q: Debug + Clone + Send + Sync + Query,
    M: FieldMapper + Debug + Clone + Send + Sync,
{
    _phantom: PhantomData<A>,
    inner: Arc<MongoDbStorage<Snapshot<A>, Q, M>>,
}

impl<A, Q> MongoDBFromSnapshotStorage<A, Q, IdentityMapper>
where
    A: Aggregate,
    Q: Debug + Clone + Send + Sync + Query,
{
    #[must_use]
    pub fn new(inner: Arc<MongoDbStorage<Snapshot<A>, Q, IdentityMapper>>) -> Self {
        Self {
            _phantom: PhantomData,
            inner,
        }
    }
}

impl<A, Q, M> MongoDBFromSnapshotStorage<A, Q, M>
where
    A: Aggregate,
    Q: Debug + Clone + Send + Sync + Query,
    M: FieldMapper + Debug + Clone + Send + Sync,
{
    #[must_use]
    pub fn with_mapper(inner: Arc<MongoDbStorage<Snapshot<A>, Q, M>>) -> Self {
        Self {
            _phantom: PhantomData,
            inner,
        }
    }
}

cqrs_async_trait! {
impl<A, Q, M> Storage<A, Q> for MongoDBFromSnapshotStorage<A, Q, M>
where
    A: Aggregate,
    Q: Clone + Debug + Send + Sync + Query,
    M: FieldMapper + Debug + Clone + Send + Sync,
{
    fn type_name(&self) -> &str {
        self.inner.type_name()
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
