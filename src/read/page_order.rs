//! The warning a read backend emits when it is asked for a page over an undefined
//! order.
//!
//! Gated in `read/mod.rs` on the backends that call it, so a build without any of them
//! does not carry an unused function — see the module declaration for why that matters.

use crate::read::Sorter;
use crate::warn_once::warn_once;

/// Warns, once per view type, when a page is requested over an undefined order.
///
/// Called by every read backend at the point where it knows both the offset and the
/// sort in effect. Behaviour does not change — see [`crate::read::Pagination`] for why
/// no order is invented — but the case stops being silent.
///
/// **Once per view type, not once per request.** The trigger is per-request — a caller
/// sending `?sort=title` does not warn, the same caller without it does — but the
/// *remedy* is static: declare `Q::default_sort()`, which is a code change, not a
/// request change. So the second report carries no information the first did not, and a
/// `warn` on every list call would only train an operator to filter the level out, at
/// which point the mechanism is worth nothing. The cost of the latch is real and worth
/// knowing: the first offending request silences the view for the process lifetime.
///
/// The first page is exempt, and for a reason of noise rather than correctness: `LIMIT
/// 20` with no `ORDER BY` returns an arbitrary *subset*, not merely an arbitrary order,
/// on page 1 as much as on page 2. But the symptom is only *observable* by comparing two
/// windows, so `skip == 0` is where a warning would be least actionable. An empty sort
/// list counts as no sort, which is how the clause builders already read it.
///
/// Note what the check can and cannot see: *any* sort silences it, but a sort on a
/// non-unique key leaves the same defect — `ORDER BY status` over five hundred rows
/// sharing a status still orders the ties arbitrarily. Hence the message asks for a
/// sort ending in a unique field, not merely for a sort.
pub(crate) fn warn_if_page_order_undefined(type_name: &str, skip: i64, sort: Option<&[Sorter]>) {
    if skip <= 0 || sort.is_some_and(|s| !s.is_empty()) {
        return;
    }
    warn_once(type_name, || {
        tracing::warn!(
            type_name = %type_name,
            skip = skip,
            "paginating a view with no sort in effect: page contents are undefined, \
             declare Query::default_sort() ending in a unique field such as the id \
             (logged once per view)"
        );
    });
}

#[cfg(test)]
mod tests {
    use crate::log_capture::events_of;
    use super::*;
    use crate::read::SortDirection;

    fn sorters(field: &str) -> Vec<Sorter> {
        vec![Sorter {
            field: field.to_string(),
            direction: SortDirection::Asc,
        }]
    }

    // `WARNED_VIEWS` is process-global and the test binary is one process, so every
    // test here uses a view name of its own. A shared name would make the outcome
    // depend on which test ran first.

    #[test]
    fn paginating_without_a_sort_warns_with_the_fields_needed_to_act() {
        let events = events_of(|| warn_if_page_order_undefined("fields_view", 20, None));

        assert_eq!(events.len(), 1, "exactly one warning, got {events:?}");
        let event = &events[0];
        assert!(
            event.starts_with("WARN "),
            "degraded but recoverable: {event}"
        );
        assert!(
            event.contains("type_name=fields_view"),
            "the view must be named so the caller knows where to add a sort: {event}"
        );
        assert!(
            event.contains("skip=20"),
            "and the offset that triggered it: {event}"
        );
        assert!(
            event.contains("default_sort"),
            "the message must name the remedy: {event}"
        );
    }

    /// The condition is static per view, so repeating the warning on every list request
    /// would only teach an operator to filter the level out.
    #[test]
    fn a_view_is_warned_about_once_and_not_again() {
        let first = events_of(|| warn_if_page_order_undefined("once_view", 20, None));
        assert_eq!(first.len(), 1, "the first call warns, got {first:?}");

        let second = events_of(|| {
            warn_if_page_order_undefined("once_view", 20, None);
            warn_if_page_order_undefined("once_view", 40, None);
        });
        assert!(
            second.is_empty(),
            "and no call after it does, got {second:?}"
        );
    }

    /// Two views without a sort are two separate problems to fix.
    #[test]
    fn a_second_view_gets_its_own_warning() {
        let events = events_of(|| {
            warn_if_page_order_undefined("view_a", 20, None);
            warn_if_page_order_undefined("view_b", 20, None);
        });
        assert_eq!(events.len(), 2, "got {events:?}");
        assert!(events[0].contains("type_name=view_a"), "{}", events[0]);
        assert!(events[1].contains("type_name=view_b"), "{}", events[1]);
    }

    /// An empty `Vec<Sorter>` is "no sort", not "a sort of nothing" — that is what the
    /// clause builders already do with it.
    #[test]
    fn an_empty_sort_list_counts_as_no_sort() {
        let events = events_of(|| warn_if_page_order_undefined("empty_sort_view", 20, Some(&[])));
        assert_eq!(events.len(), 1, "got {events:?}");
    }

    #[test]
    fn the_first_page_is_silent() {
        assert!(events_of(|| warn_if_page_order_undefined("first_page_view", 0, None)).is_empty());
        assert!(
            events_of(|| warn_if_page_order_undefined("first_page_view", 0, Some(&[]))).is_empty()
        );
    }

    #[test]
    fn a_page_with_a_sort_in_effect_is_silent() {
        let sort = sorters("created_at");
        assert!(
            events_of(|| warn_if_page_order_undefined("sorted_view", 20, Some(&sort))).is_empty()
        );
        assert!(
            events_of(|| warn_if_page_order_undefined("sorted_view", 0, Some(&sort))).is_empty()
        );
    }
}
