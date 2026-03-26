# Migration Guide: WASM Compatibility & Feature Restructuring

## Overview

This guide describes the migration to the WASM-compatible version of `cqrs-rust-lib`. The changes include:

1. **`tokio` removed from production dependencies** (moved to dev-dependencies)
2. **Feature split**: `utoipa` (schemas only) vs `rest` (Axum routers)
3. **Conditional `Send + Sync`**: the library now compiles for `wasm32` targets
4. **`cqrs_async_trait!` macro**: replaces `#[async_trait::async_trait]`

## Summary of Changes

### Cargo.toml Features

| Before | After |
|--------|-------|
| `features = ["utoipa"]` (includes axum) | `features = ["utoipa"]` (schemas only) |
| — | `features = ["rest"]` (axum + utoipa) |
| `features = ["all"]` = utoipa + mongodb + postgres | `features = ["all"]` = rest + mongodb + postgres |

### Before

```toml
[dependencies]
cqrs-rust-lib = { version = "0.4", features = ["utoipa", "postgres"] }
async-trait = "0.1"
```

```rust
#[async_trait::async_trait]
impl Aggregate for MyAggregate {
    // ...
}

#[async_trait::async_trait]
impl CommandHandler for MyAggregate {
    // ...
}

let effects: Vec<Box<dyn Dispatcher<MyAggregate>>> = vec![
    Box::new(my_dispatcher),
];
let engine = CqrsCommandEngine::new(store, effects, services, error_handler);
```

### After

```toml
[dependencies]
# Use "rest" instead of "utoipa" if you need Axum routers.
# Use "utoipa" alone if you only need ToSchema derives (e.g., for WASM).
cqrs-rust-lib = { version = "0.5", features = ["rest", "postgres"] }
# async-trait is no longer needed as a direct dependency
```

```rust
use cqrs_rust_lib::cqrs_async_trait;

cqrs_async_trait! {
impl Aggregate for MyAggregate {
    // ...
}
}

cqrs_async_trait! {
impl CommandHandler for MyAggregate {
    // ...
}
}

// On native targets, dispatchers require explicit Send + Sync bounds
let effects: Vec<Box<dyn Dispatcher<MyAggregate> + Send + Sync>> = vec![
    Box::new(my_dispatcher),
];
let engine = CqrsCommandEngine::new(store, effects, services, error_handler);
```

## Step-by-Step Migration

### Step 1: Update `Cargo.toml`

Replace the `utoipa` feature with `rest` if you use Axum routers (`CQRSWriteRouter`, `CQRSReadRouter`, `CQRSAuditLogRouter`):

```diff
-cqrs-rust-lib = { version = "0.4", features = ["utoipa", "postgres"] }
+cqrs-rust-lib = { version = "0.5", features = ["rest", "postgres"] }
```

If you only need `ToSchema` / `IntoParams` derives (no Axum routers), keep `utoipa`:

```toml
cqrs-rust-lib = { version = "0.5", features = ["utoipa", "postgres"] }
```

Remove `async-trait` from your direct dependencies (it is now re-exported by the library):

```diff
 [dependencies]
-async-trait = "0.1"
```

### Step 2: Replace `#[async_trait::async_trait]` with `cqrs_async_trait!`

Add the import at the top of files that implement `Aggregate` or `CommandHandler`:

```rust
use cqrs_rust_lib::cqrs_async_trait;
```

Then wrap each trait impl block:

```diff
-#[async_trait::async_trait]
-impl Aggregate for MyAggregate {
+cqrs_async_trait! {
+impl Aggregate for MyAggregate {
     const TYPE: &'static str = "my_aggregate";
     // ... all methods unchanged ...
 }
+}

-#[async_trait::async_trait]
-impl CommandHandler for MyAggregate {
+cqrs_async_trait! {
+impl CommandHandler for MyAggregate {
     type CreateCommand = CreateCmd;
     // ... all methods unchanged ...
 }
+}
```

> **Note:** The closing `}` of the macro is **after** the impl block's closing `}`. Method signatures and bodies remain unchanged.

### Step 3: Add `Send + Sync` to dispatcher trait objects

On native targets, `Dispatcher` no longer has `Send + Sync` as supertraits (they are conditional). You must add them explicitly on trait objects:

```diff
-let effects: Vec<Box<dyn Dispatcher<MyAggregate>>> = vec![...];
+let effects: Vec<Box<dyn Dispatcher<MyAggregate> + Send + Sync>> = vec![...];
```

The same applies to `CqrsCommandEngine::append_dispatcher`:

```diff
-engine.append_dispatcher(Box::new(my_dispatcher));
+engine.append_dispatcher(Box::new(my_dispatcher) as Box<dyn Dispatcher<_> + Send + Sync>);
```

### Step 4: (Optional) WASM target

If you target `wasm32`, no additional changes are needed. The library automatically:
- Removes `Send + Sync` bounds from all traits
- Uses `async_trait(?Send)` for non-Send futures
- Provides `DynEventStore<A>` and `DynStorage<V, Q>` without `Send + Sync`

For WASM, ensure your `Cargo.toml` does not enable `rest`, `postgres`, or `mongodb` features (they depend on native-only crates).

```toml
# WASM-compatible configuration
cqrs-rust-lib = { version = "0.5" }
# Or with schema support:
cqrs-rust-lib = { version = "0.5", features = ["utoipa"] }
```

## Feature Matrix

| Feature | Includes | WASM-compatible |
|---------|----------|-----------------|
| *(default)* | Core CQRS traits, InMemory backend | Yes |
| `utoipa` | + `ToSchema` / `IntoParams` derives | Yes |
| `rest` | + `utoipa` + Axum routers | No (requires tokio) |
| `postgres` | + PostgreSQL backend | No (requires tokio) |
| `mongodb` | + MongoDB backend | No (requires tokio) |
| `all` | `rest` + `postgres` + `mongodb` | No |

## New Public API

| Item | Description |
|------|-------------|
| `cqrs_async_trait!` | Macro replacing `#[async_trait::async_trait]` |
| `MaybeSend` | Trait alias: `Send` on native, no-op on WASM |
| `MaybeSync` | Trait alias: `Sync` on native, no-op on WASM |

These are primarily useful for library authors building on top of `cqrs-rust-lib`. Application code typically only needs `cqrs_async_trait!`.

## Design Decision: `macro_rules!` vs proc macro attribute

The `cqrs_async_trait!` macro is implemented as a `macro_rules!` rather than a `#[proc_macro_attribute]`. Here is why.

### What a proc macro approach would look like

A proc macro attribute (`#[cqrs_async_trait]`) could read `CARGO_CFG_TARGET_ARCH` at compile time and emit either `#[async_trait]` or `#[async_trait(?Send)]`:

```rust
// hypothetical proc-macro crate
#[proc_macro_attribute]
pub fn cqrs_async_trait(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let target = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    if target == "wasm32" {
        quote! { #[async_trait::async_trait(?Send)] #item }
    } else {
        quote! { #[async_trait::async_trait] #item }
    }
}
```

This works (the compiler re-expands attributes in the output), but it introduces significant costs.

### Trade-off comparison

| | `macro_rules!` (chosen) | `#[proc_macro_attribute]` |
|---|---|---|
| **Extra crate** | None | Required (`proc-macro = true` crate) |
| **Extra deps** | None | `syn`, `quote`, `proc-macro2` |
| **Compile time** | Zero overhead | Compiles an additional proc-macro crate |
| **Maintenance** | 5 lines, trivial | ~30 lines + proc-macro boilerplate |
| **Consumer deps** | None (`$crate::__async_trait` re-export) | Either `async-trait` direct dep or same re-export trick |
| **Syntax** | `cqrs_async_trait! { impl ... }` | `#[cqrs_async_trait] impl ...` |

### Why `macro_rules!` wins

1. **No additional crate**: proc macros must live in a dedicated `proc-macro = true` crate. This would add a `cqrs-rust-lib-macros` crate to the workspace just for 5 lines of logic.
2. **No heavy dependencies**: `syn`/`quote`/`proc-macro2` add significant compile time. The `macro_rules!` approach has zero compile-time cost.
3. **Transparent**: the macro expands to exactly `#[async_trait]` or `#[async_trait(?Send)]` with `cfg_attr`. No hidden transformation, easy to reason about.
4. **Re-export trick**: `async-trait` is re-exported as `$crate::__async_trait` inside the macro. Consumers do not need `async-trait` as a direct dependency regardless of the approach.

The only advantage of the proc macro is syntax (`#[attr]` vs `macro! { ... }`). This does not justify the added complexity.
