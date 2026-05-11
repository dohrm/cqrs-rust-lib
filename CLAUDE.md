# CLAUDE.md - cqrs-rust-lib

## Project Overview

Rust CQRS/Event Sourcing library with split Aggregate/CommandHandler traits, structured domain errors, and pluggable storage backends (InMemory, PostgreSQL, MongoDB, SurrealDB). WASM-compatible by default (no tokio in production deps).

## Build & Test

```bash
cargo build                      # lib only (WASM-compatible)
cargo build --features all       # all features (rest, postgres, mongodb)
cargo build --features utoipa    # schemas only (WASM-compatible)
cargo build --features rest      # axum routers + schemas (native only)
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

### WASM Compatibility

- `MaybeSend` / `MaybeSync`: Conditional trait aliases (`Send`/`Sync` on native, no-op on WASM)
- `cqrs_async_trait!`: Macro wrapping `#[async_trait]` (Send futures on native, `?Send` on WASM)
- `DynEventStore<A>`, `DynStorage<V, Q>`, `EventStream<A>`: Conditional `+ Send + Sync` via `cfg(target_arch)`
- `CqrsCommandEngine` fields (`dispatchers`, `error_handler`): Conditional `+ Send + Sync` via `cfg(target_arch)`

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
src/lib.rs                # cqrs_async_trait! macro, re-exports
src/wasm_compat.rs        # MaybeSend, MaybeSync conditional traits
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
src/es/surrealdb.rs       # SurrealDBPersist (feature: surrealdb)
src/read/storage.rs       # ViewStorage + SnapshotStorage traits
src/read/memory.rs        # InMemoryViewStore
src/read/postgres.rs      # PostgresStorage, PostgresFromSnapshotStorage, QueryBuilder (feature: postgres)
src/read/mongodb.rs       # MongoDbStorage, MongoDBFromSnapshotStorage, QueryBuilder (feature: mongodb)
src/read/surrealdb.rs     # SurrealDBStorage, SurrealDBFromSnapshotStorage, QueryBuilder (feature: surrealdb)
src/prelude/mod.rs        # Backend preludes: inmemory, postgres, mongodb, surrealdb
src/denormalizer.rs       # Dispatcher trait
src/rest/write_router.rs  # CQRSWriteRouter (feature: rest)
src/rest/read_router.rs   # CQRSReadRouter (feature: rest)
src/context.rs            # CqrsContext
```

### Backend Preludes

Each backend exposes canonical type aliases via `cqrs_rust_lib::prelude::<backend>`:

| Alias | inmemory | postgres | mongodb | surrealdb |
|---|---|---|---|---|
| `EventStorePersist` | ✓ | ✓ | ✓ | ✓ |
| `ReadStorage` | — | ✓ | ✓ | ✓ |
| `FromSnapshotStorage` | — | ✓ | ✓ | ✓ |
| `QueryBuilder` | — | ✓ | ✓ | ✓ |

Swapping the backend requires changing a single import:
```rust
use cqrs_rust_lib::prelude::postgres as db;  // or mongodb, surrealdb, inmemory
// db::EventStorePersist, db::ReadStorage, db::FromSnapshotStorage, db::QueryBuilder
```

The `QueryBuilder` trait is backend-specific (SQL params vs BSON vs SurrealQL) — it must be reimplemented when swapping backends. Original concrete names (`PostgresPersist`, `MongoDbStorage`, etc.) remain available from their respective modules.

### Feature Flags

- `utoipa`: ToSchema/IntoParams derives only (WASM-compatible)
- `rest`: Axum routers + OpenAPI (implies `utoipa`, native only)
- `postgres`: PostgresPersist + read utilities (native only)
- `mongodb`: MongoDBPersist (native only)
- `surrealdb`: SurrealDBPersist + read utilities (native only)
- `mcp`: MCP server (experimental)
- `all`: rest + postgres + mongodb

## Conventions

- All public async traits use `cqrs_async_trait! { ... }` (NOT `#[async_trait::async_trait]`)
- `async-trait` is re-exported as `__async_trait` for the macro; consumers do not need it as a direct dependency
- Trait bounds use `MaybeSend + MaybeSync` instead of `Send + Sync` in core (non-feature-gated) code
- Dispatcher/EventStore/Storage `dyn` type aliases use `cfg(target_arch)` for conditional `+ Send + Sync`
- Feature-gated code (postgres, mongodb, surrealdb) keeps explicit `Send + Sync` bounds (never compiled for WASM)
- Feature-gated utoipa derives: `#[cfg_attr(feature = "utoipa", derive(ToSchema))]`
- Error results: `Result<_, CqrsError>` everywhere in storage/engine/dispatchers
- Domain errors in examples use `define_domain_errors!` macro + `From<ErrorCode> for CqrsError`
- Aggregate `type Error` is `CqrsError` (or any type implementing `Into<CqrsError>`)
- Event types implement `Event` trait with `fn event_type(&self) -> String`
- `EventStoreStorage` is the low-level trait; `EventStore` is the high-level trait used by the engine
- `DynEventStore<A>` = `Arc<dyn EventStore<A> + Send + Sync + 'static>` (native) / `Arc<dyn EventStore<A> + 'static>` (WASM)
- Read `QueryBuilder` trait is named identically across all backends; each module has its own incompatible signature (SQL vs BSON vs SurrealQL)
- Prefer `use cqrs_rust_lib::prelude::<backend> as db` in application wiring code; use module-specific paths only when accessing backend-specific APIs (e.g., `PgPool`, `schema()`)

## Quality Checks

After completing any task, always run:

```bash
cargo fmt --all -- --check                                  # formatting
cargo clippy --all-features --all-targets -- -D warnings    # lints
```

## Examples

- `example/bank/`: MongoDB, domain errors (prefix 10), Account aggregate — uses `prelude::mongodb as db`
- `example/todolist/`: PostgreSQL, domain errors (prefix 20), TodoList aggregate, REST API + Swagger — uses `prelude::postgres as db`

## Migration Guides

- `docs/migration_guide/split_aggregate.md`: Legacy single-trait -> Aggregate + CommandHandler
- `docs/migration_guide/domain_errors.md`: std::io::Error -> CqrsError + domain error codes
- `docs/migration_guide/wasm_compat.md`: WASM compatibility, feature restructuring, cqrs_async_trait! migration
