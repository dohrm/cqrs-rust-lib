# CLAUDE.md - cqrs-rust-lib

## Project Overview

Rust CQRS/Event Sourcing library with split Aggregate/CommandHandler traits, structured domain errors, and pluggable storage backends (InMemory, PostgreSQL, MongoDB).

## Build & Test

```bash
cargo build                      # lib only
cargo build --features all       # all features (utoipa, postgres, mongodb)
cargo test                       # lib tests (13 unit + doc tests)
cargo build -p bank              # bank example (MongoDB)
cargo build -p todolist          # todolist example (PostgreSQL)
cargo test -p todolist           # integration tests
cargo doc --no-deps              # generate docs
```

## Architecture

### Trait Hierarchy

- `Aggregate`: State, event application, identity (`TYPE`, `Event`, `Error`, `apply`, `aggregate_id`)
- `CommandHandler: Aggregate`: Command processing (`CreateCommand`, `UpdateCommand`, `Services`, `handle_create`, `handle_update`)

Both traits are implemented on the same struct. `Aggregate` owns domain state, `CommandHandler` owns business logic.

### Error System

- `CqrsError`: Unified error struct (domain, code, internal_code, status, message, details)
- `CqrsErrorCode` trait: Defines domain/prefix/index/http_status per error variant
- `define_domain_errors!` macro: Generates `ErrorCode` enum implementing `CqrsErrorCode`
- `InfrastructureErrorCode` (prefix 0): Database, serialization, concurrency, etc.
- `GenericErrorCode` (prefix 1): NotFound, ValidationFailed, Conflict, etc.
- Application domains use prefix 10+
- Convenience constructors: `CqrsError::not_found()`, `::validation()`, `::database_error()`, `::from_status()`, etc.
- Engine has `A::Error: Into<CqrsError>` bound so domain errors pass through without stringification

### Key Files

```
src/aggregate.rs          # Aggregate + CommandHandler traits
src/engine.rs             # CqrsCommandEngine (command execution orchestrator)
src/errors.rs             # CqrsError, CqrsErrorCode, define_domain_errors!, InfrastructureErrorCode, GenericErrorCode
src/event.rs              # Event trait + EventEnvelope
src/event_store.rs        # EventStore trait (load/initialize/commit)
src/es/storage.rs         # EventStoreStorage trait (low-level persistence)
src/es/impl.rs            # EventStoreImpl (generic EventStore implementation)
src/es/inmemory.rs        # InMemoryPersist
src/es/postgres.rs        # PostgresPersist (feature: postgres)
src/es/mongodb.rs         # MongoDBPersist (feature: mongodb)
src/read/storage.rs       # ViewStorage + SnapshotStorage traits
src/denormalizer.rs       # Dispatcher trait
src/rest/write_router.rs  # CQRSWriteRouter (feature: utoipa)
src/rest/read_router.rs   # CQRSReadRouter (feature: utoipa)
src/context.rs            # CqrsContext
```

### Feature Flags

- `utoipa`: REST routers, OpenAPI, Axum integration (adds ToSchema bounds)
- `postgres`: PostgresPersist + read utilities
- `mongodb`: MongoDBPersist
- `mcp`: MCP server (experimental)
- `all`: utoipa + postgres + mongodb

## Conventions

- All public traits use `#[async_trait::async_trait]`
- Feature-gated utoipa derives: `#[cfg_attr(feature = "utoipa", derive(ToSchema))]`
- Error results: `Result<_, CqrsError>` everywhere in storage/engine/dispatchers
- Domain errors in examples use `define_domain_errors!` macro + `From<ErrorCode> for CqrsError`
- Aggregate `type Error` is `CqrsError` (or any type implementing `Into<CqrsError>`)
- Event types implement `Event` trait with `fn event_type(&self) -> String`
- `EventStoreStorage` is the low-level trait; `EventStore` is the high-level trait used by the engine
- `DynEventStore<A>` = `Box<dyn EventStore<A>>` (dyn-compatible)

## Examples

- `example/bank/`: MongoDB, domain errors (prefix 10), Account aggregate
- `example/todolist/`: PostgreSQL, domain errors (prefix 20), TodoList aggregate, REST API + Swagger

## Migration Guides

- `docs/migration_guide/split_aggregate.md`: Legacy single-trait -> Aggregate + CommandHandler
- `docs/migration_guide/domain_errors.md`: std::io::Error -> CqrsError + domain error codes
