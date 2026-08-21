//! Captures `WARN` and `ERROR` `tracing` events so a test can assert on them.
//!
//! `tracing-subscriber` is not a dependency of this crate and a handful of log assertions
//! are not worth adding one, so the recorder is hand-rolled: it only has to answer "was a
//! warning emitted, with what fields".
//!
//! Anything below `WARN` is dropped, and that is not a shortcut: a backend under test may
//! be chatty on its own — SurrealDB emits `TRACE Parsing SurrealQL query` per statement —
//! and a recorder that collected it would make every `is_empty()` assertion a hostage to
//! a dependency's logging.
use std::fmt::Debug;
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
struct Recorder(Arc<Mutex<Vec<String>>>);

struct Capture(String);

impl tracing::field::Visit for Capture {
    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn Debug) {
        self.0.push_str(&format!("{}={:?} ", field.name(), value));
    }
}

impl tracing::Subscriber for Recorder {
    fn enabled(&self, metadata: &tracing::Metadata<'_>) -> bool {
        *metadata.level() <= tracing::Level::WARN
    }
    fn new_span(&self, _: &tracing::span::Attributes<'_>) -> tracing::span::Id {
        tracing::span::Id::from_u64(1)
    }
    fn record(&self, _: &tracing::span::Id, _: &tracing::span::Record<'_>) {}
    fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}
    fn enter(&self, _: &tracing::span::Id) {}
    fn exit(&self, _: &tracing::span::Id) {}

    fn event(&self, event: &tracing::Event<'_>) {
        let mut capture = Capture(format!("{} ", event.metadata().level()));
        event.record(&mut capture);
        self.0
            .lock()
            .expect("recorder not poisoned")
            .push(capture.0);
    }
}

/// Runs `f` with the recorder installed and returns whatever it logged.
///
/// Gated on its callers: only the read backends log from a synchronous path, so under
/// `--features rest` alone this would be `dead_code`.
#[cfg(any(feature = "postgres", feature = "mongodb", feature = "surrealdb"))]
pub(crate) fn events_of(f: impl FnOnce()) -> Vec<String> {
    let recorder = Recorder::default();
    let events = Arc::clone(&recorder.0);
    {
        let _guard = tracing::subscriber::set_default(recorder);
        f();
    }
    events.lock().expect("recorder not poisoned").clone()
}

/// Narrows captured events to the one a test is about.
///
/// The recorder collects every `WARN` and `ERROR` in the process, so a dependency that
/// logs a warning of its own — today, or after a version bump — would otherwise break an
/// assertion that has nothing to do with the code under test.
pub(crate) fn containing<'a>(events: &'a [String], needle: &str) -> Vec<&'a String> {
    events.iter().filter(|e| e.contains(needle)).collect()
}

/// Same, for an async body.
///
/// `set_default` installs the subscriber **thread-locally**, so this only measures
/// anything on a current-thread runtime — the `#[tokio::test]` default. Under
/// `flavor = "multi_thread"` a future resuming on another worker records nothing,
/// and an `assert!(events.is_empty())` would then pass vacuously.
pub(crate) async fn events_of_async<F: std::future::Future>(f: F) -> Vec<String> {
    let recorder = Recorder::default();
    let events = Arc::clone(&recorder.0);
    {
        let _guard = tracing::subscriber::set_default(recorder);
        f.await;
    }
    events.lock().expect("recorder not poisoned").clone()
}

#[cfg(all(
    test,
    any(feature = "postgres", feature = "mongodb", feature = "surrealdb")
))]
mod tests {
    use super::{containing, events_of};

    /// The level filter is why an `is_empty()` assertion elsewhere means "no warning"
    /// rather than "no log line at all". Without it, a dependency's `info!` or `debug!`
    /// — SurrealDB emits one per parsed statement — would land in every capture.
    #[test]
    fn only_warn_and_above_are_captured() {
        let events = events_of(|| {
            tracing::trace!("a trace line");
            tracing::debug!("a debug line");
            tracing::info!("an info line");
            tracing::warn!("a warn line");
            tracing::error!("an error line");
        });

        assert_eq!(
            events.len(),
            2,
            "only the warn and the error, got {events:?}"
        );
        assert!(events[0].starts_with("WARN "), "{}", events[0]);
        assert!(events[1].starts_with("ERROR "), "{}", events[1]);
    }

    #[test]
    fn fields_and_message_are_both_captured() {
        let view = "article";
        let events = events_of(|| tracing::warn!(view = %view, skip = 20, "a warning"));

        assert_eq!(containing(&events, "a warning").len(), 1, "{events:?}");
        // `%` records through Display, so no quotes — which is what the callers assert on.
        assert!(events[0].contains("view=article"), "{}", events[0]);
        assert!(events[0].contains("skip=20"), "{}", events[0]);
    }
}
