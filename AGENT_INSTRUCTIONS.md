# AI Agent Instruction: How to Use cqrs-rust-lib

Purpose:
- This file is designed to be pasted into an AI agent prompt as context. It explains the minimal, correct way to use cqrs-rust-lib based on the current repository code and examples.
- Prefer concrete identifiers found in this repo over invented ones. Do not change function signatures.

Key concepts and types:
- Aggregate (src/aggregate.rs): core domain object implementing business logic.
- Event (src/event.rs): domain events emitted by aggregates.
- CqrsContext (src/context.rs): carries correlation IDs, user, time; used when executing commands.
- CqrsCommandEngine (src/engine.rs): orchestrates command execution against an event store.
- EventStore (src/event_store.rs): trait used by the engine; EventStoreImpl is a provided implementation.
- Event persistence backends (src/es):
  - es::inmemory::InMemoryPersist (always available)
  - es::mongodb::MongoDBPersist (feature "mongodb")
  - es::postgres::PostgresPersist (feature "postgres")
- Read/REST utilities (feature "utoipa"):
  - rest::CQRSWriteRouter and rest::CQRSReadRouter for Axum routing with OpenAPI.
- Dispatcher (src/denormalizer.rs): plug-in handlers invoked on persisted events (for projections, messaging, etc.).

Engine constructor (as implemented):
- CqrsCommandEngine::new(store, dispatchers: Vec<Box<dyn Dispatcher<A>>>, services: A::Services, error_handler: Box<dyn Fn(&AggregateError) + Send + Sync>)

Minimal getting-started flow
1) Define your domain aggregate and implement Aggregate

```rust
use cqrs_rust_lib::{Aggregate, CqrsContext};
use http::StatusCode;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub balance: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AccountCreate {
    Open { owner: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AccountUpdate {
    Deposit { amount: i64 },
    Withdraw { amount: i64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AccountEvent {
    Opened { owner: String },
    Deposited { amount: i64 },
    Withdrawn { amount: i64 },
}

#[async_trait::async_trait]
impl Aggregate for Account {
    const TYPE: &'static str = "account";

    type CreateCommand = AccountCreate;
    type UpdateCommand = AccountUpdate;
    type Event = AccountEvent;
    type Services = (); // inject your app services here if needed
    type Error = std::io::Error;

    fn aggregate_id(&self) -> String { self.id.clone() }
    fn with_aggregate_id(mut self, id: String) -> Self { self.id = id; self }

    async fn handle_create(
        &self,
        command: Self::CreateCommand,
        _services: &Self::Services,
        _context: &CqrsContext,
    ) -> Result<Vec<Self::Event>, Self::Error> {
        match command {
            AccountCreate::Open { owner } => Ok(vec![AccountEvent::Opened { owner }]),
        }
    }

    async fn handle_update(
        &self,
        command: Self::UpdateCommand,
        _services: &Self::Services,
        _context: &CqrsContext,
    ) -> Result<Vec<Self::Event>, Self::Error> {
        match command {
            AccountUpdate::Deposit { amount } => Ok(vec![AccountEvent::Deposited { amount }]),
            AccountUpdate::Withdraw { amount } => Ok(vec![AccountEvent::Withdrawn { amount }]),
        }
    }

    fn apply(&mut self, event: Self::Event) -> Result<(), Self::Error> {
        match event {
            AccountEvent::Opened { .. } => { /* initialize fields if needed */ }
            AccountEvent::Deposited { amount } => { self.balance += amount; }
            AccountEvent::Withdrawn { amount } => { self.balance -= amount; }
        }
        Ok(())
    }

    fn error(_status: StatusCode, details: &str) -> Self::Error {
        std::io::Error::new(std::io::ErrorKind::Other, details.to_string())
    }
}
```

2) Choose an event store and create the CQRS engine

In-memory (no external services):
```rust
use cqrs_rust_lib::es::inmemory::InMemoryPersist;
use cqrs_rust_lib::es::EventStoreImpl;
use cqrs_rust_lib::{CqrsCommandEngine, CqrsContext, Dispatcher};
use std::sync::Arc;

let store = InMemoryPersist::<Account>::new();
let event_store = EventStoreImpl::new(store);
let dispatchers: Vec<Box<dyn Dispatcher<Account>>> = vec![]; // add your denormalizers here
let engine = Arc::new(CqrsCommandEngine::new(
    event_store,
    dispatchers,
    (),
    Box::new(|e| { eprintln!("CQRS error: {}", e); }),
));
let ctx = CqrsContext::default();
```

PostgreSQL (feature "postgres") — mirrors example/todolist:
```rust
// Cargo.toml of your app: cqrs-rust-lib = { version = "*", features = ["postgres"] }
use cqrs_rust_lib::es::postgres::PostgresPersist;
use cqrs_rust_lib::es::EventStoreImpl;
use tokio_postgres::NoTls;
use std::sync::Arc;

let (client, connection) = tokio_postgres::connect("postgres://user:pass@localhost:5432/db", NoTls).await?;
tokio::spawn(async move { let _ = connection.await; });
let client = Arc::new(client);
let persist = PostgresPersist::<Account>::new(client.clone());
let event_store = EventStoreImpl::new(persist);
let engine = Arc::new(CqrsCommandEngine::new(event_store, vec![], (), Box::new(|e| eprintln!("{}", e))));
```

MongoDB (feature "mongodb") — analogous pattern with es::mongodb::MongoDBPersist.

3) Execute commands
```rust
use cqrs_rust_lib::CqrsContext;

let ctx = CqrsContext::new(Some("alice".to_string())).with_next_request_id();
let account_id = engine
    .execute_create(AccountCreate::Open { owner: "alice".into() }, &ctx)
    .await?;

engine
    .execute_update(&account_id, AccountUpdate::Deposit { amount: 100 }, &ctx)
    .await?;

// With metadata variants are available as well:
use std::collections::HashMap;
let mut meta = HashMap::new();
meta.insert("source".into(), "test".into());
engine.execute_update_with_metadata(&account_id, AccountUpdate::Withdraw { amount: 25 }, meta, &ctx).await?;
```

4) Expose REST endpoints (optional; feature "utoipa")
- Example pattern is in example/todolist/src/api.rs.
- Write router: rest::CQRSWriteRouter::routes(Arc<CqrsCommandEngine<_, _>>)
- Read router requires a repository. You can:
  - Implement your own storage (see example/todolist/src/pg_snapshot_storage.rs), or
  - Use read::postgres::PostgresStorage for typed queries (see src/read/postgres.rs) if it fits your view model.

Minimal Axum wiring (based on example/todolist):
```rust
use cqrs_rust_lib::rest::{CQRSReadRouter, CQRSWriteRouter};
use std::sync::Arc;

let write_router = CQRSWriteRouter::routes(engine.clone());
// For read side, provide your repository and aggregate type string
let read_router = CQRSReadRouter::routes(repository, Account::TYPE);

// Mount into your Axum/OpenApi router as shown in example/todolist/src/api.rs
```

5) Denormalizers/Dispatchers (optional)
- Implement trait denormalizer::Dispatcher<A> to react to persisted events.
- Pass your dispatchers when constructing CqrsCommandEngine via the Vec<Box<dyn Dispatcher<A>>> parameter.

Examples in this repository:
- example/todolist: full working REST app with PostgreSQL event store and Swagger UI.
  - Aggregate: example/todolist/src/todolist/aggregate.rs
  - Commands: example/todolist/src/todolist/commands.rs
  - Events: example/todolist/src/todolist/events.rs
  - API: example/todolist/src/api.rs
  - Custom snapshot storage: example/todolist/src/pg_snapshot_storage.rs
  - Tests: example/todolist/tests/integration_tests.rs
- example/bank: domain-centric showcase (account) illustrating commands/events/queries.

Build and run commands (from workspace root):
- Run todolist example (Windows PowerShell):
  - cargo run -p todolist -- start --pg-uri="postgres://user:pass@localhost:5432/db" --http-port=8081
  - Or set env var for URI: $env:PG_URL_SECRET = "postgres://user:pass@localhost:5432/db"; cargo run -p todolist -- start --http-port=8081
- Run tests:
  - cargo test
  - cargo test -p todolist

Common pitfalls:
- Engine constructor requires four parameters (store, dispatchers, services, error_handler). Do not use older signatures.
- Remember to derive Default, Debug, Clone, Serialize, Deserialize on your aggregate type.
- Implement error(StatusCode, &str) even if you use a simple error type.
- Enable appropriate features in your application Cargo.toml (e.g., utoipa, postgres, mongodb) matching the storage/router you use.

Troubleshooting quick checks:
- If you see version conflicts during commit, ensure you load/apply events in order and use the same aggregate ID.
- If REST routers are missing, verify you compiled with the "utoipa" feature: cqrs-rust-lib = { version = "*", features = ["utoipa"] }.
- For PostgreSQL, ensure required tables are created (see example/todolist/src/api.rs for DDL used at startup).

API reference pointers (exact repo paths):
- src/aggregate.rs (Aggregate trait)
- src/engine.rs (CqrsCommandEngine and methods: execute_create, execute_update, execute_*_with_metadata)
- src/es/mod.rs and backends in src/es/*
- src/read/* and src/rest/* (feature-gated)
- example/todolist/* and example/bank/* for end-to-end usage
