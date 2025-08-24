# cqrs-rust-lib

A comprehensive Command Query Responsibility Segregation (CQRS) and Event Sourcing library for Rust applications.

## Features

- CQRS and Event Sourcing core primitives
- Multiple storage options:
  - In-memory storage (testing/development)
  - MongoDB persistence (feature: `mongodb`)
  - PostgreSQL persistence (feature: `postgres`)
- Aggregate pattern with async command handling and event application
- REST routers (feature: `utoipa`) with Axum and OpenAPI integration
- Typed PostgreSQL read storage utilities (feature: `postgres`)
- Optional dispatchers for denormalization/effects

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
cqrs-rust-lib = "0.1.0"
```

### Cargo features

- `mongodb`: Enable MongoDB persistence
- `postgres`: Enable PostgreSQL persistence and read utilities
- `utoipa`: Enable REST routers and OpenAPI integration
- `mcp`: Enable MCP server integration (experimental)
- `all`: Enable `utoipa`, `mongodb`, and `postgres`

Examples:

```toml
[dependencies]
# MongoDB support
cqrs-rust-lib = { version = "0.1.0", features = ["mongodb"] }
```

```toml
[dependencies]
# PostgreSQL + REST
cqrs-rust-lib = { version = "0.1.0", features = ["postgres", "utoipa"] }
```

## Usage

### Basic (in-memory) example

```rust
use cqrs_rust_lib::{Aggregate, CqrsCommandEngine, CqrsContext};
use cqrs_rust_lib::es::{inmemory::InMemoryPersist, EventStoreImpl};
use http::StatusCode;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Account {
    id: String,
    balance: i64,
}

#[async_trait::async_trait]
impl Aggregate for Account {
    const TYPE: &'static str = "account";
    type CreateCommand = CreateAccount;
    type UpdateCommand = AccountUpdate;
    type Event = AccountEvent;
    type Services = ();
    type Error = anyhow::Error;

    fn aggregate_id(&self) -> String { self.id.clone() }
    fn with_aggregate_id(mut self, id: String) -> Self { self.id = id; self }

    async fn handle_create(&self, _cmd: Self::CreateCommand, _svc: &(), _ctx: &CqrsContext) -> Result<Vec<Self::Event>, Self::Error> {
        Ok(vec![AccountEvent::Opened])
    }
    async fn handle_update(&self, cmd: Self::UpdateCommand, _svc: &(), _ctx: &CqrsContext) -> Result<Vec<Self::Event>, Self::Error> {
        match cmd { AccountUpdate::Deposit { amount } => Ok(vec![AccountEvent::Deposited { amount }]) }
    }
    fn apply(&mut self, event: Self::Event) -> Result<(), Self::Error> {
        match event { AccountEvent::Opened => (), AccountEvent::Deposited { amount } => self.balance += amount };
        Ok(())
    }
    fn error(_status: StatusCode, details: &str) -> Self::Error { anyhow::anyhow!(details.to_string()) }
}

#[derive(Debug, Clone, Serialize, Deserialize)] struct CreateAccount;
#[derive(Debug, Clone, Serialize, Deserialize)] enum AccountUpdate { Deposit { amount: i64 } }
#[derive(Debug, Clone, Serialize, Deserialize)] enum AccountEvent { Opened, Deposited { amount: i64 } }

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let persist = InMemoryPersist::<Account>::new();
    let event_store = EventStoreImpl::new(persist);
    let engine = CqrsCommandEngine::new(
        event_store,
        vec![], // dispatchers
        (),     // services
        Box::new(|e| eprintln!("error: {e}")),
    );

    let ctx = CqrsContext::default();
    let id = engine.execute_create(CreateAccount, &ctx).await?;
    engine.execute_update(&id, AccountUpdate::Deposit { amount: 100 }, &ctx).await?;

    // With metadata
    use std::collections::HashMap;
    let mut meta = HashMap::new();
    meta.insert("source".into(), "readme".into());
    engine.execute_update_with_metadata(&id, AccountUpdate::Deposit { amount: 25 }, meta, &ctx).await?;
    Ok(())
}
```

### PostgreSQL example

Requires feature `postgres`.

```rust
use cqrs_rust_lib::es::{postgres::PostgresPersist, EventStoreImpl};
use cqrs_rust_lib::{CqrsCommandEngine, Dispatcher};
use std::sync::Arc;
use tokio_postgres::NoTls;

async fn setup() -> anyhow::Result<()> {
let (client, connection) = tokio_postgres::connect("postgres://user:pass@localhost/db", NoTls).await?;
tokio::spawn(async move { let _ = connection.await; });
let client = Arc::new(client);

let es_store = PostgresPersist::<Account>::new(client.clone());
let event_store = EventStoreImpl::new(es_store);
let engine = CqrsCommandEngine::new(event_store, vec![] as Vec<Box<dyn Dispatcher<Account>>>, (), Box::new(|e| eprintln!("{e}")));
Ok(()) }
```

### REST routers (optional, feature `utoipa`)

- Write router: `rest::CQRSWriteRouter::routes(Arc<CqrsCommandEngine<_, _>>)`
- Read router: `rest::CQRSReadRouter::routes(repository, Aggregate::TYPE)`
- See a complete wiring in `example\todolist\src\api.rs`.

## API Overview

- Aggregate: Define domain behavior and state transitions
- CqrsCommandEngine: Execute create/update commands, with optional metadata
- EventStore: trait (see `src\event_store.rs`); `EventStoreImpl` provided for common storages
- Event store implementations: `es::inmemory::InMemoryPersist`, `es::mongodb::MongoDBPersist` (feature), `es::postgres::PostgresPersist` (feature)
- Read utilities: `read::postgres::{PostgresStorage, PostgresFromSnapshotStorage}` (feature: `postgres`)

## Examples and running

- example\todolist: full REST app with PostgreSQL and Swagger UI
- example\bank: domain-centric showcase

Run todolist (PowerShell):

```powershell
cargo run -p todolist -- start --pg-uri="postgres://user:pass@localhost:5432/db" --http-port=8081
```

Or set env variable for URI:

```powershell
$env:PG_URL_SECRET = "postgres://user:pass@localhost:5432/db"; cargo run -p todolist -- start --http-port=8081
```

Run tests:

```powershell
cargo test
cargo test -p todolist
```

## License

This project is licensed under the terms found in the [LICENSE](LICENSE) file.

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for details on how to contribute to this project.