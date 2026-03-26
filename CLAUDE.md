# CLAUDE.md - cqrs-rust-lib

## Project Overview

Rust CQRS/Event Sourcing library with split Aggregate/CommandHandler traits, structured domain errors, and pluggable storage backends (InMemory, PostgreSQL, MongoDB). WASM-compatible by default (no tokio in production deps).

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
src/read/storage.rs       # ViewStorage + SnapshotStorage traits
src/denormalizer.rs       # Dispatcher trait
src/rest/write_router.rs  # CQRSWriteRouter (feature: rest)
src/rest/read_router.rs   # CQRSReadRouter (feature: rest)
src/context.rs            # CqrsContext
```

### Feature Flags

- `utoipa`: ToSchema/IntoParams derives only (WASM-compatible)
- `rest`: Axum routers + OpenAPI (implies `utoipa`, native only)
- `postgres`: PostgresPersist + read utilities (native only)
- `mongodb`: MongoDBPersist (native only)
- `mcp`: MCP server (experimental)
- `all`: rest + postgres + mongodb

## Conventions

- All public async traits use `cqrs_async_trait! { ... }` (NOT `#[async_trait::async_trait]`)
- `async-trait` is re-exported as `__async_trait` for the macro; consumers do not need it as a direct dependency
- Trait bounds use `MaybeSend + MaybeSync` instead of `Send + Sync` in core (non-feature-gated) code
- Dispatcher/EventStore/Storage `dyn` type aliases use `cfg(target_arch)` for conditional `+ Send + Sync`
- Feature-gated code (postgres, mongodb) keeps explicit `Send + Sync` bounds (never compiled for WASM)
- Feature-gated utoipa derives: `#[cfg_attr(feature = "utoipa", derive(ToSchema))]`
- Error results: `Result<_, CqrsError>` everywhere in storage/engine/dispatchers
- Domain errors in examples use `define_domain_errors!` macro + `From<ErrorCode> for CqrsError`
- Aggregate `type Error` is `CqrsError` (or any type implementing `Into<CqrsError>`)
- Event types implement `Event` trait with `fn event_type(&self) -> String`
- `EventStoreStorage` is the low-level trait; `EventStore` is the high-level trait used by the engine
- `DynEventStore<A>` = `Arc<dyn EventStore<A> + Send + Sync + 'static>` (native) / `Arc<dyn EventStore<A> + 'static>` (WASM)

## Quality Checks

After completing any task, always run:

```bash
cargo fmt --all -- --check                                  # formatting
cargo clippy --all-features --all-targets -- -D warnings    # lints
```

## Examples

- `example/bank/`: MongoDB, domain errors (prefix 10), Account aggregate
- `example/todolist/`: PostgreSQL, domain errors (prefix 20), TodoList aggregate, REST API + Swagger

## Migration Guides

- `docs/migration_guide/split_aggregate.md`: Legacy single-trait -> Aggregate + CommandHandler
- `docs/migration_guide/domain_errors.md`: std::io::Error -> CqrsError + domain error codes
- `docs/migration_guide/wasm_compat.md`: WASM compatibility, feature restructuring, cqrs_async_trait! migration
