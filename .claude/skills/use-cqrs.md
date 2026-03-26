# Skill: use-cqrs

You are about to implement or modify code that uses `cqrs-rust-lib`, a Rust CQRS/Event Sourcing library.

Read these instructions carefully before writing any code.

---

## 1. Cargo.toml

```toml
[dependencies]
cqrs-rust-lib = { version = "...", features = [] }
# Features: "utoipa" (schemas only), "rest" (axum routers, implies utoipa),
#           "postgres", "mongodb", "all" (rest + postgres + mongodb)
# async-trait is NOT needed — it is re-exported by the library.
serde = { version = "1", features = ["derive"] }
thiserror = "2"          # required for define_domain_errors!
http = "1"
tokio = { version = "1", features = ["full"] }
```

---

## 2. Trait hierarchy

Two traits, both implemented on the same struct:

- **`Aggregate`**: state + event application. Required: `TYPE`, `Event`, `Error`, `aggregate_id`, `with_aggregate_id`, `apply`, `error`.
- **`CommandHandler: Aggregate`**: command processing. Required: `CreateCommand`, `UpdateCommand`, `Services`, `handle_create`, `handle_update`.

---

## 3. Minimal example

```rust
use cqrs_rust_lib::cqrs_async_trait;
use cqrs_rust_lib::{Aggregate, CommandHandler, CqrsContext, CqrsError, CqrsErrorCode};
use http::StatusCode;
use serde::{Deserialize, Serialize};

// --- Events ---
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MyEvent {
    Created { name: String },
    Updated { value: i32 },
}

impl cqrs_rust_lib::Event for MyEvent {
    fn event_type(&self) -> String {
        match self {
            Self::Created { .. } => "Created".into(),
            Self::Updated { .. } => "Updated".into(),
        }
    }
}

// --- Commands ---
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CreateCmd { Create { name: String } }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UpdateCmd { Update { value: i32 } }

// --- Aggregate ---
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MyAggregate {
    pub id: String,
    pub name: String,
    pub value: i32,
}

cqrs_async_trait! {
impl Aggregate for MyAggregate {
    const TYPE: &'static str = "my_aggregate";

    type Event = MyEvent;
    type Error = CqrsError;

    fn aggregate_id(&self) -> String { self.id.clone() }

    fn with_aggregate_id(mut self, id: String) -> Self {
        self.id = id;
        self
    }

    fn apply(&mut self, event: Self::Event) -> Result<(), Self::Error> {
        match event {
            MyEvent::Created { name } => self.name = name,
            MyEvent::Updated { value } => self.value = value,
        }
        Ok(())
    }

    fn error(status: StatusCode, details: &str) -> Self::Error {
        CqrsError::from_status(status, details)
    }
}
}

cqrs_async_trait! {
impl CommandHandler for MyAggregate {
    type CreateCommand = CreateCmd;
    type UpdateCommand = UpdateCmd;
    type Services = ();   // use a struct if external services are needed

    async fn handle_create(
        &self,
        command: Self::CreateCommand,
        _services: &Self::Services,
        _context: &CqrsContext,
    ) -> Result<Vec<Self::Event>, Self::Error> {
        match command {
            CreateCmd::Create { name } => Ok(vec![MyEvent::Created { name }]),
        }
    }

    async fn handle_update(
        &self,
        command: Self::UpdateCommand,
        _services: &Self::Services,
        _context: &CqrsContext,
    ) -> Result<Vec<Self::Event>, Self::Error> {
        match command {
            UpdateCmd::Update { value } => Ok(vec![MyEvent::Updated { value }]),
        }
    }
}
}
```

Key rules:
- `Aggregate` owns: state fields, `apply`, `error`.
- `CommandHandler` owns: command types, `Services`, `handle_create`, `handle_update`.
- Both blocks must be wrapped with `cqrs_async_trait! { ... }` (NOT `#[async_trait::async_trait]`).
- The struct must derive `Default` (used to initialize a new aggregate).
- `CqrsContext` carries the request ID and current user. Use `context.next_uuid()` to generate deterministic UUIDs for child entities.

---

## 4. Domain error codes

Replace `CqrsError::from_status(...)` inline errors with typed domain errors.

### errors.rs

```rust
use cqrs_rust_lib::{define_domain_errors, CqrsError, CqrsErrorCode};
use http::StatusCode;

define_domain_errors! {
    domain: "my_domain",
    prefix: 10,          // unique per domain; 0 = infra, 1 = generic, 10+ = app domains
    errors: {
        //   Variant          => (index, HTTP status,             display string)
        ItemNotFound     => (1, StatusCode::NOT_FOUND,      "ITEM_NOT_FOUND"),
        InvalidInput     => (2, StatusCode::BAD_REQUEST,    "INVALID_INPUT"),
        AlreadyExists    => (3, StatusCode::CONFLICT,       "ALREADY_EXISTS"),
    }
}

impl From<ErrorCode> for CqrsError {
    fn from(e: ErrorCode) -> Self {
        e.error(e.to_string())
    }
}
```

### Usage in command handlers

```rust
use super::errors::ErrorCode;

// Return a domain error:
return Err(ErrorCode::ItemNotFound.error(format!("Item '{}' not found", id)));
```

### Prefix registry (keep unique across the app)

| Prefix | Domain |
|--------|--------|
| 0 | infrastructure (reserved) |
| 1 | generic (reserved) |
| 2-9 | reserved for lib |
| 10+ | application domains |

Internal code = `prefix * 1000 + index`. Example: prefix 10, index 1 -> `10001`.

JSON error format:
```json
{
  "domain": "my_domain",
  "code": "MY_DOMAIN_ITEM_NOT_FOUND",
  "internalCode": 10001,
  "status": 404,
  "message": "Item '42' not found"
}
```

---

## 5. Storage backends & engine wiring

### InMemory (tests / prototypes)

```rust
use cqrs_rust_lib::es::inmemory::InMemoryPersist;
use cqrs_rust_lib::es::EventStoreImpl;
use cqrs_rust_lib::{CqrsCommandEngine, Dispatcher};

let es_store = InMemoryPersist::<MyAggregate>::default();
let event_store = EventStoreImpl::new(es_store);
let effects: Vec<Box<dyn Dispatcher<MyAggregate> + Send + Sync>> = vec![];
let engine = CqrsCommandEngine::new(
    event_store,
    effects,
    (),                            // Services
    Box::new(|e| eprintln!("error: {}", e)),
);
```

### PostgreSQL (feature: `postgres`)

```rust
use cqrs_rust_lib::es::postgres::PostgresPersist;
use std::sync::Arc;
use tokio_postgres::NoTls;

let (client, connection) = tokio_postgres::connect(&pg_uri, NoTls).await?;
tokio::spawn(async move { let _ = connection.await; });
let client = Arc::new(client);

// Create tables (idempotent)
client.batch_execute(&PostgresPersist::<MyAggregate>::schema()).await?;

let es_store = PostgresPersist::<MyAggregate>::from_client(client.clone());
let event_store = EventStoreImpl::new(es_store);
```

### MongoDB (feature: `mongodb`)

```rust
use cqrs_rust_lib::es::mongodb::MongoDBPersist;
use mongodb::Client;

let mongo = Client::with_uri_str(&mongo_uri).await?;
let es_store = MongoDBPersist::<MyAggregate>::new(mongo.database("mydb"));
let event_store = EventStoreImpl::new(es_store);
```

---

## 6. Executing commands

```rust
use cqrs_rust_lib::CqrsContext;

let context = CqrsContext::new(Some("user-id".to_string())).with_next_request_id();

// Create a new aggregate (generates a new UUID internally)
engine.execute_create(CreateCmd::Create { name: "foo".into() }, &context).await?;

// Update an existing aggregate by ID
engine.execute_update("aggregate-id", UpdateCmd::Update { value: 42 }, &context).await?;
```

---

## 7. REST API (feature: `rest`)

```rust
use cqrs_rust_lib::rest::{CQRSWriteRouter, CQRSReadRouter, CQRSAuditLogRouter};
use std::sync::Arc;

let engine = Arc::new(engine);
let repository = Arc::new(/* PostgresFromSnapshotStorage or similar */);

let write_router = CQRSWriteRouter::routes(engine);
let read_router  = CQRSReadRouter::routes(repository, MyAggregate::TYPE);
let audit_router = CQRSAuditLogRouter::routes(event_store, MyAggregate::TYPE);

// nest these under your Axum / OpenApiRouter
```

The `CQRSWriteRouter` exposes:
- `POST /{type}` -> `handle_create`
- `PUT  /{type}/{id}` -> `handle_update`

The `CQRSReadRouter` exposes:
- `GET /{type}` -> list
- `GET /{type}/{id}` -> get by id

---

## 8. CqrsContext in middleware (Axum example)

```rust
use cqrs_rust_lib::CqrsContext;
use axum::{body::Body, extract::Request, middleware::Next, response::Response};
use http::StatusCode;

pub async fn context_middleware(mut req: Request<Body>, next: Next) -> Result<Response, StatusCode> {
    let context = CqrsContext::new(Some("anonymous".to_string())).with_next_request_id();
    req.extensions_mut().insert(context);
    Ok(next.run(req).await)
}
```

---

## 9. Checklist before submitting code

- [ ] `Aggregate` has no command types or handler methods
- [ ] `CommandHandler` has no state fields or `apply`
- [ ] Both impls wrapped with `cqrs_async_trait! { ... }`
- [ ] Struct derives `Default`
- [ ] `define_domain_errors!` prefix is unique in the app
- [ ] `From<ErrorCode> for CqrsError` is implemented
- [ ] Dispatcher trait objects have `+ Send + Sync` bounds
- [ ] `cargo build` passes
- [ ] `cargo clippy --all-features --all-targets -- -D warnings` passes
- [ ] `cargo fmt --all -- --check` passes
