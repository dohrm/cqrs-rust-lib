# cqrs-rust-lib

A comprehensive Command Query Responsibility Segregation (CQRS) and Event Sourcing library for Rust applications.

## Features

- CQRS and Event Sourcing core primitives
- Split `Aggregate` / `CommandHandler` traits (Single Responsibility)
- Structured domain error system with `CqrsError` and `define_domain_errors!` macro
- Multiple storage backends:
  - In-memory (testing/development)
  - MongoDB (feature: `mongodb`)
  - PostgreSQL (feature: `postgres`)
- REST routers with Axum and OpenAPI integration (feature: `utoipa`)
- Audit log router for event history
- Typed read storage and snapshot support
- Pluggable dispatchers for denormalization/projections

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
cqrs-rust-lib = "0.1.0"
```

### Cargo features

| Feature    | Description                                        |
|------------|----------------------------------------------------|
| `mongodb`  | MongoDB event persistence                          |
| `postgres` | PostgreSQL event persistence and read utilities     |
| `utoipa`   | REST routers and OpenAPI/Swagger integration        |
| `mcp`      | MCP server integration (experimental)               |
| `all`      | Enables `utoipa`, `mongodb`, and `postgres`         |

```toml
# PostgreSQL + REST
cqrs-rust-lib = { version = "0.1.0", features = ["postgres", "utoipa"] }

# MongoDB only
cqrs-rust-lib = { version = "0.1.0", features = ["mongodb"] }
```

## Quick Start

### 1. Define your domain

```rust
use cqrs_rust_lib::{Aggregate, CommandHandler, CqrsContext, CqrsError, Event};
use http::StatusCode;
use serde::{Deserialize, Serialize};

// Events
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

// Commands
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CreateCommand { Open { owner: String } }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UpdateCommand { Deposit { amount: i64 }, Withdraw { amount: i64 } }

// Aggregate
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub balance: i64,
}

#[async_trait::async_trait]
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

    fn error(status: StatusCode, details: &str) -> Self::Error {
        CqrsError::from_status(status, details)
    }
}

#[async_trait::async_trait]
impl CommandHandler for Account {
    type CreateCommand = CreateCommand;
    type UpdateCommand = UpdateCommand;
    type Services = ();

    async fn handle_create(
        &self, command: Self::CreateCommand, _svc: &(), _ctx: &CqrsContext,
    ) -> Result<Vec<Self::Event>, Self::Error> {
        match command {
            CreateCommand::Open { owner } => Ok(vec![AccountEvent::Opened { owner }]),
        }
    }

    async fn handle_update(
        &self, command: Self::UpdateCommand, _svc: &(), _ctx: &CqrsContext,
    ) -> Result<Vec<Self::Event>, Self::Error> {
        match command {
            UpdateCommand::Deposit { amount } => Ok(vec![AccountEvent::Deposited { amount }]),
            UpdateCommand::Withdraw { amount } => Ok(vec![AccountEvent::Withdrawn { amount }]),
        }
    }
}
```

### 2. Create the engine and execute commands

```rust
use cqrs_rust_lib::es::{inmemory::InMemoryPersist, EventStoreImpl};
use cqrs_rust_lib::{CqrsCommandEngine, CqrsContext};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let store = EventStoreImpl::new(InMemoryPersist::<Account>::new());
    let engine = CqrsCommandEngine::new(store, vec![], (), Box::new(|_e| {}));

    let ctx = CqrsContext::default();
    let id = engine.execute_create(CreateCommand::Open { owner: "Alice".into() }, &ctx).await?;
    engine.execute_update(&id, UpdateCommand::Deposit { amount: 100 }, &ctx).await?;
    Ok(())
}
```

## Domain Error Codes

Define structured, domain-specific error codes with the `define_domain_errors!` macro:

```rust
use cqrs_rust_lib::{define_domain_errors, CqrsError, CqrsErrorCode};
use http::StatusCode;

define_domain_errors! {
    domain: "account",
    prefix: 10,
    errors: {
        InsufficientFunds => (1, StatusCode::BAD_REQUEST, "INSUFFICIENT_FUNDS"),
        InvalidAmount     => (2, StatusCode::BAD_REQUEST, "INVALID_AMOUNT"),
        AccountClosed     => (3, StatusCode::GONE, "ACCOUNT_CLOSED"),
    }
}

impl From<ErrorCode> for CqrsError {
    fn from(e: ErrorCode) -> Self {
        e.error(e.to_string())
    }
}
```

Use in command handlers:

```rust
if self.balance < amount {
    return Err(ErrorCode::InsufficientFunds.error(
        format!("Cannot withdraw {}, balance is {}", amount, self.balance)
    ));
}
```

API response:

```json
{
  "domain": "account",
  "code": "ACCOUNT_INSUFFICIENT_FUNDS",
  "internalCode": 10001,
  "status": 400,
  "message": "Cannot withdraw 500, balance is 200"
}
```

See `docs/migration_guide/domain_errors.md` for the full migration guide.

## Storage Backends

### PostgreSQL (feature: `postgres`)

```rust
use cqrs_rust_lib::es::{postgres::PostgresPersist, EventStoreImpl};
use std::sync::Arc;
use tokio_postgres::NoTls;

let (client, conn) = tokio_postgres::connect("postgres://user:pass@localhost/db", NoTls).await?;
tokio::spawn(async move { let _ = conn.await; });
let client = Arc::new(client);

let store = EventStoreImpl::new(PostgresPersist::<Account>::new(client.clone()));
let engine = CqrsCommandEngine::new(store, vec![], (), Box::new(|_e| {}));
```

### MongoDB (feature: `mongodb`)

Same pattern with `es::mongodb::MongoDBPersist`.

## REST Routers (feature: `utoipa`)

Axum routers with auto-generated OpenAPI schemas:

- **Write router**: `rest::CQRSWriteRouter::routes(Arc<CqrsCommandEngine<A>>)`
- **Read router**: `rest::CQRSReadRouter::routes(repository, Aggregate::TYPE)`
- **Audit log router**: `rest::CQRSAuditLogRouter::routes(event_store, tag)`

See `example/todolist/src/api.rs` for complete wiring with Swagger UI.

## Architecture

```
Aggregate (state + events)     CommandHandler (commands -> events)
         \                       /
          CqrsCommandEngine ----+---- EventStore (persist)
                |                         |
           Dispatchers              Storage backends
          (projections)          (InMemory/PG/Mongo)
```

### Key Types

| Type | Description |
|------|-------------|
| `Aggregate` | Domain state, event application, identity |
| `CommandHandler` | Command processing, business validation |
| `CqrsCommandEngine` | Orchestrates command execution |
| `EventStore` / `EventStoreImpl` | Event persistence abstraction |
| `CqrsError` | Unified structured error type |
| `CqrsErrorCode` | Trait for domain-specific error codes |
| `CqrsContext` | Carries user, request ID, correlation |
| `Dispatcher` | React to persisted events (projections) |
| `View` / `ViewElements` | Read model projections |

## Examples

| Example | Storage | Features |
|---------|---------|----------|
| `example/bank` | MongoDB | Domain errors, commands, queries, views |
| `example/todolist` | PostgreSQL | REST API, Swagger UI, snapshots, integration tests |

### Run todolist

```bash
cargo run -p todolist -- start --pg-uri="postgres://user:pass@localhost:5432/db" --http-port=8081
```

### Run tests

```bash
cargo test             # lib tests
cargo test -p todolist # integration tests
```

## Migration Guides

- [Aggregate / CommandHandler Split](docs/migration_guide/split_aggregate.md)
- [Domain Error Codes](docs/migration_guide/domain_errors.md)

## License

This project is licensed under the terms found in the [LICENSE](LICENSE) file.

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for details.
