#[doc(hidden)]
pub use async_trait::async_trait as __async_trait;

/// Use this macro instead of `#[async_trait::async_trait]` for WASM compatibility.
///
/// On native targets, this expands to `#[async_trait::async_trait]` (Send futures).
/// On WASM targets, this expands to `#[async_trait::async_trait(?Send)]` (no Send requirement).
#[macro_export]
macro_rules! cqrs_async_trait {
    ($($tt:tt)*) => {
        #[cfg_attr(not(target_arch = "wasm32"), $crate::__async_trait)]
        #[cfg_attr(target_arch = "wasm32", $crate::__async_trait(?Send))]
        $($tt)*
    };
}

mod wasm_compat;
pub use wasm_compat::*;

mod aggregate;
pub use aggregate::*;
mod engine;
pub use engine::*;

mod denormalizer;
pub use denormalizer::*;

mod errors;
#[allow(deprecated)]
pub use errors::*;

mod event;
pub use event::*;

pub mod problem;

mod event_store;
pub use event_store::*;

pub mod es;
#[cfg(feature = "postgres")]
pub mod pg;
pub mod read;

#[cfg(feature = "rest")]
pub mod rest;

mod context;
pub use context::*;
mod snapshot;

pub use snapshot::*;
pub mod dispatchers;

pub mod prelude;
pub use rest_sql as rsql;
#[cfg(test)]
pub mod testing;
