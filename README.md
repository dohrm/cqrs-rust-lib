# cqrs-rust-lib

A pragmatic CQRS / Event Sourcing library for Rust with pluggable storage backends, structured domain errors, and REST integration.

## Features

- Split `Aggregate` / `CommandHandler` traits (Single Responsibility)
- Structured domain errors — `CqrsError` + `define_domain_errors!` macro
- Pluggable storage backends: InMemory, MongoDB, PostgreSQL, SurrealDB
- Unified `Query` trait — auto-derives filter from struct fields (RSQL under the hood)
- HTTP Codex convention — `CqrsHttpQuery<Q>` extracts `_q`, `skip`/`limit`, `page`/`page_size`, `sort` from HTTP params
- RFC 9457 `application/problem+json` error responses (feature: `problem-json`)
- Backend prelude pattern — swap the entire backend with one `use` line
- REST routers with Axum and auto-generated OpenAPI/Swagger (feature: `rest`)
- Audit log router for event history
- Snapshot support
- WASM-compatible core (no Tokio in production deps)

## Installation

```toml
[dependencies]
cqrs-rust-lib = { version = "0.7", features = ["postgres"] }
```

### Feature flags

| Feature     | Description                                            |
|-------------|--------------------------------------------------------|
| `mongodb`   | MongoDB event store + read storage                     |
| `postgres`  | PostgreSQL event store + read storage                  |
| `surrealdb` | SurrealDB event store + read storage                   |
| `utoipa`    | OpenAPI schema derives only (WASM-compatible)          |
| `rest`      | Axum routers + OpenAPI (implies `utoipa`, native only) |
| `problem-json` | Serve errors as RFC 9457 `application/problem+json` |
| `all`       | `rest` + `mongodb` + `postgres` + `surrealdb`          |

## Quick Start

### 1. Define your domain

```rust
use cqrs_rust_lib::{Aggregate, CommandHandler, CqrsContext, CqrsError, Event};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AccountEvent {
    Opened { owner: String },
    Deposited { amount: i64 },
    Withdrawn { amount: i64 },
}

impl Event for AccountEvent {
    fn event_type(&self) -> String {
        match self {
            Self::Opened { .. } => "opened".into(),
            Self::Deposited { .. } => "deposited".into(),
            Self::Withdrawn { .. } => "withdrawn".into(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub balance: i64,
}

impl Aggregate for Account {
    const TYPE: &'static str = "account";
    type Event = AccountEvent;
    type Error = CqrsError;

    fn aggregate_id(&self) -> String { self.id.clone() }
    fn with_aggregate_id(mut self, id: String) -> Self { self.id = id; self }

    fn apply(&mut self, event: Self::Event) -> Result<(), Self::Error> {
        match event {
            AccountEvent::Opened { .. } => {}
            AccountEvent::Deposited { amount } => self.balance += amount,
            AccountEvent::Withdrawn { amount } => self.balance -= amount,
        }
        Ok(())
    }
}

impl CommandHandler for Account {
    type CreateCommand = CreateCommand;
    type UpdateCommand = UpdateCommand;
    type Services = ();

    async fn handle_create(&self, cmd: CreateCommand, _: &(), _: &CqrsContext)
        -> Result<Vec<AccountEvent>, CqrsError>
    {
        match cmd {
            CreateCommand::Open { owner } => Ok(vec![AccountEvent::Opened { owner }]),
        }
    }

    async fn handle_update(&self, cmd: UpdateCommand, _: &(), _: &CqrsContext)
        -> Result<Vec<AccountEvent>, CqrsError>
    {
        match cmd {
            UpdateCommand::Deposit { amount } => Ok(vec![AccountEvent::Deposited { amount }]),
            UpdateCommand::Withdraw { amount } => Ok(vec![AccountEvent::Withdrawn { amount }]),
        }
    }
}
```

### 2. Execute commands

```rust
use cqrs_rust_lib::es::{inmemory::InMemoryPersist, EventStoreImpl};
use cqrs_rust_lib::{CqrsCommandEngine, CqrsContext};

let store = EventStoreImpl::new(InMemoryPersist::<Account>::new());
let engine = CqrsCommandEngine::new(store, vec![], (), Box::new(|_e| {}));

let ctx = CqrsContext::default();
let id = engine.execute_create(CreateCommand::Open { owner: "Alice".into() }, &ctx).await?;
engine.execute_update(&id, UpdateCommand::Deposit { amount: 100 }, &ctx).await?;
```

## Domain Error Codes

```rust
use cqrs_rust_lib::{define_domain_errors, CqrsError, CqrsErrorCode};
use http::StatusCode;

define_domain_errors! {
    domain: "account",
    prefix: 10,
    errors: {
        InsufficientFunds => (1, StatusCode::BAD_REQUEST, "INSUFFICIENT_FUNDS"),
        AccountClosed     => (3, StatusCode::GONE,        "ACCOUNT_CLOSED"),
    }
}

impl From<ErrorCode> for CqrsError {
    fn from(e: ErrorCode) -> Self { e.error(e.to_string()) }
}
```

Response shape (default):
```json
{
  "domain": "account",
  "code": "ACCOUNT_INSUFFICIENT_FUNDS",
  "internalCode": 10001,
  "message": "Cannot withdraw 500, balance is 200",
  "requestId": "req-123"
}
```

`CqrsError::from_status` never degrades a status: any code without a dedicated
`GenericErrorCode` variant keeps its value through `GenericErrorCode::Other`
(`GENERIC_HTTP_418`, internal code 1418). 402, 405, 406, 408, 412, 413, 415,
422, 423, 428, 429, 501, 503 and 504 have dedicated variants whose internal code
is `1000 + status`.

### RFC 9457 problem details (`feature: problem-json`)

With the `problem-json` feature the REST layer serves
`application/problem+json` documents instead:

```json
{
  "type": "urn:cqrs-error:account:ACCOUNT_INSUFFICIENT_FUNDS",
  "title": "ACCOUNT_INSUFFICIENT_FUNDS",
  "status": 400,
  "detail": "Cannot withdraw 500, balance is 200",
  "instance": "urn:cqrs-request:req-123",
  "domain": "account",
  "code": "ACCOUNT_INSUFFICIENT_FUNDS",
  "internalCode": 10001,
  "requestId": "req-123"
}
```

The `type` member defaults to `urn:cqrs-error:{domain}:{code}`. Point it at your
own documentation with a base URI, or override it per error:

```rust
use cqrs_rust_lib::problem::set_problem_type_base_uri;

set_problem_type_base_uri("https://api.example.com/errors").unwrap();
// -> "type": "https://api.example.com/errors/ACCOUNT_INSUFFICIENT_FUNDS"

CqrsError::conflict("slug taken").with_type_uri("https://api.example.com/errors/slug");
```

`CqrsError::to_problem()` is available without the feature, for hand-rolled
routes. See `docs/migration_guide/problem_json.md`.

## Backend Preludes

Each backend exposes canonical type aliases under `cqrs_rust_lib::prelude::<backend>`.
**Swapping the backend requires changing a single import line** — the rest of the wiring is identical.

```rust
// Change only this line to swap backends:
use cqrs_rust_lib::prelude::postgres as db;
// use cqrs_rust_lib::prelude::mongodb as db;
// use cqrs_rust_lib::prelude::surrealdb as db;

// Everything below stays the same:
let es = db::EventStorePersist::<MyAggregate>::new(connection.clone());
let repo = Arc::new(db::ReadStorage::<MyView, MyQuery>::new(connection.clone(), "my_view", ...));
// Reads the event store's own snapshot table, so it takes the table, not a view storage.
let snap = Arc::new(db::FromSnapshotStorage::<MyAggregate, MyQuery>::new(
    connection.clone(),
    es.snapshot_table_name(),
));
```

| Alias                | inmemory | postgres | mongodb | surrealdb |
|----------------------|----------|----------|---------|-----------|
| `EventStorePersist`  | ✓        | ✓        | ✓       | ✓         |
| `ReadStorage`        | —        | ✓        | ✓       | ✓         |
| `FromSnapshotStorage`| —        | ✓        | ✓       | ✓         |

The connection setup (client, pool, URI) is necessarily backend-specific and stays outside the prelude.

`FromSnapshotStorage` reads the event store's snapshot table directly — its layout differs from a view table on every backend — and defaults to a mapper naming where the aggregate actually sits: `data->>'field'` on Postgres, `data.field` on SurrealDB, `state.field` on MongoDB. See [`docs/migration_guide/snapshot_read_storage.md`](docs/migration_guide/snapshot_read_storage.md).

## Query Trait (Read Side)

`Query` is the unified read-side filter/pagination/sort interface. It requires `Serialize` (supertrait) so that equality filters are auto-derived from struct fields — **no boilerplate needed in most cases**.

```rust
use cqrs_rust_lib::read::Query;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameQuery {
    pub category: Option<String>,   // non-None → category == "value"
    pub available: Option<bool>,    // non-None → available == true/false
}

// Empty impl: filter auto-derived, no pagination override, no sort
impl Query for GameQuery {}
```

Override only what you need:

```rust
use cqrs_rust_lib::read::{Query, Sorter, SortDirection};
use cqrs_rust_lib::rsql::{Ast, Constraint, Operator, RestSql, Value};

impl Query for ProductQuery {
    // Custom filter: min_price uses >= instead of ==
    fn filter(&self) -> Option<RestSql> {
        self.min_price.and_then(|p| {
            RestSql::from_ast(Ast::Constraint(Constraint {
                field: "price".into(),
                operator: Operator::Gte,
                value: Value::Float(p),
            })).ok()
        })
    }

    // Static default sort — applied when no HTTP sort param is given
    fn default_sort() -> Option<Vec<Sorter>> {
        Some(vec![Sorter { field: "name".into(), direction: SortDirection::Asc }])
    }
}
```

### HTTP Codex convention (`feature: rest`)

`CqrsHttpQuery<Q>` is an Axum extractor that adds `_q` (RSQL), pagination and `sort` on top of any typed `Q`. Use it directly with `CQRSCodexReadRouter`:

```rust
use cqrs_rust_lib::rest::{CQRSCodexReadRouter, CqrsHttpQuery};

// GET /games?_q=available==true&skip=20&limit=20&sort=-title
CQRSCodexReadRouter::<Game, GameView, GameQuery>::routes(storage, "games")
```

Filter priority: `_q` (RSQL) AND `Q::filter()` — combined. Sort priority: HTTP `sort` → `Q::sort()` → `Q::default_sort()`.

The typed params of `Q` and the RSQL `_q` string are **one set of filterable fields in two syntaxes** — RSQL exists because a flat `?field=value` cannot express `>=`, `=in=`, `or` or a range. So `_q` may only name fields of the query struct: a field not reachable as a query param has no reason to be reachable from `_q`. The set is derived from `Q`'s `Deserialize` impl, so there is no second list to keep in step and every filterable field is a typed OpenAPI parameter by construction. A field the struct does not declare is rejected with **422** naming it; a query struct with no fields offers no filter at all.

```rust
#[derive(Debug, Clone, Serialize, Deserialize, IntoParams)]
pub struct GameQuery {
    pub category: Option<String>,
    pub title: Option<String>,   // filterable, therefore a field
}

impl Query for GameQuery {
    // Sorting gets its own list: ordering by a column of the view is reasonable
    // where filtering on it is not, so there is nothing to derive it from. Empty
    // (the default) means the view offers no sort at all.
    fn sortable_fields(&self) -> Vec<&str> {
        vec!["id", "title", "category"]
    }
}
```

Both constrain the caller, not `Query::default_sort()` — a view can order its own results while offering the caller no say. Note what this does and does not do: the fields are still returned in the response body, so it stops a listing being used as a *lookup by* an unoffered field — it does not hide it. See [ADR-0002](docs/adr/0002-declare-the-queryable-field-surface-on-the-view.md), [ADR-0003](docs/adr/0003-sorting-declares-its-own-field-list.md) and [`docs/migration_guide/queryable_fields.md`](docs/migration_guide/queryable_fields.md).

A query parameter the extractor cannot read is rejected with **422 Unprocessable Entity**, not silently dropped: a `_q` that fails to parse (the response carries rest-sql's positioned error, caret included), and a `skip`/`limit`/`page`/`page_size` that is not a non-negative integer. An *empty* value — `?_q=&limit=10` — means the parameter is unset, not unreadable, and is accepted. See [`docs/migration_guide/codex_query_rejection.md`](docs/migration_guide/codex_query_rejection.md).

A sort field name must be one or more `.`-separated segments matching `[A-Za-z_][A-Za-z0-9_]*` — the dot addresses a nested path on MongoDB and SurrealDB. The name is interpolated into the generated `ORDER BY`, never bound as a parameter, so anything else (a space, a quote, a hyphen, a non-ASCII letter) is rejected with **400 Validation failed** naming the field. The check runs in the storage layer, so it applies to a `Sorter` built in Rust and handed to `Storage::filter` just as much as to the HTTP `sort` param. A view whose stored keys do not fit that grammar needs a `FieldMapper` translating a legal logical name to it.

Pagination accepts both vocabularies; `skip`/`limit` wins when both are present:

| Params | Meaning |
|---|---|
| `skip`, `limit` | Offset based, maps straight to `Pagination`. `skip` alone is honoured (backend default limit applies). |
| `page`, `page_size` (alias `pageSize`) | Page based, translated to `skip = page * page_size`. |

`Paged<T>` reports both forms, so `skip`/`limit` stay exact even when `skip` is not a multiple of `limit`:

```json
{ "items": [], "total": 137, "skip": 25, "limit": 10, "page": 2, "pageSize": 10 }
```

## Storage Backends

### PostgreSQL

```rust
use cqrs_rust_lib::prelude::postgres as db;
use tokio_postgres::NoTls;

let (client, conn) = tokio_postgres::connect("postgres://user:pass@localhost/db", NoTls).await?;
tokio::spawn(async move { let _ = conn.await; });
let client = Arc::new(client);

client.batch_execute(&db::EventStorePersist::<Account>::schema()).await?;
let es = db::EventStorePersist::<Account>::from_client(client.clone());
let views = db::ReadStorage::<AccountView, AccountQuery>::new(client, "account", "account_view");
```

#### Connection pooling

Both the event store and the read storage acquire connections through the same
`PgPool` trait. The default `SharedClient` wraps a single `Arc<Client>`; plug a
real pool (deadpool-postgres, bb8, …) by implementing the two traits — no extra
dependency is pulled into the library:

```rust
use cqrs_rust_lib::prelude::postgres::{PgConn, PgPool};
use cqrs_rust_lib::{cqrs_async_trait, CqrsError};

#[derive(Debug, Clone)]
struct DeadPool(deadpool_postgres::Pool);
struct DeadConn(deadpool_postgres::Object);

impl PgConn for DeadConn {
    fn client(&self) -> &tokio_postgres::Client { &self.0 }
}

cqrs_async_trait! {
impl PgPool for DeadPool {
    type Connection = DeadConn;
    async fn acquire(&self) -> Result<Self::Connection, CqrsError> {
        self.0.get().await.map(DeadConn).map_err(CqrsError::database_error)
    }
}
}

let es = db::EventStorePersist::<Account>::with_pool(pool.clone());
let views = db::ReadStorage::<AccountView, AccountQuery>::with_pool(pool, "account", "account_view");
```

### MongoDB

```rust
use cqrs_rust_lib::prelude::mongodb as db;

let options = ClientOptions::parse(uri).await?;
let db_client = mongodb::Client::with_options(options.clone())?;
let database = db_client.database(&options.default_database.unwrap());

let es = db::EventStorePersist::<Account>::new(database.clone());
```

### SurrealDB

```rust
use cqrs_rust_lib::prelude::surrealdb as db;
use surrealdb::engine::any::connect;

let surreal = connect(uri).await?;
surreal.use_ns("myns").use_db("mydb").await?;
surreal.query(db::EventStorePersist::<Game>::schema()).await?.check()?;

let es = db::EventStorePersist::<Game>::new(surreal.clone());
```

## REST Routers (feature: `rest`)

```rust
use cqrs_rust_lib::rest::{CQRSWriteRouter, CQRSReadRouter, CQRSAuditLogRouter, CQRSCodexReadRouter};

// Standard router — typed query params only
CQRSReadRouter::routes(repository, Aggregate::TYPE)

// Codex router — adds _q, page, page_size, sort HTTP params
CQRSCodexReadRouter::<A, V, Q>::routes(storage, tag)

// Write + audit
CQRSWriteRouter::routes(engine)
CQRSAuditLogRouter::routes(event_store, tag)
```

See `example/todolist/src/api.rs` for complete wiring with Swagger UI.

## Architecture

```
Aggregate (state + events)     CommandHandler (commands → events)
         \                       /
          CqrsCommandEngine ────── EventStore (persist)
                │                         │
           Dispatchers              Storage backends
          (projections)          (InMemory / PG / Mongo / Surreal)
                │
           ReadStorage ← Query (filter + sort + pagination)
```

### Key Types

| Type                            | Description                                          |
|---------------------------------|------------------------------------------------------|
| `Aggregate`                     | Domain state, event application, identity            |
| `CommandHandler`                | Command processing, business validation              |
| `CqrsCommandEngine`             | Orchestrates command execution                       |
| `EventStore` / `EventStoreImpl` | Event persistence abstraction                        |
| `CqrsError`                     | Unified structured error type                        |
| `CqrsContext`                   | Carries user, request ID, correlation ID             |
| `Dispatcher`                    | Reacts to persisted events (projections / views)     |
| `View`                          | Read model projection                                |
| `Query`                         | Read-side filter / pagination / sort interface       |
| `CqrsHttpQuery<Q>`              | HTTP Codex extractor wrapping a typed `Q`            |

## Examples

| Example                  | Storage    | Highlights                                                  |
|--------------------------|------------|-------------------------------------------------------------|
| `example/bank`           | MongoDB    | Domain errors (prefix 10), views, movements sub-resource   |
| `example/todolist`       | PostgreSQL | REST API, Swagger UI, snapshots, integration tests          |
| `example/ludotheque`     | SurrealDB  | Full pipeline: event store + view + filter + sort           |

```bash
cargo run -p todolist    -- start --pg-uri="postgres://..." --http-port=8081
cargo run -p ludotheque  -- start --surreal-uri="ws://..." --http-port=8082

cargo test               # lib unit tests
cargo test -p todolist   # todolist integration tests
cargo test -p ludotheque # ludotheque integration tests
```

## Migration Guides

- [Aggregate / CommandHandler Split](docs/migration_guide/split_aggregate.md)
- [Domain Error Codes](docs/migration_guide/domain_errors.md)
- [WASM Compatibility](docs/migration_guide/wasm_compat.md)
- [Query Trait (0.6 → 0.7)](docs/migration_guide/query_trait.md)

## License

MIT — see [LICENSE](LICENSE).
