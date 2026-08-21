use crate::read::Sorter;
use crate::{MaybeSend, MaybeSync};
use serde::Serialize;
use serde_json::Value as JsonValue;
use std::fmt::Debug;

/// A window into a result set: how many items to step over, and how many to return.
///
/// # A window is only meaningful over a defined order
///
/// There is no such thing as a database default sort. MongoDB does not guarantee
/// natural order across two reads, and Postgres guarantees nothing at all without an
/// `ORDER BY`. So when `skip > 0` and no sort is in effect, the storage layer is free
/// to hand the same document to two pages and another to none — while still reporting a
/// correct `total`, `skip` and `limit` on every response, which is what makes the
/// symptom so hard to see.
///
/// The library does **not** invent an order to cover this: an imposed `ORDER BY id` on a
/// filtered query that does not use the id index forces a sort of the whole set, and
/// that cost is the caller's to choose, not the library's to impose. A view that wants
/// stable pages declares [`Query::default_sort`], **ending in a unique field** — a sort
/// on a non-unique key still orders its ties arbitrarily between two requests.
/// Paginating without any sort logs a warning at `warn` rather than failing.
#[derive(Debug, Clone, Default)]
pub struct Pagination {
    pub skip: Option<i64>,
    pub limit: Option<i64>,
}

/// Query abstraction for read-side storage.
///
/// All methods have default implementations so minimal structs need zero
/// boilerplate:
///
/// ```rust,ignore
/// #[derive(Debug, Serialize, Deserialize)]
/// struct GameQuery { category: Option<String>, available: Option<bool> }
/// impl Query for GameQuery {}
/// ```
///
/// The default `filter()` converts every non-`None` field to an equality
/// constraint (`field == value`) ANDed together. Override only when you need
/// non-equality operators (`Gte`, `Like`, …) or field-name remapping.
///
/// # What `_q` may name
///
/// Under `CqrsHttpQuery`, this struct's fields and the RSQL `_q` parameter are **one set
/// in two syntaxes**: RSQL exists because a flat `?field=value` cannot express `>=`,
/// `=in=`, `or` or a range. So `_q` may only name fields of the implementing struct,
/// derived from its `Deserialize` impl — a field not reachable as a query param has no
/// reason to be reachable from `_q`. A struct with no fields offers no filter at all.
/// Sorting is a different question — see [`Query::sortable_fields`].
///
/// **The derivation cannot read every shape.** It reads names off
/// `Deserializer::deserialize_struct`, which serde does not emit for a
/// `#[serde(flatten)]` field, for a unit/newtype/tuple struct, or for a hand-written
/// `Deserialize` that goes through `deserialize_map`. A query type of any of those shapes
/// derives *no* field, and every `_q` against it is rejected. Keep the fields plain and
/// on the struct itself.
///
/// **`#[serde(alias)]` is unsupported here.** serde expands aliases into a struct's
/// `FIELDS`, so the alias is admitted — but `_q` does not pass through serde, and the
/// name reaches the storage as written. `?label=x` would filter on the real field while
/// `_q=label==x` filters on `label`. `#[serde(rename)]` is fine: it moves both sides.
///
/// A field named after a param the extractor owns — `_q`, `skip`, `limit`, `page`,
/// `page_size`, `pageSize`, `sort` — is dropped from the set too, and from the published
/// params: the extractor eats the value, so the field is unreachable either way.
pub trait Query: Debug + Serialize + MaybeSend + MaybeSync {
    /// Returns a filter derived from the struct's serializable fields.
    /// Override when you need operators other than `==` or custom field names.
    fn filter(&self) -> Option<rest_sql::RestSql> {
        derive_filter_from_serde(self)
    }

    /// Pagination hint for the storage layer. Defaults to `None` (let the
    /// storage use its own defaults or rely on `CqrsHttpQuery` page params).
    fn pagination(&self) -> Option<Pagination> {
        None
    }

    /// Static default sort for this view type, applied when no explicit sort
    /// is requested (neither HTTP `sort` param nor `sort()` override).
    /// Use this instead of `sort()` when the sort is unconditional.
    ///
    /// Returning `None` — the default — means this view has no order of its own.
    /// That is fine for a single unpaginated read, but **paginating a view with no
    /// sort in effect is undefined**: `skip`/`limit` may hand the same item to two
    /// pages and another to none. See [`Pagination`]. Declare a sort here if the view
    /// is paged — and end it in a unique field such as the id, or ties are still
    /// ordered arbitrarily between two requests. The storage layer logs a warning when
    /// no sort at all is in effect; it cannot tell whether the one you declared is
    /// unique.
    fn default_sort() -> Option<Vec<Sorter>>
    where
        Self: Sized,
    {
        None
    }

    /// Dynamic sort order. Falls back to `default_sort()`.
    /// Override only when the sort depends on query field values.
    fn sort(&self) -> Option<Vec<Sorter>>
    where
        Self: Sized,
    {
        Self::default_sort()
    }

    /// The complete set of field names a caller may name in `sort`.
    ///
    /// **Empty — the default — means this view offers no sort**, and every
    /// caller-supplied `sort` is refused with `422`. A field is sortable because the view
    /// says so, never by default; that is the same reading as the empty filterable set
    /// in `_q`.
    ///
    /// The list is the *whole* sortable set, not an addition to the query's own fields:
    /// declare `vec!["title"]` and `?sort=category` is refused even if `category` is a
    /// field of the struct. List everything a caller may sort on.
    ///
    /// It does not constrain [`Query::default_sort`], which is written in Rust: a view
    /// can order its own results without offering the caller any say over it.
    ///
    /// ```rust
    /// # use cqrs_rust_lib::read::Query;
    /// # #[derive(Debug, serde::Serialize)]
    /// # struct GameQuery { category: Option<String> }
    /// impl Query for GameQuery {
    ///     fn sortable_fields(&self) -> Vec<&str> {
    ///         vec!["title", "category", "id"]
    ///     }
    /// }
    /// ```
    ///
    /// A list here, a derivation for `_q`, and deliberately so. `_q` is checked against
    /// the **query struct's own fields**, because a filterable field should be a typed
    /// parameter in the OpenAPI document. Sorting has no such constraint: a caller
    /// routinely wants to order by a column of the *view* — `title`, `created_at` — that
    /// the query type has no reason to expose as a filter, so there is nothing to derive
    /// the set from.
    fn sortable_fields(&self) -> Vec<&str> {
        vec![]
    }
}

/// Converts every non-`null` scalar field of a serializable struct into an
/// equality constraint and ANDs them together.
///
/// Useful in `Query::filter()` overrides that need to combine the auto-derived
/// filter with custom logic.
pub fn derive_filter_from_serde<T: Serialize + ?Sized>(val: &T) -> Option<rest_sql::RestSql> {
    use rest_sql::{Ast, filter};

    let json = serde_json::to_value(val).ok()?;
    let JsonValue::Object(map) = json else {
        return None;
    };

    let constraints: Vec<Ast> = map
        .into_iter()
        .filter(|(_, v)| !v.is_null())
        .filter_map(|(k, v)| json_to_rsql_value(v).map(|rv| filter::eq(&k, rv)))
        .collect();

    Ast::try_and(constraints).and_then(|ast| rest_sql::RestSql::from_ast(ast).ok())
}

fn json_to_rsql_value(v: JsonValue) -> Option<rest_sql::Value> {
    use rest_sql::Value;
    match v {
        JsonValue::Bool(b) => Some(Value::Bool(b)),
        JsonValue::Number(n) => {
            if let Some(i) = n.as_i64() {
                Some(Value::Int(i))
            } else {
                n.as_f64().map(Value::Float)
            }
        }
        JsonValue::String(s) => Some(Value::String(s)),
        JsonValue::Array(arr) => {
            let items: Option<Vec<_>> = arr.into_iter().map(json_to_rsql_value).collect();
            items.map(Value::List)
        }
        JsonValue::Null | JsonValue::Object(_) => None,
    }
}
