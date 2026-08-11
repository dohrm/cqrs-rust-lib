use serde::{Deserialize, Serialize};
#[cfg(feature = "utoipa")]
use utoipa::ToSchema;

/// A page of results.
///
/// Both pagination vocabularies are exposed:
/// - `skip` / `limit` — the offset-based form used internally by
///   [`crate::read::Pagination`] and the storage backends. Always exact.
/// - `page` / `page_size` — the derived page-based form, kept for clients that
///   use it. `page` is `skip / limit`, so it is only meaningful when `skip` is a
///   multiple of `limit`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct Paged<T> {
    pub items: Vec<T>,
    pub total: i64,
    /// Number of items skipped before this page.
    pub skip: i64,
    /// Maximum number of items in this page.
    pub limit: i64,
    /// Zero-based page number, derived from `skip / limit`.
    pub page: i64,
    /// Alias of `limit`, kept for page-based clients.
    pub page_size: i64,
}

impl<T> Paged<T> {
    /// Builds a page from the offset-based values, deriving `page` and
    /// `page_size`.
    #[must_use]
    pub fn new(items: Vec<T>, total: i64, skip: i64, limit: i64) -> Self {
        Self {
            items,
            total,
            skip,
            limit,
            page: if limit > 0 { (skip / limit).abs() } else { 0 },
            page_size: limit,
        }
    }

    /// Maps the items while keeping every pagination counter untouched.
    #[must_use]
    pub fn map<U, F: FnMut(T) -> U>(self, f: F) -> Paged<U> {
        Paged {
            items: self.items.into_iter().map(f).collect(),
            total: self.total,
            skip: self.skip,
            limit: self.limit,
            page: self.page,
            page_size: self.page_size,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_page_from_skip_and_limit() {
        let p = Paged::new(vec![1, 2, 3], 137, 20, 10);
        assert_eq!(p.skip, 20);
        assert_eq!(p.limit, 10);
        assert_eq!(p.page, 2);
        assert_eq!(p.page_size, 10);
    }

    #[test]
    fn keeps_exact_skip_when_not_a_multiple_of_limit() {
        let p = Paged::new(vec![1], 137, 25, 10);
        assert_eq!(p.skip, 25);
        assert_eq!(p.limit, 10);
        // page is a lossy approximation, skip/limit stay exact
        assert_eq!(p.page, 2);
    }

    #[test]
    fn zero_limit_does_not_divide_by_zero() {
        let p = Paged::new(Vec::<i32>::new(), 0, 0, 0);
        assert_eq!(p.page, 0);
        assert_eq!(p.page_size, 0);
    }

    #[test]
    fn map_preserves_counters() {
        let p = Paged::new(vec![1, 2], 7, 5, 2).map(|i| i.to_string());
        assert_eq!(p.items, vec!["1".to_string(), "2".to_string()]);
        assert_eq!(p.total, 7);
        assert_eq!(p.skip, 5);
        assert_eq!(p.limit, 2);
        assert_eq!(p.page, 2);
        assert_eq!(p.page_size, 2);
    }
}
