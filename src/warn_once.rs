//! Says a thing once per key, not once per occurrence.
//!
//! Both callers warn about a **static property** — a view that declares no sort order, a
//! query type from which no field can be derived — reached on a **per-request** path.
//! Repeating it carries no information after the first time and is unbounded log volume
//! a caller can trigger at will, which `rules/rust/logging.md` reserves `warn` against.

use std::collections::HashSet;
use std::sync::{Mutex, OnceLock};

static SEEN: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

/// Runs `emit` the first time this `key` is seen, and never again for it.
///
/// Poisoning recovers rather than propagating: a poisoned lock is permanent, so treating
/// it as "not yet seen" would turn the latch into the flood it exists to prevent.
pub(crate) fn warn_once(key: &str, emit: impl FnOnce()) {
    let mut seen = SEEN
        .get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);

    if seen.insert(key.to_owned()) {
        emit();
    }
}
