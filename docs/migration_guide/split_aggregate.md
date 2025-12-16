# Migration Guide: Aggregate / CommandHandler Split

## Overview

This guide describes the migration from the legacy single `Aggregate` trait to the new split design with two separate traits:

- **`Aggregate`**: Handles event processing and state management
- **`CommandHandler`**: Handles command processing to produce events

This separation follows the Single Responsibility Principle and enables more flexible architectures (e.g., aggregates that can be reconstituted from events without command handling capabilities).

## Summary of Changes

### Before (Legacy)

```rust
#[async_trait::async_trait]
impl Aggregate for MyAggregate {
    const TYPE: &'static str = "my_aggregate";

    type CreateCommand = CreateCmd;
    type UpdateCommand = UpdateCmd;
    type Event = MyEvent;
    type Services = MyServices;
    type Error = MyError;

    fn aggregate_id(&self) -> String { ... }
    fn with_aggregate_id(self, id: String) -> Self { ... }

    async fn handle_create(...) -> Result<Vec<Self::Event>, Self::Error> { ... }
    async fn handle_update(...) -> Result<Vec<Self::Event>, Self::Error> { ... }

    fn apply(&mut self, event: Self::Event) -> Result<(), Self::Error> { ... }
    fn error(status: StatusCode, details: &str) -> Self::Error { ... }
}
```

### After (New)

```rust
#[async_trait::async_trait]
impl Aggregate for MyAggregate {
    const TYPE: &'static str = "my_aggregate";

    type Event = MyEvent;
    type Error = MyError;

    fn aggregate_id(&self) -> String { ... }
    fn with_aggregate_id(self, id: String) -> Self { ... }

    fn apply(&mut self, event: Self::Event) -> Result<(), Self::Error> { ... }
    fn error(status: StatusCode, details: &str) -> Self::Error { ... }
}

#[async_trait::async_trait]
impl CommandHandler for MyAggregate {
    type CreateCommand = CreateCmd;
    type UpdateCommand = UpdateCmd;
    type Services = MyServices;

    async fn handle_create(...) -> Result<Vec<Self::Event>, Self::Error> { ... }
    async fn handle_update(...) -> Result<Vec<Self::Event>, Self::Error> { ... }
}
```

## Step-by-Step Migration

### Step 1: Update Imports

Add `CommandHandler` to your imports from `cqrs_rust_lib`.

**Before:**
```rust
use cqrs_rust_lib::{Aggregate, CqrsContext, ...};
```

**After:**
```rust
use cqrs_rust_lib::{Aggregate, CommandHandler, CqrsContext, ...};
```

### Step 2: Split the Aggregate Implementation

#### 2.1 Keep in `impl Aggregate`:
- `const TYPE`
- `type Event`
- `type Error`
- `fn aggregate_id(&self)`
- `fn with_aggregate_id(self, id: String)`
- `fn apply(&mut self, event: Self::Event)`
- `fn error(status: StatusCode, details: &str)`

#### 2.2 Move to `impl CommandHandler`:
- `type CreateCommand`
- `type UpdateCommand`
- `type Services`
- `async fn handle_create(...)`
- `async fn handle_update(...)`

### Step 3: Add the CommandHandler Implementation Block

Create a new `impl CommandHandler for YourAggregate` block with the `#[async_trait::async_trait]` attribute.

### Step 4: Verify Compilation

```bash
cargo build
```

## Complete Example

### Before Migration

```rust
use cqrs_rust_lib::{Aggregate, CqrsContext};
use http::StatusCode;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub owner: String,
    pub balance: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CreateCommand {
    Open { owner: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UpdateCommand {
    Deposit { amount: f64 },
    Withdraw { amount: f64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AccountEvent {
    Opened { owner: String },
    Deposited { amount: f64 },
    Withdrawn { amount: f64 },
}

impl cqrs_rust_lib::Event for AccountEvent {
    fn event_type(&self) -> String {
        match self {
            Self::Opened { .. } => "Opened".into(),
            Self::Deposited { .. } => "Deposited".into(),
            Self::Withdrawn { .. } => "Withdrawn".into(),
        }
    }
}

#[async_trait::async_trait]
impl Aggregate for Account {
    const TYPE: &'static str = "account";

    type CreateCommand = CreateCommand;
    type UpdateCommand = UpdateCommand;
    type Event = AccountEvent;
    type Services = ();
    type Error = std::io::Error;

    fn aggregate_id(&self) -> String {
        self.id.clone()
    }

    fn with_aggregate_id(mut self, id: String) -> Self {
        self.id = id;
        self
    }

    async fn handle_create(
        &self,
        command: Self::CreateCommand,
        _services: &Self::Services,
        _context: &CqrsContext,
    ) -> Result<Vec<Self::Event>, Self::Error> {
        match command {
            CreateCommand::Open { owner } => Ok(vec![AccountEvent::Opened { owner }]),
        }
    }

    async fn handle_update(
        &self,
        command: Self::UpdateCommand,
        _services: &Self::Services,
        _context: &CqrsContext,
    ) -> Result<Vec<Self::Event>, Self::Error> {
        match command {
            UpdateCommand::Deposit { amount } => Ok(vec![AccountEvent::Deposited { amount }]),
            UpdateCommand::Withdraw { amount } => Ok(vec![AccountEvent::Withdrawn { amount }]),
        }
    }

    fn apply(&mut self, event: Self::Event) -> Result<(), Self::Error> {
        match event {
            AccountEvent::Opened { owner } => self.owner = owner,
            AccountEvent::Deposited { amount } => self.balance += amount,
            AccountEvent::Withdrawn { amount } => self.balance -= amount,
        }
        Ok(())
    }

    fn error(_status: StatusCode, details: &str) -> Self::Error {
        std::io::Error::other(details.to_string())
    }
}
```

### After Migration

```rust
use cqrs_rust_lib::{Aggregate, CommandHandler, CqrsContext};
use http::StatusCode;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Account {
    pub id: String,
    pub owner: String,
    pub balance: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CreateCommand {
    Open { owner: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UpdateCommand {
    Deposit { amount: f64 },
    Withdraw { amount: f64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AccountEvent {
    Opened { owner: String },
    Deposited { amount: f64 },
    Withdrawn { amount: f64 },
}

impl cqrs_rust_lib::Event for AccountEvent {
    fn event_type(&self) -> String {
        match self {
            Self::Opened { .. } => "Opened".into(),
            Self::Deposited { .. } => "Deposited".into(),
            Self::Withdrawn { .. } => "Withdrawn".into(),
        }
    }
}

// ============================================================
// AGGREGATE TRAIT: Event processing and state management
// ============================================================
#[async_trait::async_trait]
impl Aggregate for Account {
    const TYPE: &'static str = "account";

    type Event = AccountEvent;
    type Error = std::io::Error;

    fn aggregate_id(&self) -> String {
        self.id.clone()
    }

    fn with_aggregate_id(mut self, id: String) -> Self {
        self.id = id;
        self
    }

    fn apply(&mut self, event: Self::Event) -> Result<(), Self::Error> {
        match event {
            AccountEvent::Opened { owner } => self.owner = owner,
            AccountEvent::Deposited { amount } => self.balance += amount,
            AccountEvent::Withdrawn { amount } => self.balance -= amount,
        }
        Ok(())
    }

    fn error(_status: StatusCode, details: &str) -> Self::Error {
        std::io::Error::other(details.to_string())
    }
}

// ============================================================
// COMMAND HANDLER TRAIT: Command processing
// ============================================================
#[async_trait::async_trait]
impl CommandHandler for Account {
    type CreateCommand = CreateCommand;
    type UpdateCommand = UpdateCommand;
    type Services = ();

    async fn handle_create(
        &self,
        command: Self::CreateCommand,
        _services: &Self::Services,
        _context: &CqrsContext,
    ) -> Result<Vec<Self::Event>, Self::Error> {
        match command {
            CreateCommand::Open { owner } => Ok(vec![AccountEvent::Opened { owner }]),
        }
    }

    async fn handle_update(
        &self,
        command: Self::UpdateCommand,
        _services: &Self::Services,
        _context: &CqrsContext,
    ) -> Result<Vec<Self::Event>, Self::Error> {
        match command {
            UpdateCommand::Deposit { amount } => Ok(vec![AccountEvent::Deposited { amount }]),
            UpdateCommand::Withdraw { amount } => Ok(vec![AccountEvent::Withdrawn { amount }]),
        }
    }
}
```

## Checklist for Migration

Use this checklist to ensure complete migration:

- [ ] Update import statement to include `CommandHandler`
- [ ] Remove `type CreateCommand` from `impl Aggregate`
- [ ] Remove `type UpdateCommand` from `impl Aggregate`
- [ ] Remove `type Services` from `impl Aggregate`
- [ ] Remove `async fn handle_create` from `impl Aggregate`
- [ ] Remove `async fn handle_update` from `impl Aggregate`
- [ ] Create new `#[async_trait::async_trait] impl CommandHandler for YourAggregate`
- [ ] Add `type CreateCommand` to `impl CommandHandler`
- [ ] Add `type UpdateCommand` to `impl CommandHandler`
- [ ] Add `type Services` to `impl CommandHandler`
- [ ] Add `async fn handle_create` to `impl CommandHandler`
- [ ] Add `async fn handle_update` to `impl CommandHandler`
- [ ] Run `cargo build` to verify compilation
- [ ] Run tests to verify behavior

## AI Agent Instructions

For AI agents performing this migration automatically:

### Detection

Search for files containing the old pattern:
```
grep -r "impl Aggregate for" --include="*.rs" | grep -v "impl CommandHandler"
```

Identify aggregates that have `type CreateCommand` inside `impl Aggregate` blocks.

### Transformation Algorithm

1. **Parse** the `impl Aggregate` block
2. **Extract** the following items:
   - `type CreateCommand`
   - `type UpdateCommand`
   - `type Services`
   - `async fn handle_create`
   - `async fn handle_update`
3. **Remove** extracted items from `impl Aggregate`
4. **Create** new `impl CommandHandler` block with extracted items
5. **Update** imports to include `CommandHandler`
6. **Preserve** all other code unchanged

### Validation

After transformation:
1. Run `cargo build` - must succeed
2. Run `cargo test` - all tests must pass
3. Verify no duplicate implementations exist

### Edge Cases

- **Multiple aggregates in one file**: Process each separately
- **Custom derives or attributes**: Preserve all attributes
- **Documentation comments**: Keep doc comments with their associated items
- **Feature flags on associated types**: Preserve `#[cfg(...)]` attributes

## Troubleshooting

### Error: "the trait bound `YourAggregate: CommandHandler` is not satisfied"

You forgot to implement `CommandHandler` for your aggregate. Add the `impl CommandHandler` block.

### Error: "duplicate definitions for `CreateCommand`"

You still have `type CreateCommand` in both `impl Aggregate` and `impl CommandHandler`. Remove it from `impl Aggregate`.

### Error: "method `handle_create` is not a member of trait `Aggregate`"

The method was not moved to `impl CommandHandler`. Move `handle_create` and `handle_update` to the `CommandHandler` implementation.

## Benefits of This Change

1. **Separation of Concerns**: Event application logic is separate from command handling
2. **Flexibility**: Aggregates can be reconstituted from events without command handling
3. **Testability**: Each trait can be tested independently
4. **Read Model Support**: Enables aggregate state reconstruction for read models without command processing overhead
