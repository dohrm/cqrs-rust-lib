mod sorter;
pub use sorter::*;
mod paged;
pub use paged::*;
pub mod query;
pub use query::{derive_filter_from_serde, Pagination, Query};

mod memory;
pub use memory::*;

// Only the read backends call this, and they are all feature-gated. Gating the module
// declaration — rather than each item — keeps the cfg in one place and keeps the
// function out of a build that has no caller for it, where it would be `dead_code`
// under CI's `cargo clippy --features rest --all-targets -- -D warnings`.
#[cfg(any(feature = "postgres", feature = "mongodb", feature = "surrealdb"))]
pub(crate) mod page_order;

#[cfg(feature = "mongodb")]
pub mod mongodb;
#[cfg(feature = "postgres")]
pub mod postgres;
pub mod storage;
#[cfg(feature = "surrealdb")]
pub mod surrealdb;
