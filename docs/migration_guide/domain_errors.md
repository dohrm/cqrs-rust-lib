# Migration Guide: Domain Error Codes

## Overview

This guide describes the migration from `std::io::Error` (or any generic error type) to the structured `CqrsError` system with domain-specific error codes.

The new error system provides:

- **Structured error codes** per domain (e.g., `ACCOUNT_INSUFFICIENT_FUNDS`, `TODOLIST_TODO_NOT_FOUND`)
- **Internal numeric codes** for support/debugging (e.g., `10001`, `20001`)
- **Unified JSON format** for API responses
- **HTTP status mapping** per error variant
- **`define_domain_errors!` macro** for zero-boilerplate error domain definition

## Summary of Changes

### Before (Legacy)

```rust
use cqrs_rust_lib::Aggregate;

#[async_trait::async_trait]
impl Aggregate for MyAggregate {
    type Error = std::io::Error;

    fn error(_status: StatusCode, details: &str) -> Self::Error {
        std::io::Error::other(details.to_string())
    }
    // ...
}

// In command handlers, errors are generic strings:
async fn handle_update(&self, command: ...) -> Result<Vec<Self::Event>, Self::Error> {
    if invalid {
        return Err(std::io::Error::other("Something went wrong"));
    }
    // ...
}
```

### After (New)

```rust
use cqrs_rust_lib::{Aggregate, CqrsError, CqrsErrorCode, define_domain_errors};

// 1. Define domain errors
define_domain_errors! {
    domain: "my_domain",
    prefix: 10,
    errors: {
        InvalidInput => (1, StatusCode::BAD_REQUEST, "INVALID_INPUT"),
        NotAllowed   => (2, StatusCode::FORBIDDEN, "NOT_ALLOWED"),
        ResourceGone => (3, StatusCode::GONE, "RESOURCE_GONE"),
    }
}

// 2. Implement From<ErrorCode> for CqrsError
impl From<ErrorCode> for CqrsError {
    fn from(e: ErrorCode) -> Self {
        e.error(e.to_string())
    }
}

// 3. Use CqrsError as the aggregate error type
#[async_trait::async_trait]
impl Aggregate for MyAggregate {
    type Error = CqrsError;

    fn error(status: StatusCode, details: &str) -> Self::Error {
        CqrsError::from_status(status, details)
    }
    // ...
}

// 4. In command handlers, return structured domain errors:
async fn handle_update(&self, command: ...) -> Result<Vec<Self::Event>, Self::Error> {
    if invalid {
        return Err(ErrorCode::InvalidInput.error("Field 'name' is required"));
    }
    // ...
}
```

## Step-by-Step Migration

### Step 1: Add `thiserror` Dependency

The `define_domain_errors!` macro uses `thiserror` internally. Add it to your `Cargo.toml`:

```toml
[dependencies]
thiserror = "2"
```

### Step 2: Create an Error Module

Create a file `errors.rs` next to your aggregate (e.g., `src/my_domain/errors.rs`):

```rust
use cqrs_rust_lib::{define_domain_errors, CqrsError, CqrsErrorCode};
use http::StatusCode;

define_domain_errors! {
    domain: "my_domain",
    prefix: 10,                // Unique prefix for your domain (0-65)
    errors: {
        //   Variant       => (index, HTTP status,          display string)
        InvalidInput   => (1, StatusCode::BAD_REQUEST,   "INVALID_INPUT"),
        NotAllowed     => (2, StatusCode::FORBIDDEN,     "NOT_ALLOWED"),
        ResourceGone   => (3, StatusCode::GONE,          "RESOURCE_GONE"),
    }
}

impl From<ErrorCode> for CqrsError {
    fn from(e: ErrorCode) -> Self {
        e.error(e.to_string())
    }
}
```

**Key parameters:**
- `domain`: String identifier for the domain (used in JSON `domain` field)
- `prefix`: Unique numeric prefix (0-65). Internal codes = `prefix * 1000 + index`
- `errors`: Variants with `(index, StatusCode, display_string)`

Register the module in your `mod.rs`:

```rust
pub mod errors;
```

### Step 3: Update the Aggregate Error Type

**Before:**
```rust
impl Aggregate for MyAggregate {
    type Error = std::io::Error;

    fn error(_status: StatusCode, details: &str) -> Self::Error {
        std::io::Error::other(details.to_string())
    }
}
```

**After:**
```rust
use cqrs_rust_lib::{CqrsError, CqrsErrorCode};

impl Aggregate for MyAggregate {
    type Error = CqrsError;

    fn error(status: StatusCode, details: &str) -> Self::Error {
        CqrsError::from_status(status, details)
    }
}
```

### Step 4: Add Domain Validation in Command Handlers

Replace generic errors with domain-specific ones:

**Before:**
```rust
async fn handle_update(&self, command: UpdateCommand, ...) -> Result<Vec<Self::Event>, Self::Error> {
    match command {
        UpdateCommand::Withdraw { amount } => {
            // No validation, or generic error:
            Ok(vec![Events::Withdrawn { amount }])
        }
    }
}
```

**After:**
```rust
use super::errors::ErrorCode;

async fn handle_update(&self, command: UpdateCommand, ...) -> Result<Vec<Self::Event>, Self::Error> {
    match command {
        UpdateCommand::Withdraw { amount } => {
            if self.balance < amount {
                return Err(ErrorCode::InsufficientFunds.error(
                    format!("Cannot withdraw {}, balance is {}", amount, self.balance)
                ));
            }
            Ok(vec![Events::Withdrawn { amount }])
        }
    }
}
```

### Step 5: Update Imports

Update your aggregate file imports:

**Before:**
```rust
use cqrs_rust_lib::{Aggregate, CommandHandler, CqrsContext};
use std::io::ErrorKind;
```

**After:**
```rust
use cqrs_rust_lib::{Aggregate, CommandHandler, CqrsContext, CqrsError, CqrsErrorCode};
use super::errors::ErrorCode;
```

### Step 6: Verify

```bash
cargo build
cargo test
```

## Complete Examples

See the example projects for complete implementations:

- **Bank** (`example/bank/src/account/errors.rs`): Domain errors for insufficient funds, invalid amounts, account closed
- **TodoList** (`example/todolist/src/todolist/errors.rs`): Domain errors for todo not found, already resolved, empty title

## API Response Format

When a domain error is returned, the API response is:

```json
{
  "domain": "account",
  "code": "ACCOUNT_INSUFFICIENT_FUNDS",
  "internalCode": 10001,
  "status": 400,
  "message": "Cannot withdraw 500, balance is 200"
}
```

The `internalCode` is computed as `prefix * 1000 + index` (e.g., `10 * 1000 + 1 = 10001`).

## Prefix Registry

Each domain must have a unique prefix. Here is the convention:

| Prefix | Domain |
|--------|--------|
| 0 | `infrastructure` (reserved) |
| 1 | `generic` (reserved) |
| 2-9 | Reserved for future cqrs-rust-lib use |
| 10+ | Application domains |

## Checklist for Migration

- [ ] Add `thiserror = "2"` to `Cargo.toml`
- [ ] Create `errors.rs` with `define_domain_errors!` macro
- [ ] Register `pub mod errors;` in `mod.rs`
- [ ] Add `impl From<ErrorCode> for CqrsError`
- [ ] Change `type Error = std::io::Error` to `type Error = CqrsError` in `impl Aggregate`
- [ ] Update `fn error(...)` to use `CqrsError::from_status(status, details)`
- [ ] Replace generic errors with `ErrorCode::Variant.error("message")` in command handlers
- [ ] Import `CqrsErrorCode` trait (needed for `.error()` method)
- [ ] Import domain `ErrorCode` in aggregate file
- [ ] Run `cargo build` to verify compilation
- [ ] Run tests to verify behavior

## AI Agent Instructions

For AI agents performing this migration automatically:

### Detection

Search for aggregates using generic error types:

```bash
grep -rn "type Error = std::io::Error" --include="*.rs"
grep -rn "type Error = " --include="*.rs" | grep -v "CqrsError"
```

### Context Gathering

Before migrating, the agent must:

1. **Read the aggregate file** to understand the domain (fields, events, commands)
2. **Identify all command handlers** and their business rules
3. **List existing error sites** (places that return `Err(...)`)
4. **Choose a unique prefix** (check existing `define_domain_errors!` for used prefixes)

### Transformation Algorithm

1. **Create `errors.rs`** in the same module as the aggregate:
   - Choose a `domain` name matching the aggregate context (lowercase, e.g., `"account"`, `"todolist"`)
   - Choose a unique `prefix` (10+ for application domains)
   - Define error variants based on the domain's business rules:
     - Validation errors → `StatusCode::BAD_REQUEST`
     - Not found errors → `StatusCode::NOT_FOUND`
     - Conflict/duplicate errors → `StatusCode::CONFLICT`
     - State errors (closed, expired) → `StatusCode::GONE` or `StatusCode::BAD_REQUEST`
   - Add `impl From<ErrorCode> for CqrsError`

2. **Update `mod.rs`** to register the error module:
   ```rust
   pub mod errors;
   ```

3. **Update the aggregate**:
   - Change `type Error = std::io::Error` → `type Error = CqrsError`
   - Change `fn error(...)` → `CqrsError::from_status(status, details)`
   - Add imports: `CqrsError`, `CqrsErrorCode`, and `ErrorCode`
   - Remove `use std::io::ErrorKind` or similar imports

4. **Add domain validation** in command handlers:
   - Analyze each command and identify business invariants
   - Add validation checks before event emission
   - Return `ErrorCode::Variant.error("descriptive message")` for violations
   - Include relevant context in error messages (IDs, amounts, etc.)

5. **Update `Cargo.toml`**:
   - Add `thiserror = "2"` to dependencies

### Error Variant Design Guidelines

For the AI agent choosing error variants:

- **Name variants after the business rule violation**, not the HTTP status (e.g., `InsufficientFunds` not `BadRequest`)
- **One variant per distinct business error** (don't reuse a generic variant for different rules)
- **Error messages should include context**: IDs, values, limits
- **Common patterns**:
  - `EntityNotFound` → `NOT_FOUND` (404)
  - `InvalidField` / `EmptyField` → `BAD_REQUEST` (400)
  - `EntityAlreadyExists` / `DuplicateSlug` → `CONFLICT` (409)
  - `EntityClosed` / `EntityExpired` → `GONE` (410) or `BAD_REQUEST` (400)
  - `InsufficientX` / `LimitExceeded` → `BAD_REQUEST` (400)
  - `NotAllowed` / `OperationForbidden` → `FORBIDDEN` (403)

### Validation

After transformation:

1. `cargo build` must succeed
2. `cargo test` must pass
3. Verify JSON response format by inspecting the error serialization
4. Check that all error variants have unique `(prefix, index)` pairs

### Example Prompt for AI Migration

```
Migrate the aggregate in `src/my_domain/aggregate.rs` from `std::io::Error` to domain-specific
`CqrsError` codes:

1. Read the aggregate to understand the domain (fields, events, commands, business rules)
2. Create `src/my_domain/errors.rs` using `define_domain_errors!` macro with prefix N
3. Register `pub mod errors;` in `src/my_domain/mod.rs`
4. Update the aggregate:
   - `type Error = CqrsError`
   - `fn error(status, details) -> CqrsError::from_status(status, details)`
   - Add domain validation in command handlers
5. Add `thiserror = "2"` to Cargo.toml if not present
6. Verify with `cargo build && cargo test`
```
