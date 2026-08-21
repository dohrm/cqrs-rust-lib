//! The read paths that #10 found broken, one test per backend.
//!
//! A snapshot row is written by the event store and read by a `FromSnapshotStorage`, and
//! the two disagreed on every backend: Postgres and SurrealDB answered 500, MongoDB
//! returned an empty page with no error. The Postgres *view* round-trip was broken too,
//! independently.
//!
//! The whole file needs `postgres` (it is the only backend whose *view* round-trip is
//! covered here); each test additionally needs its own feature. Postgres and MongoDB read
//! `PG_TEST_URI` / `MONGODB_TEST_URI` and skip without them — `just db-up && just test-db`
//! sets both — while SurrealDB runs against the in-memory engine and needs no server.
#![cfg(feature = "postgres")]

use cqrs_rust_lib::read::postgres::{PostgresFromSnapshotStorage, PostgresStorage};
use cqrs_rust_lib::read::storage::{HasId, Storage};
use cqrs_rust_lib::read::Query;
use cqrs_rust_lib::CqrsContext;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio_postgres::{Client, NoTls};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
struct Article {
    id: String,
    title: String,
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct CounterQuery {
    counter: Option<i32>,
}
impl Query for CounterQuery {}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct NameQuery {
    name: Option<String>,
}
impl Query for NameQuery {}

// A minimal aggregate: `cqrs_rust_lib::testing` is `#[cfg(test)]`, so an integration
// test cannot see it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
struct Counter {
    id: String,
    counter: i32,
    name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(utoipa::ToSchema))]
enum CounterEvent {
    Created,
}

impl cqrs_rust_lib::Event for CounterEvent {
    fn event_type(&self) -> String {
        "Created".to_string()
    }
}

cqrs_rust_lib::cqrs_async_trait! {
impl cqrs_rust_lib::Aggregate for Counter {
    const TYPE: &'static str = "TEST";
    type Event = CounterEvent;
    type Error = cqrs_rust_lib::CqrsError;

    fn aggregate_id(&self) -> String {
        self.id.clone()
    }
    fn with_aggregate_id(self, id: String) -> Self {
        Self { id, ..self }
    }
    fn apply(&mut self, _event: Self::Event) -> Result<(), Self::Error> {
        Ok(())
    }
    fn error(status: http::StatusCode, details: &str) -> Self::Error {
        cqrs_rust_lib::CqrsError::from_status(status, details)
    }
}
}

fn expected() -> Counter {
    Counter {
        id: "a1".into(),
        counter: 3,
        name: "first".into(),
    }
}

/// Each test owns its tables. libtest runs them on separate threads, and the setup drops
/// and recreates what it touches — sharing a table name would let one test pull the rows
/// out from under another.
async fn client(view_table: &str, snapshot_table: &str) -> Option<Arc<Client>> {
    let dsn = std::env::var("PG_TEST_URI").ok()?;
    let (client, connection) = tokio_postgres::connect(&dsn, NoTls).await.ok()?;
    tokio::spawn(async move {
        let _ = connection.await;
    });
    client
        .batch_execute(&format!(
            r#"
            DROP TABLE IF EXISTS {view_table};
            CREATE TABLE {view_table} (id TEXT PRIMARY KEY, parent_id TEXT, data JSONB NOT NULL);
            DROP TABLE IF EXISTS {snapshot_table};
            CREATE TABLE {snapshot_table} (
                aggregate_id TEXT PRIMARY KEY, data JSONB NOT NULL, version BIGINT NOT NULL
            );
            INSERT INTO {snapshot_table} (aggregate_id, data, version)
            VALUES ('a1', '{{"id":"a1","counter":3,"name":"first"}}'::jsonb, 1);
            "#
        ))
        .await
        .ok()?;
    Some(Arc::new(client))
}

/// A plain view: save it, then read it back the two ways the trait offers.
#[tokio::test]
async fn view_round_trip() {
    let Some(client) = client("articles_rt", "snapshots_rt").await else {
        return;
    };
    let store: PostgresStorage<Article, ArticleQuery> =
        PostgresStorage::new(client, "article", "articles_rt");
    let ctx = CqrsContext::default();

    let article = Article {
        id: "a1".into(),
        title: "Catan".into(),
    };
    store
        .save(article.clone(), ctx.clone())
        .await
        .expect("save");

    // `save` strips the id field out of `data` — it lives in its own column — so the
    // read side has to put it back. Without that, every read of every view failed with
    // `missing field \`id\``.
    let found = store
        .find_by_id(None, "a1", ctx.clone())
        .await
        .expect("find_by_id");
    assert_eq!(found.as_ref(), Some(&article));

    let page = store
        .filter(None, ArticleQuery {}, ctx)
        .await
        .expect("filter");
    assert_eq!(page.items, vec![article]);
    assert_eq!(page.total, 1);
}

/// The snapshot-backed read storage, over the table the event store actually writes.
#[tokio::test]
async fn snapshot_read_path() {
    let Some(client) = client("articles_snap", "snapshots_snap").await else {
        return;
    };
    let client2 = client.clone();
    let client3 = client.clone();
    let store = PostgresFromSnapshotStorage::<Counter, ArticleQuery>::new(client, "snapshots_snap");
    let ctx = CqrsContext::default();

    // The key column is `aggregate_id`, and `data` is the bare aggregate.
    assert_eq!(
        store
            .find_by_id(None, "a1", ctx.clone())
            .await
            .expect("find_by_id"),
        Some(expected())
    );
    assert_eq!(
        store
            .filter(None, ArticleQuery {}, ctx.clone())
            .await
            .expect("filter")
            .items,
        vec![expected()]
    );

    // A filter on an aggregate field has to reach `data->>'name'`, which is what the
    // default JsonbDataMapper emits.
    let filtered: PostgresFromSnapshotStorage<Counter, NameQuery> =
        PostgresFromSnapshotStorage::new(client2, "snapshots_snap");
    assert_eq!(
        filtered
            .filter(
                None,
                NameQuery {
                    name: Some("first".into())
                },
                ctx.clone()
            )
            .await
            .expect("filter by name")
            .items,
        vec![expected()]
    );
    assert!(
        filtered
            .filter(
                None,
                NameQuery {
                    name: Some("absent".into())
                },
                ctx.clone()
            )
            .await
            .expect("filter by name")
            .items
            .is_empty()
    );

    // The boundary, pinned so that moving it has to be deliberate: `data->>'counter'`
    // is `text`, while the driver binds an integer filter as `i64`. tokio-postgres
    // refuses the pair before it reaches the wire. A snapshot table is the event store's,
    // not a read model — project a real view when a filter needs types.
    let numeric: PostgresFromSnapshotStorage<Counter, CounterQuery> =
        PostgresFromSnapshotStorage::new(client3, "snapshots_snap");
    let err = numeric
        .filter(None, CounterQuery { counter: Some(3) }, ctx)
        .await
        .expect_err("a non-string filter cannot be compared against a ->> expression");
    assert_eq!(err.status, 500);
}

/// Same question for SurrealDB, whose snapshot row also holds a bare `A` under `data`.
#[cfg(feature = "surrealdb")]
#[tokio::test]
async fn surreal_snapshot_read_path() {
    use cqrs_rust_lib::es::storage::EventStoreStorage;
    use cqrs_rust_lib::read::surrealdb::SurrealDBFromSnapshotStorage;
    use surrealdb::engine::any::connect;

    let db = connect("mem://").await.unwrap();
    db.use_ns("t").use_db("t").await.unwrap();
    let persist = cqrs_rust_lib::es::surrealdb::SurrealDBPersist::<Counter>::new(db.clone());
    db.query(cqrs_rust_lib::es::surrealdb::SurrealDBPersist::<Counter>::schema())
        .await
        .unwrap()
        .check()
        .unwrap();

    // SurrealDB's session type is `()`, hence no binding — clippy rejects one.
    persist
        .save_snapshot(&expected(), 1, &mut persist.start_session().await.unwrap())
        .await
        .unwrap();

    let store = SurrealDBFromSnapshotStorage::<Counter, ArticleQuery>::new(db, "TEST_snapshots");
    let ctx = CqrsContext::default();

    assert_eq!(
        store
            .find_by_id(None, "a1", ctx.clone())
            .await
            .expect("find_by_id"),
        Some(expected())
    );
    assert_eq!(
        store
            .filter(None, ArticleQuery {}, ctx)
            .await
            .expect("filter")
            .items,
        vec![expected()]
    );
}

/// MongoDB stores a whole `Snapshot<A>`, so deserialization works — but does a filter on
/// an aggregate field reach it?
#[cfg(feature = "mongodb")]
#[tokio::test]
async fn mongo_snapshot_read_path() {
    use cqrs_rust_lib::es::storage::EventStoreStorage;
    use cqrs_rust_lib::read::mongodb::MongoDBFromSnapshotStorage;

    let Ok(uri) = std::env::var("MONGODB_TEST_URI") else {
        return;
    };
    let client = mongodb::Client::with_uri_str(&uri).await.unwrap();
    let db = client.database("snap_probe");
    let _ = db.drop().await;

    let persist = cqrs_rust_lib::es::mongodb::MongoDBPersist::<Counter>::new(db.clone());
    let mut session = persist.start_session().await.unwrap();
    persist
        .save_snapshot(&expected(), 1, &mut session)
        .await
        .unwrap();
    session.commit_transaction().await.unwrap();

    let store = MongoDBFromSnapshotStorage::<Counter, NameQuery>::new(db, "TEST_snapshots");
    let ctx = CqrsContext::default();

    assert_eq!(
        store
            .find_by_id(None, "a1", ctx.clone())
            .await
            .expect("find_by_id"),
        Some(expected())
    );
    assert_eq!(
        store
            .filter(None, NameQuery { name: None }, ctx.clone())
            .await
            .expect("filter")
            .items,
        vec![expected()]
    );

    // The document is `{_id, state: {…}, version}`, so a filter on `name` has to reach
    // `state.name`. Under IdentityMapper this returned an empty page and no error.
    assert_eq!(
        store
            .filter(
                None,
                NameQuery {
                    name: Some("first".into())
                },
                ctx.clone()
            )
            .await
            .expect("filter by name")
            .items,
        vec![expected()]
    );
    assert!(
        store
            .filter(
                None,
                NameQuery {
                    name: Some("absent".into())
                },
                ctx
            )
            .await
            .expect("filter by name")
            .items
            .is_empty()
    );
}
