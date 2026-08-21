use cqrs_rust_lib::read::{Query, SortDirection, Sorter};
use serde::{Deserialize, Serialize};
use utoipa::IntoParams;

#[derive(Debug, Clone, Serialize, Deserialize, IntoParams)]
pub struct GameQuery {
    pub category: Option<String>,
    pub available: Option<bool>,
    /// Filterable, therefore a field: `_q` may only name what this struct declares, and
    /// declaring it here is also what gives `title` a typed parameter in the OpenAPI
    /// document instead of a name buried in a string list.
    pub title: Option<String>,
}

impl Query for GameQuery {
    /// `GameView` also stores `borrower`, `borrow_until`, `min_players` and
    /// `max_players`; none is a field of this struct, so none is filterable — a caller
    /// writing `_q=borrower==alice` gets a 422. That is not a confidentiality boundary,
    /// since the fields are still in the response body: it stops the listing from being
    /// usable as a *lookup by* borrower.
    ///
    /// Sorting is a separate list because it answers a different need: ordering by a
    /// column of the view is reasonable where filtering on it is not.
    fn sortable_fields(&self) -> Vec<&str> {
        vec!["id", "title", "category", "available"]
    }

    /// The route is paginated (`CQRSCodexReadRouter` exposes `skip`/`limit`), so this is
    /// not decoration: a page over an undefined order can hand the same game to two pages
    /// and another to none. `title` is not unique, so it ends in `id` — a sort that stops
    /// at a non-unique key still orders its ties arbitrarily between two requests. See
    /// `Pagination`.
    fn default_sort() -> Option<Vec<Sorter>> {
        Some(vec![
            Sorter {
                field: "title".into(),
                direction: SortDirection::Asc,
            },
            Sorter {
                field: "id".into(),
                direction: SortDirection::Asc,
            },
        ])
    }
}
