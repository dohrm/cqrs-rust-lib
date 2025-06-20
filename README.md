# cqrs-rust-lib

A comprehensive Command Query Responsibility Segregation (CQRS) and Event Sourcing library for Rust applications.

## Features

- **CQRS Pattern Implementation**: Separate command and query responsibilities
- **Event Sourcing**: Store state changes as a sequence of events
- **Multiple Storage Options**:
    - In-memory storage for testing and development
    - MongoDB persistence for production use
- **Aggregate Pattern**: Define domain entities with encapsulated business logic
- **Async Support**: Built with async/await for non-blocking operations
- **REST API Integration**: Optional REST API integration with Axum
- **API Documentation**: Optional OpenAPI documentation with utoipa

## Installation

Add this to your `Cargo.toml`:

```toml
[dependencies]
cqrs-rust-lib = "0.1.0"
```

### Features

- `mongodb`: Enable MongoDB persistence
- `utoipa`: Enable REST API and OpenAPI documentation
- `all`: Enable all features

```toml
# Example with MongoDB support
[dependencies]
cqrs-rust-lib = { version = "0.1.0", features = ["mongodb"] }
```

## Usage

### Basic Example

```rust
use cqrs_rust_lib::{Aggregate, CqrsCommandEngine, CqrsContext, Event};
use cqrs_rust_lib::es::inmemory::InMemoryPersist;
use cqrs_rust_lib::es::EventStoreImpl;

// Define your aggregate
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Account {
    id: String,
    balance: f64,
}

// Implement the Aggregate trait
#[async_trait::async_trait]
impl Aggregate for Account {
    // Implementation details...
}

// Create and use the CQRS engine
async fn main() {
    // Create an in-memory event store
    let store = InMemoryPersist::<Account>::new();
    let event_store = EventStoreImpl::new(store);

    // Create the CQRS engine
    let engine = CqrsCommandEngine::new(event_store, vec![], ());
    let context = CqrsContext::default();

    // Execute commands
    let account_id = engine
        .execute_create(CreateAccountCommand::Create, &context)
        .await
        .expect("Failed to create account");

    engine
        .execute_update(&account_id, DepositCommand { amount: 100.0 }, &context)
        .await
        .expect("Failed to deposit");
}
```

### MongoDB Example

```rust
use cqrs_rust_lib::es::mongodb::MongoDBPersist;
use mongodb::{Client, Database};

async fn setup_mongodb() {
    let client = Client::with_uri_str("mongodb://localhost:27017")
        .await
        .expect("Failed to connect to MongoDB");
    let database = client.database("my_app");

    let store = MongoDBPersist::<Account>::new(database);
    let event_store = EventStoreImpl::new(store);

    // Create the CQRS engine with MongoDB persistence
    let engine = CqrsCommandEngine::new(event_store, vec![], ());
}
```

## API Overview

### Core Components

- **Aggregate**: Trait for domain entities with business logic
- **Event**: Trait for domain events
- **CqrsCommandEngine**: Main engine for executing commands
- **EventStore**: Interface for event persistence
- **CqrsContext**: Context for command execution

### Event Store Implementations

- **InMemoryPersist**: In-memory storage for testing
- **MongoDBPersist**: MongoDB-based persistence

## To run the example project:

```shell
cargo run -p example -- start --mongo-uri=mongodb://localhost:27017/test-lib --http-port=8989 --log-level=debug
```

## License

This project is licensed under the terms found in the [LICENSE](LICENSE) file.

## Contributing

Contributions are welcome! Please see [CONTRIBUTING.md](CONTRIBUTING.md) for details on how to contribute to this
project.