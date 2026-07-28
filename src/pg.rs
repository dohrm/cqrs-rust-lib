//! Shared PostgreSQL connection abstraction.
//!
//! Both the event store ([`crate::es::postgres`]) and the read side
//! ([`crate::read::postgres`]) acquire their connections through [`PgPool`], so
//! a single pool implementation serves the whole stack.
//!
//! The crate ships [`SharedClient`], a zero-cost wrapper around a single
//! `Arc<Client>`. Real pools (deadpool-postgres, bb8, …) are supported by
//! implementing [`PgConn`] and [`PgPool`] in the application:
//!
//! ```rust,ignore
//! use cqrs_rust_lib::prelude::postgres::{PgConn, PgPool};
//!
//! #[derive(Debug, Clone)]
//! struct DeadPool(deadpool_postgres::Pool);
//!
//! struct DeadConn(deadpool_postgres::Object);
//!
//! impl PgConn for DeadConn {
//!     fn client(&self) -> &tokio_postgres::Client { &self.0 }
//! }
//!
//! cqrs_async_trait! {
//! impl PgPool for DeadPool {
//!     type Connection = DeadConn;
//!     async fn acquire(&self) -> Result<Self::Connection, CqrsError> {
//!         self.0.get().await.map(DeadConn).map_err(CqrsError::database_error)
//!     }
//! }
//! }
//! ```

use crate::errors::CqrsError;
use std::fmt::Debug;
use std::sync::Arc;
use tokio_postgres::Client;

/// Access to a `tokio_postgres::Client`.
pub trait PgConn: Send + Sync {
    fn client(&self) -> &Client;
}

cqrs_async_trait! {
/// Factory / pool of connections.
pub trait PgPool: Send + Sync + Debug + Clone + 'static {
    type Connection: PgConn + Send + Sync + 'static;
    async fn acquire(&self) -> Result<Self::Connection, CqrsError>;
}
}

/// Wraps a single `Arc<Client>`. NOT safe for concurrent transactions.
#[derive(Debug, Clone)]
pub struct SharedClient(pub Arc<Client>);

impl PgConn for SharedClient {
    fn client(&self) -> &Client {
        &self.0
    }
}

cqrs_async_trait! {
impl PgPool for SharedClient {
    type Connection = SharedClient;
    async fn acquire(&self) -> Result<Self::Connection, CqrsError> {
        Ok(self.clone())
    }
}
}
