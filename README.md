# cqrs-rust-lib

A pragmatic CQRS / Event Sourcing library for Rust with pluggable storage backends, structured domain errors, and REST integration.

## Features

- Split `Aggregate` / `CommandHandler` traits (Single Responsibility)
- Structured domain errors — `CqrsError` + `define_domain_errors!` macro
- Pluggable storage backends: InMemory, MongoDB, PostgreSQL, SurrealDB
- Unified `Query` trait — auto-derives filter from struct fields (RSQL under the hood)
- HTTP Codex convention — `CqrsHttpQuery<Q>` extracts `_q`, `page`, `page_size`, `sort` from HTTP params
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

Response shape:
```json
{
  "domain": "account",
  "code": "ACCOUNT_INSUFFICIENT_FUNDS",
  "internalCode": 10001,
  "status": 400,
  "message": "Cannot withdraw 500, balance is 200"
}
```

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
let snap = Arc::new(db::FromSnapshotStorage::<MyAggregate, MyQuery>::new(Arc::clone(&repo)));
```

| Alias                | inmemory | postgres | mongodb | surrealdb |
|----------------------|----------|----------|---------|-----------|
| `EventStorePersist`  | ✓        | ✓        | ✓       | ✓         |
| `ReadStorage`        | —        | ✓        | ✓       | ✓         |
| `FromSnapshotStorage`| —        | ✓        | ✓       | ✓         |

The connection setup (client, pool, URI) is necessarily backend-specific and stays outside the prelude.

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

`CqrsHttpQuery<Q>` is an Axum extractor that adds `_q` (RSQL), `page`, `page_size`, `sort` on top of any typed `Q`. Use it directly with `CQRSCodexReadRouter`:

```rust
use cqrs_rust_lib::rest::{CQRSCodexReadRouter, CqrsHttpQuery};

// Routes with HTTP Codex params: GET /games?_q=available==true&page=0&page_size=20&sort=-title
CQRSCodexReadRouter::<Game, GameView, GameQuery>::routes(storage, "games")
```

Filter priority: `_q` (RSQL) AND `Q::filter()` — combined. Sort priority: HTTP `sort` → `Q::sort()` → `Q::default_sort()`.

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
