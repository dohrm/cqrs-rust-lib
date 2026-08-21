use crate::read::page_order::warn_if_page_order_undefined;
use crate::read::query::{Pagination, Query};
use crate::read::sorter::{SortDirection, Sorter};
use crate::read::storage::{HasId, Storage, StorageError};
use crate::read::Paged;
use crate::{Aggregate, CqrsContext, CqrsError, Snapshot};
use futures::TryStreamExt;
use mongodb::bson::{doc, serialize_to_document, Bson, Document};
use mongodb::Database;
use rest_sql::{FieldMapper, IdentityMapper};
use std::borrow::Cow;
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

/// Compiles the sorters into a MongoDB sort document.
///
/// Fallible for the same reason as the SQL backends: a sort key is not a bound
/// parameter, and a `$`-prefixed or dotted key is meaningful to the driver, so
/// [`Sorter::validated_field`] gates every field name.
fn sorters_to_mongo_sort(
    sort: Option<Vec<Sorter>>,
    mapper: &impl FieldMapper,
) -> Result<Option<Document>, CqrsError> {
    let sorters = match sort {
        Some(s) if !s.is_empty() => s,
        _ => return Ok(None),
    };
    let mut doc = Document::new();
    for s in &sorters {
        let field = mapper.map(s.validated_field()?).into_owned();
        let dir: i32 = match s.direction {
            SortDirection::Asc => 1,
            SortDirection::Desc => -1,
        };
        doc.insert(field, Bson::Int32(dir));
    }
    Ok(Some(doc))
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

        // `map_err(...)?`, not `unwrap_or_default()`: an empty `Document` matches
        // *everything*, so a filter that parsed but failed to compile used to return the
        // whole collection with a 200 and nothing saying the filter had been dropped —
        // the same fail-open shape ADR-0001 closes at the HTTP boundary, one layer down.
        // Postgres and SurrealDB already propagate this error; MongoDB was the outlier.
        let user_filter = match query.filter() {
            Some(rsql) => MongoCompiler::new(self.mapper.clone())
                .compile(&rsql)
                .map_err(|e| CqrsError::internal(e.to_string()))?,
            None => Document::new(),
        };
        let filter_doc = self.parent_id_query(user_filter, &parent_id)?;
        let Pagination { skip, limit } = query.pagination().unwrap_or_default();
        let skip_v = skip.unwrap_or(0).max(0);
        let limit_v = limit.unwrap_or(20);

        let sort = query.sort();
        warn_if_page_order_undefined(&self.type_name, skip_v, sort.as_deref());
        let sort_doc = sorters_to_mongo_sort(sort, &self.mapper)?;

        let total = collection
            .count_documents(filter_doc.clone())
            .await
            .map_err(map_mongo_error)?;

        let find = collection
            .find(filter_doc.clone())
            .skip(skip_v as u64)
            .limit(limit_v);
        let cursor = (if let Some(sort) = sort_doc {
            find.sort(sort)
        } else {
            find
        })
        .await
        .map_err(map_mongo_error)?;

        let items = cursor.try_collect().await.map_err(map_mongo_error)?;
        Ok(Paged::new(items, total as i64, skip_v, limit_v))
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

/// Maps a logical field name onto its place inside a snapshot document.
///
/// The snapshot collection stores a whole `Snapshot<A>` — `{_id, state: {…}, version}` —
/// so a filter on `name` has to reach `state.name`. With the default [`IdentityMapper`]
/// it reached `name`, which no document has: the query matched nothing and returned an
/// empty page with `total: 0` and no error at all. See #10.
#[derive(Debug, Clone, Copy, Default)]
pub struct SnapshotStateMapper;

impl FieldMapper for SnapshotStateMapper {
    fn map<'a>(&self, field: &'a str) -> Cow<'a, str> {
        Cow::Owned(format!("state.{field}"))
    }
}

/// Rejected when a caller hands a snapshot storage a parent id. Same wording as the
/// other backends.
const NO_PARENT_ON_SNAPSHOT: &str =
    "a snapshot table has no parent column, so a parent id cannot be filtered on";

/// Read-side storage over the event store's **snapshot** collection.
///
/// Unlike the Postgres and SurrealDB ones, this does reuse [`MongoDbStorage`]: the
/// snapshot document *is* a serialized `Snapshot<A>`, so reading it as one is correct.
/// What was wrong is the mapper — see [`SnapshotStateMapper`]. Writing stays unsupported:
/// the event store owns this collection.
#[derive(Debug, Clone)]
pub struct MongoDBFromSnapshotStorage<A, Q, M = SnapshotStateMapper>
where
    A: Aggregate,
    Q: Debug + Clone + Send + Sync + Query,
    M: FieldMapper + Debug + Clone + Send + Sync,
{
    _phantom: PhantomData<A>,
    inner: Arc<MongoDbStorage<Snapshot<A>, Q, M>>,
}

impl<A, Q> MongoDBFromSnapshotStorage<A, Q, SnapshotStateMapper>
where
    A: Aggregate,
    Q: Debug + Clone + Send + Sync + Query,
{
    /// `snapshot_collection` is what `MongoDBPersist::snapshot_collection_name()` returns.
    #[must_use]
    pub fn new(database: Database, snapshot_collection: &str) -> Self {
        Self::with_mapper(database, snapshot_collection, SnapshotStateMapper)
    }
}

impl<A, Q, M> MongoDBFromSnapshotStorage<A, Q, M>
where
    A: Aggregate,
    Q: Debug + Clone + Send + Sync + Query,
    M: FieldMapper + Debug + Clone + Send + Sync,
{
    #[must_use]
    pub fn with_mapper(database: Database, snapshot_collection: &str, mapper: M) -> Self {
        Self {
            _phantom: PhantomData,
            inner: Arc::new(MongoDbStorage::with_mapper(
                database,
                A::TYPE,
                snapshot_collection,
                mapper,
            )),
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
        if parent_id.is_some() {
            return Err(CqrsError::validation(NO_PARENT_ON_SNAPSHOT));
        }
        let result = self.inner.filter(parent_id, query, context).await?;
        Ok(result.map(|s| s.state))
    }

    async fn find_by_id(
        &self,
        parent_id: Option<String>,
        id: &str,
        context: CqrsContext,
    ) -> Result<Option<A>, CqrsError> {
        if parent_id.is_some() {
            return Err(CqrsError::validation(NO_PARENT_ON_SNAPSHOT));
        }
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
    use crate::log_capture::{containing, events_of_async};
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

    fn asc(field: &str) -> Sorter {
        Sorter {
            field: field.to_string(),
            direction: SortDirection::Asc,
        }
    }

    /// Pure, and therefore reachable by `cargo mutants`: the integration test skips
    /// without a server and asserts nothing under mutation.
    #[test]
    fn the_snapshot_mapper_addresses_the_state_subdocument() {
        assert_eq!(SnapshotStateMapper.map("name"), "state.name");
        assert_eq!(SnapshotStateMapper.map("counter"), "state.counter");
    }

    /// No server needed: the sort document is built before the driver is reached.
    #[test]
    fn a_valid_sort_compiles_to_the_expected_document() {
        let sorters = vec![
            asc("score"),
            Sorter {
                field: "muscle.primary".into(),
                direction: SortDirection::Desc,
            },
        ];
        assert_eq!(
            sorters_to_mongo_sort(Some(sorters), &IdentityMapper).unwrap(),
            Some(doc! { "score": 1i32, "muscle.primary": -1i32 })
        );
    }

    #[test]
    fn no_sort_compiles_to_no_document() {
        assert_eq!(
            sorters_to_mongo_sort(None, &IdentityMapper).unwrap(),
            None,
            "no sort must stay no sort, not an empty document"
        );
        assert_eq!(
            sorters_to_mongo_sort(Some(vec![]), &IdentityMapper).unwrap(),
            None
        );
    }

    /// A sort key is not a bound parameter here either: it is a document key the
    /// driver interprets, so the same validation applies as on the SQL backends.
    #[test]
    fn a_hostile_sort_field_is_rejected() {
        let hostile = "1 UNION ALL SELECT data FROM secrets--";
        let err = sorters_to_mongo_sort(Some(vec![asc(hostile)]), &IdentityMapper).unwrap_err();
        assert_eq!(err.code, "GENERIC_VALIDATION_FAILED");
        assert!(err.message.contains(hostile));
    }

    #[test]
    fn an_operator_prefixed_sort_field_is_rejected() {
        let err = sorters_to_mongo_sort(Some(vec![asc("$natural")]), &IdentityMapper).unwrap_err();
        assert_eq!(err.code, "GENERIC_VALIDATION_FAILED");
        assert!(err.message.contains("$natural"));
    }

    /// A storage pointed at a server that is not there. `Client::with_uri_str` does no
    /// I/O — it only parses the URI and starts a background topology monitor — so
    /// `filter` reaches the warning and then fails on the first command. That is enough
    /// to pin the call: delete it in `filter` and this test fails, with no server.
    async fn unreachable_storage<Q>(type_name: &str) -> MongoDbStorage<Article, Q> {
        let client = mongodb::Client::with_uri_str(
            "mongodb://127.0.0.1:1/?serverSelectionTimeoutMS=50&connectTimeoutMS=50",
        )
        .await
        .expect("the URI parses; nothing connects yet");
        MongoDbStorage::new(client.database("test"), type_name, "articles")
    }

    /// A query asking for a later page with no sort declared.
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

    /// A `_q` that parses but does not compile used to become `Document::new()` — which
    /// matches every document — so the caller received the whole collection with a `200`
    /// and no sign the filter had been dropped. `Like` against a non-string is the case
    /// rest-sql-drivers rejects, and it now surfaces as an error like it does on the
    /// other two backends.
    #[tokio::test]
    async fn a_filter_that_fails_to_compile_is_an_error_not_an_empty_document() {
        use rest_sql::{Ast, Constraint, Operator, RestSql, Value};

        #[derive(Debug, Clone, Default, Serialize, Deserialize)]
        struct UncompilableQuery;

        impl Query for UncompilableQuery {
            fn filter(&self) -> Option<RestSql> {
                // Parses and validates; the MongoDB compiler rejects it.
                RestSql::from_ast(Ast::Constraint(Constraint {
                    field: "title".into(),
                    operator: Operator::Like,
                    value: Value::Int(1),
                }))
                .ok()
            }
        }

        let storage = unreachable_storage::<UncompilableQuery>("mongo_uncompilable").await;
        let err = storage
            .filter(None, UncompilableQuery, CqrsContext::default())
            .await
            .expect_err("an uncompilable filter must not silently match everything");

        assert!(
            err.message.to_lowercase().contains("like"),
            "the error must say which operator it could not compile, got: {}",
            err.message
        );
    }

    #[tokio::test]
    async fn filter_warns_when_paging_without_a_sort() {
        // A view name of its own: the warning is once-per-view and process-global.
        let storage = unreachable_storage::<SecondPageQuery>("mongo_unsorted_view").await;

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
            ours[0].contains("type_name=mongo_unsorted_view"),
            "{}",
            ours[0]
        );
        assert!(ours[0].contains("skip=20"), "{}", ours[0]);
    }
}
