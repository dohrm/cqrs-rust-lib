use crate::read::{Pagination, Query, SortDirection, Sorter};
use rest_sql::RestSql;
use std::fmt::Debug;

/// HTTP Codex convention: parses `_q` (RSQL), pagination and `sort` from HTTP
/// query params alongside any typed fields in `Q`. The resulting filter is
/// `_q AND Q::filter()`.
///
/// ## Pagination
///
/// Two vocabularies are accepted; `skip`/`limit` wins when both are present:
/// - `skip` / `limit` — offset based, maps directly to [`Pagination`].
///   `skip` alone is honoured (the backend applies its default limit).
/// - `page` / `page_size` (or its camelCase alias `pageSize`) — page based,
///   translated to `skip = page * page_size`.
///
/// `sort` format: comma-separated field names, prefix `-` for descending.
/// Example: `sort=-created_at,name` → `[Desc(created_at), Asc(name)]`.
///
/// Use as a handler extractor or as the `Q` type in `CQRSCodexReadRouter` to
/// enable the Codex convention on REST routes.
///
/// ## Extraction
///
/// `CqrsHttpQuery<Q>` implements `axum::extract::FromRequestParts`. Use it
/// directly as a handler parameter:
///
/// ```ignore
/// async fn list(
///     CqrsHttpQuery(query): CqrsHttpQuery<GameQuery>,
///     Extension(ctx): Extension<CqrsContext>,
/// ) -> impl IntoResponse { ... }
/// ```
#[derive(Debug, Clone, serde::Serialize)]
pub struct CqrsHttpQuery<Q: serde::Serialize> {
    /// The parsed `_q`, not the raw text: a query that does not parse is rejected
    /// during extraction, so by the time this exists it is known-good.
    #[serde(skip)]
    parsed_q: Option<RestSql>,
    #[serde(skip)]
    skip: Option<i64>,
    #[serde(skip)]
    limit: Option<i64>,
    #[serde(skip)]
    page: Option<i64>,
    #[serde(skip)]
    page_size: Option<i64>,
    #[serde(skip)]
    sort: Option<String>,
    #[serde(flatten)]
    typed: Q,
}

/// The query params the Codex convention owns, consumed before `Q` ever sees them.
///
/// A field of `Q` with one of these names is unreachable as a typed param — the extractor
/// eats it — so it must not be reachable from `_q` either, nor be published as if it
/// were, or the two syntaxes name different sets. That is the one thing ADR-0002 forbids.
const RESERVED_PARAMS: &[&str] = &[
    "_q",
    "skip",
    "limit",
    "page",
    "page_size",
    "pageSize",
    "sort",
];

impl<Q: serde::Serialize> CqrsHttpQuery<Q> {
    pub fn typed(&self) -> &Q {
        &self.typed
    }
}

#[cfg(feature = "rest")]
mod axum_impl {
    use super::CqrsHttpQuery;
    use crate::warn_once::warn_once;
    use crate::read::Query;
    use crate::CqrsError;
    use axum::extract::FromRequestParts;
    use axum::http::request::Parts;
    use axum::response::{IntoResponse, Response};
    use percent_encoding::percent_decode_str;
    use rest_sql::RestSql;
    use serde::de::DeserializeOwned;
    use std::fmt;

    #[derive(Debug)]
    pub struct CodexRejection(String);

    impl fmt::Display for CodexRejection {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "invalid query parameters: {}", self.0)
        }
    }

    /// `422 Unprocessable Entity`, not `400`: the request itself is well-formed HTTP —
    /// it is the *content* of a query parameter the server cannot act on. Every param
    /// the **extractor** parses answers this way. `sort` is the exception and answers
    /// `400`, because it is validated at the storage layer instead — a `Sorter` also
    /// arrives from `Query::default_sort()` and never passes through here.
    impl From<CodexRejection> for CqrsError {
        fn from(rejection: CodexRejection) -> Self {
            CqrsError::unprocessable(rejection.to_string())
        }
    }

    /// Rejections go through [`CqrsError`] so a malformed query yields the same
    /// error body as any other failure instead of a bare text response.
    impl IntoResponse for CodexRejection {
        fn into_response(self) -> Response {
            CqrsError::from(self).into_response()
        }
    }

    /// Parses `_q` and checks every field it names against `Q`'s own fields.
    ///
    /// A parse failure surfaces rest-sql's own error rather than a summary of it:
    /// `ParseErrorAt`'s `Display` carries the position, the offending line and a caret,
    /// which is what makes the response actionable. Why a parse failure is rejected at
    /// all rather than ignored: `docs/adr/0001-reject-malformed-codex-query-params.md`.
    ///
    /// The allowlist is **derived from the struct**, not declared as a list of strings.
    /// `_q` and the typed query params are the same set in two syntaxes — RSQL only adds
    /// the expressiveness a flat `?field=value` cannot carry (`>=`, `=in=`, `or`) — so a
    /// field that is not reachable as a query param has no reason to be reachable from
    /// `_q`. A derived set that is empty means the endpoint offers no filter at all.
    ///
    /// The field check is done here rather than by `RestSql::new_for_fields`, whose
    /// `ValidationError` renders as a `Debug` dump of a rest-sql-internal enum
    /// (`validation error: [ForbiddenField("internal_score")]`) — the shape ADR-0001
    /// exists to avoid. It shares one phrasing with `sort`, in `not_offered` below.
    fn parse_q<Q: DeserializeOwned>(raw: &str) -> Result<RestSql, CodexRejection> {
        let parsed = RestSql::new(raw).map_err(|e| CodexRejection(format!("_q: {e}")))?;

        let allowed: Vec<&str> = rest_sql::dsl::serde_fields::<Q>()
            .iter()
            .copied()
            .filter(|f| !super::RESERVED_PARAMS.contains(f))
            .collect();

        // An empty set is the answer, not a failure to produce one: nothing is reachable
        // as a typed param, so nothing is reachable from `_q`. One shape lands here
        // without meaning to, though — serde emits no `deserialize_struct` for a
        // `#[serde(flatten)]` field or a unit/newtype/tuple struct, so such a type
        // derives nothing while `IntoParams` still publishes its params. Only the
        // operator can fix that, and the message says derivation rather than claiming
        // the endpoint offers no filter, which for the flattened case is false.
        if allowed.is_empty() {
            warn_once_no_derivable_field(std::any::type_name::<Q>());
            return Err(CodexRejection(
                "_q: no filterable field could be derived from the query type".to_string(),
            ));
        }

        for field in parsed.fields() {
            if !allowed.contains(&field) {
                return Err(not_offered("_q", field, &allowed));
            }
        }
        Ok(parsed)
    }

    /// Says it once per query type — whether a type derives a field is fixed at compile
    /// time, so a per-request line would be unbounded volume a caller can trigger.
    ///
    /// The message does not name a closed list of causes. `serde_fields` returns nothing
    /// whenever `deserialize_struct` is not reached, which a `#[serde(flatten)]` field, a
    /// unit/newtype/tuple struct **and a hand-written `Deserialize`** all manage; and a
    /// struct whose every field is a reserved codex name lands here for a different
    /// reason entirely. Naming three of those would send the reader looking for something
    /// that is not there.
    fn warn_once_no_derivable_field(name: &'static str) {
        warn_once(name, || {
            tracing::warn!(
                query_type = name,
                "no filterable field could be derived from this query type, so every _q \
                 against it is refused. Its fields must be plain fields of the struct, \
                 reachable through the derived Deserialize and not shadowed by a codex \
                 param name (logged once per type)"
            );
        });
    }

    /// The one phrasing for "that field is not on offer", shared by `_q` and `sort`.
    fn not_offered(param: &str, field: &str, allowed: &[&str]) -> CodexRejection {
        CodexRejection(format!(
            "{param}: field {field:?} is not available on this view; allowed: {}",
            allowed.join(", ")
        ))
    }

    /// Checks the caller's `sort` fields against `Q::sortable_fields()`.
    ///
    /// An empty list means **no sortable field**, so every caller-supplied `sort` is
    /// refused — the same reading as an empty filterable set in `parse_q`. A field is
    /// sortable because the view says so, never by default.
    ///
    /// Only the caller's fields are checked: `Query::default_sort()` is written in Rust
    /// and is not a caller, so a view can order its own results without offering the
    /// caller any say. What gates both is `Sorter::validated_field` at the storage layer,
    /// asking the different question of whether the name is an identifier at all.
    ///
    /// Checked here rather than in `Query::sort`, which returns `Option<Vec<Sorter>>` and
    /// has nowhere to put an error.
    fn check_sort_fields(raw: &str, allowed: &[&str]) -> Result<(), CodexRejection> {
        if allowed.is_empty() {
            return Err(CodexRejection(
                "sort: this endpoint offers no sortable field".to_string(),
            ));
        }
        for sorter in super::parse_sort(raw) {
            if !allowed.contains(&sorter.field.as_str()) {
                return Err(not_offered("sort", &sorter.field, allowed));
            }
        }
        Ok(())
    }

    /// Parses one of the pagination params: a non-negative `i64`, or a rejection.
    ///
    /// Out-of-range is refused rather than clamped because it has no single meaning
    /// across the backends, and `minimum` is where that line falls:
    ///
    /// - `skip` and `page` take `0` — the first page. Negative is refused: mongodb and
    ///   surrealdb clamp it with `.max(0)`, postgres passes it to `OFFSET` and the
    ///   server errors.
    /// - `limit` and `page_size` take `1`. `0` is refused for the same reason one step
    ///   over: postgres and surrealdb read it as "zero rows", while it is the MongoDB
    ///   wire protocol's *no limit* sentinel — so `?limit=0` would return the whole
    ///   collection on one backend and nothing on the others.
    ///
    /// An empty value never reaches here: the caller treats it as an absent param.
    ///
    /// The value is percent-decoded first, like `_q` and `sort`. Digits are unreserved
    /// so it rarely matters, but a caller who encodes them anyway (`?limit=%35`) should
    /// not get a rejection for it — and the asymmetry became visible the moment a bad
    /// value stopped being silently defaulted.
    fn parse_count(name: &str, raw: &str, minimum: i64) -> Result<i64, CodexRejection> {
        let raw = percent_decode_str(raw).decode_utf8_lossy();
        let expected = if minimum == 0 {
            "a non-negative integer"
        } else {
            "an integer of at least 1"
        };
        match raw.parse::<i64>() {
            Ok(n) if n >= minimum => Ok(n),
            Ok(n) => Err(CodexRejection(format!(
                "{name}: expected {expected}, got {n}"
            ))),
            Err(e) => Err(CodexRejection(format!(
                "{name}: expected {expected}, got {raw:?} ({e})"
            ))),
        }
    }

    impl<S, Q> FromRequestParts<S> for CqrsHttpQuery<Q>
    where
        // `Query` is new here, and it is what gives the extractor access to
        // `Q::sortable_fields()`. Every existing use already satisfies it: the value is
        // useless without `impl Query for CqrsHttpQuery<Q>`, which requires it too.
        Q: Query + serde::Serialize + DeserializeOwned + Send,
        S: Send + Sync,
    {
        type Rejection = CodexRejection;

        async fn from_request_parts(
            parts: &mut Parts,
            _state: &S,
        ) -> Result<Self, Self::Rejection> {
            let raw = parts.uri.query().unwrap_or("");

            let mut parsed_q: Option<RestSql> = None;
            let mut skip: Option<i64> = None;
            let mut limit: Option<i64> = None;
            let mut page: Option<i64> = None;
            let mut page_size: Option<i64> = None;
            let mut sort: Option<String> = None;
            let mut rest: Vec<(&str, &str)> = Vec::new();

            for pair in raw.split('&').filter(|s| !s.is_empty()) {
                let (k, v) = pair.split_once('=').unwrap_or((pair, ""));

                match k {
                    "_q" if !v.is_empty() => {
                        let decoded = percent_decode_str(v).decode_utf8_lossy().into_owned();
                        parsed_q = Some(parse_q::<Q>(&decoded)?);
                    }
                    "skip" if !v.is_empty() => skip = Some(parse_count("skip", v, 0)?),
                    "limit" if !v.is_empty() => limit = Some(parse_count("limit", v, 1)?),
                    "page" if !v.is_empty() => page = Some(parse_count("page", v, 0)?),
                    "page_size" | "pageSize" if !v.is_empty() => {
                        page_size = Some(parse_count(k, v, 1)?);
                    }
                    // No emptiness guard here, unlike the params above: `Query::sort`
                    // already falls back to the typed query on an empty string, and a
                    // second guard saying the same thing is one no test can distinguish.
                    // Validated after the loop: `sortable_fields` takes `&self`, so it
                    // needs the deserialized `Q`, which does not exist yet.
                    "sort" => {
                        sort = Some(percent_decode_str(v).decode_utf8_lossy().into_owned());
                    }
                    // An empty value is how a form or a serialized object writes "unset"
                    // — `?_q=&limit=10`, `?skip=&limit=20`. That is an *absent* param, not
                    // an unreadable one, so it is consumed and sets nothing. Consumed, not
                    // forwarded: a codex param must never reach `Q`, whatever `Q`'s fields
                    // are called. An empty *typed* field still goes to serde_urlencoded
                    // below, where `?name=` keeps whatever meaning `Q` gives it.
                    "_q" | "skip" | "limit" | "page" | "page_size" | "pageSize" => {}
                    _ => rest.push((k, v)),
                }
            }

            // Rebuild the remaining query string and let serde_urlencoded handle
            // percent-decoding and type coercion for Q's fields.
            let rest_qs = rest
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect::<Vec<_>>()
                .join("&");

            // `page * page_size` is computed in `pagination()`. Both factors are
            // individually valid `i64`s, so the product is where an unauthenticated
            // query string can overflow — panicking under `overflow-checks` and wrapping
            // to a *negative* skip without them, which is the state `parse_count` exists
            // to prevent.
            // `pagination()` reads `page` only alongside `page_size`, so `?page=3` on its
            // own used to be answered with page 0 — the caller asked one question and was
            // answered another, which is the shape ADR-0001 is about.
            //
            // Scoped to the case where nothing else decides the window. `?page=2&limit=10`
            // is a documented mix — "skip/limit wins when both are present" — so it keeps
            // answering as it did; widening the rejection there would break a combination
            // this type explicitly blesses.
            if page.is_some() && page_size.is_none() && skip.is_none() && limit.is_none() {
                return Err(CodexRejection(
                    "page: requires page_size (or use skip/limit)".to_string(),
                ));
            }

            if let (Some(page), Some(size)) = (page, page_size) {
                page.checked_mul(size).ok_or_else(|| {
                    CodexRejection(format!(
                        "page: page * page_size overflows (page={page}, page_size={size})"
                    ))
                })?;
            }

            let typed = serde_urlencoded::from_str::<Q>(&rest_qs)
                .map_err(|e| CodexRejection(e.to_string()))?;

            // An empty value is an absent param, as everywhere else in this loop — so
            // `?sort=` asks for nothing and is not a sort to validate. Without the guard
            // it would trip the "no sortable field" refusal on a view that never wanted
            // to sort in the first place.
            if let Some(raw_sort) = sort.as_deref().filter(|s| !s.is_empty()) {
                check_sort_fields(raw_sort, &typed.sortable_fields())?;
            }

            Ok(CqrsHttpQuery {
                parsed_q,
                skip,
                limit,
                page,
                page_size,
                sort,
                typed,
            })
        }
    }
}

#[cfg(feature = "rest")]
pub use axum_impl::CodexRejection;

impl<Q: Query> Query for CqrsHttpQuery<Q> {
    fn filter(&self) -> Option<RestSql> {
        let from_raw = self.parsed_q.clone();
        let from_typed = self.typed.filter();
        match (from_raw, from_typed) {
            (Some(a), Some(b)) => RestSql::from_ast(a.ast().clone() & b.ast().clone()).ok(),
            (Some(r), None) | (None, Some(r)) => Some(r),
            (None, None) => None,
        }
    }

    /// `skip`/`limit` take precedence over `page`/`page_size`; when neither is
    /// present the typed query decides.
    fn pagination(&self) -> Option<Pagination> {
        if self.limit.is_some() || self.skip.is_some() {
            return Some(Pagination {
                skip: Some(self.skip.unwrap_or(0)),
                limit: self.limit,
            });
        }
        match self.page_size {
            // Saturating, not `*`: the extractor rejects an overflowing pair, and this
            // keeps the method total for a value built any other way.
            Some(size) => Some(Pagination {
                limit: Some(size),
                skip: Some(self.page.unwrap_or(0).saturating_mul(size)),
            }),
            None => self.typed.pagination(),
        }
    }

    fn sort(&self) -> Option<Vec<Sorter>> {
        match self.sort.as_deref() {
            Some(s) if !s.is_empty() => Some(parse_sort(s)),
            _ => self.typed.sort(),
        }
    }

    /// Forwards the inner type's sortable fields.
    ///
    /// Enforcement does not go through here — the extractor asks `Q` directly, before a
    /// `CqrsHttpQuery` exists. This is so the wrapper does not *lie*: it is the query
    /// type the storage layer sees under `CQRSCodexReadRouter`, and inheriting the empty
    /// default would have it report "no restriction" for a view that declares one.
    fn sortable_fields(&self) -> Vec<&str> {
        self.typed.sortable_fields()
    }
}

fn parse_sort(s: &str) -> Vec<Sorter> {
    s.split(',')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(|p| {
            if let Some(field) = p.strip_prefix('-') {
                Sorter {
                    field: field.to_string(),
                    direction: SortDirection::Desc,
                }
            } else {
                Sorter {
                    field: p.to_string(),
                    direction: SortDirection::Asc,
                }
            }
        })
        .collect()
}

#[cfg(feature = "utoipa")]
mod utoipa_impl {
    use super::CqrsHttpQuery;
    use crate::read::Query;
    use utoipa::openapi::path::{Parameter, ParameterBuilder, ParameterIn};
    use utoipa::openapi::Required;
    use utoipa::{IntoParams, PartialSchema};

    impl<Q: Query + IntoParams> IntoParams for CqrsHttpQuery<Q> {
        fn into_params(parameter_in_provider: impl Fn() -> Option<ParameterIn>) -> Vec<Parameter> {
            // Drop any field of `Q` whose name the extractor consumes: it is unreachable
            // as a typed param, so publishing it would promise a parameter that silently
            // becomes pagination — and would emit the name twice.
            let mut params: Vec<Parameter> = Q::into_params(&parameter_in_provider)
                .into_iter()
                .filter(|p| !super::RESERVED_PARAMS.contains(&p.name.as_str()))
                .collect();

            params.push(
                ParameterBuilder::new()
                    .name("_q")
                    .parameter_in(ParameterIn::Query)
                    .description(Some(
                        "RSQL filter string, ANDed with the typed query params. It may \
                         only name this endpoint's own typed parameters — not _q, sort, \
                         or the pagination params — and a string that does not parse, or \
                         that names anything else, is rejected with 422. An empty value \
                         means no filter. Syntax: field==value;other!=value.",
                    ))
                    .required(Required::False)
                    .schema(Some(String::schema()))
                    .build(),
            );
            params.push(
                ParameterBuilder::new()
                    .name("skip")
                    .parameter_in(ParameterIn::Query)
                    .description(Some(
                        "Number of items to skip. Takes precedence over page/page_size. \
                         Must be a non-negative integer; anything else is rejected \
                         with 422.",
                    ))
                    .required(Required::False)
                    .schema(Some(i64::schema()))
                    .build(),
            );
            params.push(
                ParameterBuilder::new()
                    .name("limit")
                    .parameter_in(ParameterIn::Query)
                    .description(Some(
                        "Maximum number of items to return. Takes precedence over \
                         page/page_size. Must be an integer of at least 1; anything \
                         else is rejected with 422.",
                    ))
                    .required(Required::False)
                    .schema(Some(i64::schema()))
                    .build(),
            );
            params.push(
                ParameterBuilder::new()
                    .name("page")
                    .parameter_in(ParameterIn::Query)
                    .description(Some(
                        "Zero-based page number. Requires page_size unless skip or \
                         limit is given. Must be a non-negative integer; anything else \
                         is rejected with 422.",
                    ))
                    .required(Required::False)
                    .schema(Some(i64::schema()))
                    .build(),
            );
            params.push(
                ParameterBuilder::new()
                    .name("page_size")
                    .parameter_in(ParameterIn::Query)
                    .description(Some(
                        "Items per page. Alias: pageSize. Must be an integer of at \
                         least 1, and page requires it; anything else is rejected \
                         with 422.",
                    ))
                    .required(Required::False)
                    .schema(Some(i64::schema()))
                    .build(),
            );
            params.push(
                ParameterBuilder::new()
                    .name("sort")
                    .parameter_in(ParameterIn::Query)
                    .description(Some(
                        "Comma-separated sort fields. Prefix `-` for descending. \
                         Example: -created_at,name. A view declares which of its fields \
                         it sorts on and offers none by default, so a field it has not \
                         declared — or any field at all on a view declaring none — is \
                         rejected with 422. A name must also be one or more \
                         `.`-separated segments of [A-Za-z_][A-Za-z0-9_]*, else 400.",
                    ))
                    .required(Required::False)
                    .schema(Some(String::schema()))
                    .build(),
            );
            params
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::read::{Pagination, Query, Sorter};
    use rest_sql::{filter, RestSql};

    #[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
    #[allow(dead_code)]
    // `deny_unknown_fields` is load-bearing: it is what makes "a codex param is consumed,
    // never forwarded to `Q`" observable. Without it a leaked `_q=` would be quietly
    // ignored by serde and no test could tell.
    #[serde(deny_unknown_fields)]
    struct TestQuery {
        name: Option<String>,
        count: Option<i64>,
        active: Option<bool>,
    }

    impl Query for TestQuery {
        fn filter(&self) -> Option<RestSql> {
            let name = self.name.as_deref()?;
            RestSql::from_ast(filter::eq("name", name)).ok()
        }
        fn pagination(&self) -> Option<Pagination> {
            None
        }
        fn sort(&self) -> Option<Vec<Sorter>> {
            None
        }
    }

    /// All params unset — combine with struct update syntax to set the ones a
    /// test cares about.
    fn base(typed: TestQuery) -> CqrsHttpQuery<TestQuery> {
        CqrsHttpQuery {
            parsed_q: None,
            skip: None,
            limit: None,
            page: None,
            page_size: None,
            sort: None,
            typed,
        }
    }

    fn make(
        raw_q: Option<&str>,
        page: Option<i64>,
        page_size: Option<i64>,
        sort: Option<&str>,
        typed: TestQuery,
    ) -> CqrsHttpQuery<TestQuery> {
        CqrsHttpQuery {
            // The extractor is what parses `_q`; these tests build the struct directly,
            // so they parse here for the same reason — a `_q` in the struct is valid.
            parsed_q: raw_q.map(|q| RestSql::new(q).expect("test _q must parse")),
            page,
            page_size,
            sort: sort.map(String::from),
            ..base(typed)
        }
    }

    #[test]
    fn no_params() {
        let q = make(None, None, None, None, TestQuery::default());
        assert!(q.filter().is_none());
        assert!(q.pagination().is_none());
        assert!(q.sort().is_none());
    }

    #[test]
    fn typed_only() {
        let q = make(
            None,
            None,
            None,
            None,
            TestQuery {
                name: Some("hello".into()),
                ..Default::default()
            },
        );
        let f = q.filter().unwrap();
        assert!(f.fields().contains(&"name"));
    }

    #[test]
    fn raw_q_only() {
        let q = make(Some("score==42"), None, None, None, TestQuery::default());
        let f = q.filter().unwrap();
        assert!(f.fields().contains(&"score"));
    }

    #[test]
    fn raw_q_and_typed_combined() {
        let q = make(
            Some("score==42"),
            None,
            None,
            None,
            TestQuery {
                name: Some("alice".into()),
                ..Default::default()
            },
        );
        let f = q.filter().unwrap();
        let fields = f.fields();
        assert!(fields.contains(&"score"), "missing score: {:?}", fields);
        assert!(fields.contains(&"name"), "missing name: {:?}", fields);
    }

    #[test]
    fn pagination_from_http() {
        let q = make(None, Some(2), Some(10), None, TestQuery::default());
        let p = q.pagination().unwrap();
        assert_eq!(p.limit, Some(10));
        assert_eq!(p.skip, Some(20));
    }

    #[test]
    fn pagination_page_zero_default() {
        let q = make(None, None, Some(5), None, TestQuery::default());
        let p = q.pagination().unwrap();
        assert_eq!(p.limit, Some(5));
        assert_eq!(p.skip, Some(0));
    }

    #[test]
    fn pagination_from_skip_and_limit() {
        let q = CqrsHttpQuery {
            skip: Some(25),
            limit: Some(10),
            ..base(TestQuery::default())
        };
        let p = q.pagination().unwrap();
        assert_eq!(p.skip, Some(25));
        assert_eq!(p.limit, Some(10));
    }

    #[test]
    fn pagination_limit_without_skip() {
        let q = CqrsHttpQuery {
            limit: Some(10),
            ..base(TestQuery::default())
        };
        let p = q.pagination().unwrap();
        assert_eq!(p.skip, Some(0));
        assert_eq!(p.limit, Some(10));
    }

    #[test]
    fn pagination_skip_without_limit_lets_backend_default() {
        let q = CqrsHttpQuery {
            skip: Some(30),
            ..base(TestQuery::default())
        };
        let p = q.pagination().unwrap();
        assert_eq!(p.skip, Some(30));
        assert_eq!(p.limit, None);
    }

    #[test]
    fn pagination_skip_limit_wins_over_page() {
        let q = CqrsHttpQuery {
            skip: Some(25),
            limit: Some(10),
            page: Some(3),
            page_size: Some(50),
            ..base(TestQuery::default())
        };
        let p = q.pagination().unwrap();
        assert_eq!(p.skip, Some(25));
        assert_eq!(p.limit, Some(10));
    }

    #[test]
    fn sort_parsed() {
        let q = make(None, None, None, Some("-age,weight"), TestQuery::default());
        let s = q.sort().unwrap();
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].field, "age");
        assert_eq!(s[0].direction, SortDirection::Desc);
        assert_eq!(s[1].field, "weight");
        assert_eq!(s[1].direction, SortDirection::Asc);
    }

    #[test]
    fn sort_falls_back_to_typed_when_empty() {
        let q = make(None, None, None, Some(""), TestQuery::default());
        assert!(q.sort().is_none());
    }

    #[cfg(feature = "rest")]
    mod extraction {
        use super::*;
        use crate::CqrsError;
        use axum::extract::FromRequestParts;

        /// The fallible form — rejection is now half of what these tests assert.
        async fn try_extract(query: &str) -> Result<CqrsHttpQuery<TestQuery>, CqrsError> {
            let req = http::Request::builder()
                .uri(format!("/items?{query}"))
                .body(())
                .unwrap();
            let (mut parts, _) = req.into_parts();
            CqrsHttpQuery::<TestQuery>::from_request_parts(&mut parts, &())
                .await
                .map_err(CqrsError::from)
        }

        async fn extract(query: &str) -> CqrsHttpQuery<TestQuery> {
            try_extract(query)
                .await
                .unwrap_or_else(|e| panic!("{query} must extract, got: {}", e.message))
        }

        #[tokio::test]
        async fn parses_skip_and_limit() {
            let q = extract("skip=25&limit=10&name=bob").await;
            let p = q.pagination().unwrap();
            assert_eq!(p.skip, Some(25));
            assert_eq!(p.limit, Some(10));
            assert_eq!(q.typed().name.as_deref(), Some("bob"));
        }

        #[tokio::test]
        async fn parses_camel_case_page_size_alias() {
            let q = extract("pageSize=5&page=2").await;
            let p = q.pagination().unwrap();
            assert_eq!(p.limit, Some(5));
            assert_eq!(p.skip, Some(10));
        }

        #[tokio::test]
        async fn snake_case_page_size_still_works() {
            let q = extract("page_size=5&page=2").await;
            let p = q.pagination().unwrap();
            assert_eq!(p.limit, Some(5));
            assert_eq!(p.skip, Some(10));
        }

        // ── _q ───────────────────────────────────────────────────────────────────

        /// The defect: `RestSql::new(q).ok()` dropped the error, so this returned 200 with
        /// the whole collection — a filter that fails open.
        #[tokio::test]
        async fn a_malformed_q_is_rejected_instead_of_being_dropped() {
            let err = try_extract("_q=status%3Dbad%3Dactive")
                .await
                .expect_err("a _q that does not parse must not be silently ignored");

            assert_eq!(err.status, 422);
            assert!(err.message.contains("_q"), "{}", err.message);
        }

        /// rest-sql's `ParseErrorAt` renders the position, the offending line and a caret.
        /// That is the half the old code threw away, and it is what makes the response
        /// actionable rather than merely correct.
        #[tokio::test]
        async fn the_rejection_carries_rest_sqls_positioned_error() {
            let err = try_extract("_q=status%3Dbad%3Dactive").await.unwrap_err();

            assert!(
                err.message.contains("parse error at"),
                "expected a positioned error, got: {}",
                err.message
            );
            assert!(
                err.message.contains('^'),
                "expected the caret line, got: {}",
                err.message
            );
        }

        #[tokio::test]
        async fn a_valid_q_still_extracts() {
            let q = extract("_q=name%3D%3DCatan").await;
            assert!(q.filter().is_some());
        }

        // ── pagination params ────────────────────────────────────────────────────

        /// Same shape as `_q`: `v.parse().ok()` meant `?limit=abc` silently became the
        /// default of 20 — the caller asked one question and was answered another.
        #[tokio::test]
        async fn a_non_numeric_pagination_param_is_rejected_and_named() {
            for (param, query) in [
                ("skip", "skip=abc"),
                ("limit", "limit=abc"),
                ("page", "page=abc"),
                ("page_size", "page_size=abc"),
                ("pageSize", "pageSize=abc"),
            ] {
                let Err(err) = try_extract(query).await else {
                    panic!("{query} must be rejected, not silently replaced by a default")
                };

                assert_eq!(err.status, 422, "{query}");
                assert!(
                    err.message.contains(param),
                    "the error must name the parameter the caller got wrong; \
                     for {query} it said: {}",
                    err.message
                );
                assert!(
                    err.message.contains("abc"),
                    "and echo the value it could not read; for {query} it said: {}",
                    err.message
                );
            }
        }

        #[tokio::test]
        async fn a_negative_pagination_param_is_rejected() {
            for param in ["skip", "limit", "page", "page_size"] {
                let err = try_extract(&format!("{param}=-1")).await.unwrap_err();
                assert_eq!(err.status, 422, "{param}");
                assert!(err.message.contains(param), "{}", err.message);
            }
        }

        /// `0` is a page size nobody asks for, and it does not mean the same thing
        /// twice: postgres and surrealdb read `LIMIT 0` as zero rows, while `0` is the
        /// MongoDB wire protocol's *no limit* sentinel — so it would return the whole
        /// collection on one backend and nothing on the others.
        #[tokio::test]
        async fn a_zero_limit_is_rejected_but_a_zero_offset_is_not() {
            for param in ["limit", "page_size", "pageSize"] {
                let err = try_extract(&format!("{param}=0")).await.unwrap_err();
                assert_eq!(err.status, 422, "{param}");
                assert!(err.message.contains("at least 1"), "{}", err.message);
            }

            for query in ["skip=0&limit=10", "page=0&page_size=10"] {
                let q = extract(query).await;
                assert_eq!(
                    q.pagination().expect("pagination").skip,
                    Some(0),
                    "{query}: the first page is a legitimate request"
                );
            }
        }

        /// `pagination()` reads `page` only alongside `page_size`, so `?page=3` alone was
        /// answered with page 0 — the exact "asked one question, answered another" shape.
        #[tokio::test]
        async fn a_page_without_a_page_size_is_rejected_rather_than_ignored() {
            let err = try_extract("page=3").await.unwrap_err();
            assert_eq!(err.status, 422);
            assert!(err.message.contains("page_size"), "{}", err.message);

            // `page_size` alone stays valid: it is page 0 of that size.
            let q = extract("page_size=10").await;
            assert_eq!(q.pagination().expect("pagination").skip, Some(0));

            // And mixing vocabularies stays legal: `skip`/`limit` wins when both are
            // present, which this type documents, so `page` is not orphaned there.
            let q = extract("page=2&limit=10").await;
            let pagination = q.pagination().expect("pagination");
            assert_eq!(pagination.limit, Some(10));
            assert_eq!(pagination.skip, Some(0), "skip/limit decides the window");
        }

        // ── the query surface: `_q` from the struct, `sort` from a list ──────────

        /// A view that restricts sorting. `internal_rank` is the field it does *not*
        /// offer; `title` is the case the list exists for — a column of the view worth
        /// ordering by that has no business being a filter parameter.
        #[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
        #[serde(deny_unknown_fields)]
        struct GuardedQuery {
            name: Option<String>,
        }

        impl Query for GuardedQuery {
            fn sortable_fields(&self) -> Vec<&str> {
                vec!["name", "title"]
            }
        }

        async fn try_extract_guarded(
            query: &str,
        ) -> Result<CqrsHttpQuery<GuardedQuery>, CqrsError> {
            let req = http::Request::builder()
                .uri(format!("/items?{query}"))
                .body(())
                .unwrap();
            let (mut parts, _) = req.into_parts();
            CqrsHttpQuery::<GuardedQuery>::from_request_parts(&mut parts, &())
                .await
                .map_err(CqrsError::from)
        }

        // ── `_q` is bounded by the query struct, always ──────────────────────────

        #[tokio::test]
        async fn a_field_of_the_query_struct_is_filterable() {
            let q = extract("_q=name%3D%3DCatan").await;
            assert!(q.filter().is_some(), "`name` is a field of TestQuery");
        }

        /// The defect #4 is about: `RestSql::new` validates operators, not names, so any
        /// field a caller wrote was compiled into a storage filter against the stored
        /// document — including fields the query type does not expose.
        #[tokio::test]
        async fn a_field_that_is_not_in_the_query_struct_is_rejected_and_named() {
            let Err(err) = try_extract("_q=internal_score%3D%3D5").await else {
                panic!("internal_score is not a field of TestQuery")
            };

            assert_eq!(err.status, 422);
            assert!(
                err.message.contains("internal_score"),
                "the error must name the field the caller cannot use: {}",
                err.message
            );
            assert!(
                err.message.contains("name"),
                "and list what is on offer: {}",
                err.message
            );
        }

        /// Every field of the struct, not just the one the test happens to send — this is
        /// derived from `Deserialize`, so a field added to `Q` is filterable with no
        /// second list to update.
        #[tokio::test]
        async fn the_whole_struct_is_the_surface() {
            for field in ["name", "count", "active"] {
                let q = extract(&format!("_q={field}%3D%3D1")).await;
                assert!(q.filter().is_some(), "{field} is a field of TestQuery");
            }
        }

        /// The derivation reads field names off `deserialize_struct`. serde does not emit
        /// that for a `#[serde(flatten)]` field, nor for a unit/newtype/tuple struct — so
        /// a query type with visible fields can derive none of them, and the answer has
        /// to say so rather than blame the caller's field name.
        #[tokio::test]
        async fn a_query_type_whose_fields_cannot_be_derived_says_so() {
            #[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
            struct Page {
                cursor: Option<String>,
            }

            #[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
            struct FlattenedQuery {
                name: Option<String>,
                #[serde(flatten)]
                page: Page,
            }

            impl Query for FlattenedQuery {}

            let req = http::Request::builder()
                .uri("/items?_q=name%3D%3DCatan")
                .body(())
                .unwrap();
            let (mut parts, _) = req.into_parts();
            let Err(err) =
                CqrsHttpQuery::<FlattenedQuery>::from_request_parts(&mut parts, &()).await
            else {
                panic!("serde derives no field for a flattened struct, so _q cannot work")
            };

            let err = CqrsError::from(err);
            assert_eq!(err.status, 422);
            assert!(
                err.message.contains("could be derived"),
                "the message says derivation — claiming the endpoint offers no filter \
                 would be false here, since the typed params still filter: {}",
                err.message
            );
            assert!(
                !err.message.contains("allowed: "),
                "and must not print an empty allowlist: {}",
                err.message
            );
            assert!(
                !err.message.contains("name"),
                "nor blame the field the caller wrote: {}",
                err.message
            );
        }

        /// Whether a type derives a field is fixed at compile time, so repeating the
        /// notice per request would be unbounded log volume a caller can trigger at will.
        #[tokio::test]
        async fn the_underivable_query_type_is_reported_once_not_per_request() {
            use crate::log_capture::{containing, events_of_async};

            #[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
            struct Nested {
                cursor: Option<String>,
            }

            #[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
            struct OnceWarnedQuery {
                name: Option<String>,
                #[serde(flatten)]
                nested: Nested,
            }

            impl Query for OnceWarnedQuery {}

            async fn attempt() {
                let req = http::Request::builder()
                    .uri("/items?_q=name%3D%3DCatan")
                    .body(())
                    .unwrap();
                let (mut parts, _) = req.into_parts();
                let _ = CqrsHttpQuery::<OnceWarnedQuery>::from_request_parts(&mut parts, &()).await;
            }

            let first = events_of_async(attempt()).await;
            assert_eq!(
                containing(&first, "no filterable field").len(),
                1,
                "the operator is told once, got {first:?}"
            );

            let second = events_of_async(async {
                attempt().await;
                attempt().await;
            })
            .await;
            assert!(
                containing(&second, "no filterable field").is_empty(),
                "and not again, got {second:?}"
            );
        }

        /// A query type that genuinely offers no filter lands on the same message. The
        /// two cases are indistinguishable from here, which is why it names both.
        #[tokio::test]
        async fn a_query_type_with_no_fields_rejects_every_filter() {
            #[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
            struct NoFilters {}

            impl Query for NoFilters {}

            let req = http::Request::builder()
                .uri("/items?_q=anything%3D%3D1")
                .body(())
                .unwrap();
            let (mut parts, _) = req.into_parts();
            let Err(err) = CqrsHttpQuery::<NoFilters>::from_request_parts(&mut parts, &()).await
            else {
                panic!("a query type with no fields offers no filter")
            };
            assert_eq!(CqrsError::from(err).status, 422);
        }

        /// A field of `Q` whose name the extractor consumes is unreachable as a typed
        /// param — `?limit=5` is pagination, never `Q::limit`. So it must not be
        /// reachable from `_q` either, or the two syntaxes name different sets.
        #[tokio::test]
        async fn a_field_shadowed_by_a_codex_param_is_reachable_from_neither() {
            #[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
            struct ShadowedQuery {
                name: Option<String>,
                limit: Option<i64>,
            }

            impl Query for ShadowedQuery {}

            async fn extract_shadowed(
                query: &str,
            ) -> Result<CqrsHttpQuery<ShadowedQuery>, CqrsError> {
                let req = http::Request::builder()
                    .uri(format!("/items?{query}"))
                    .body(())
                    .unwrap();
                let (mut parts, _) = req.into_parts();
                CqrsHttpQuery::<ShadowedQuery>::from_request_parts(&mut parts, &())
                    .await
                    .map_err(CqrsError::from)
            }

            let Err(err) = extract_shadowed("_q=limit%3D%3D5").await else {
                panic!("`limit` is eaten as pagination, so _q must not reach it either")
            };
            assert_eq!(err.status, 422);
            assert!(err.message.contains("limit"), "{}", err.message);

            // The field that is not shadowed still works.
            let q = extract_shadowed("_q=name%3D%3DCatan")
                .await
                .expect("`name` is reachable both ways");
            assert!(q.filter().is_some());
        }

        /// The migration guide calls this out as a break callers must act on, and it is a
        /// property of `serde_fields` + `RestSql::fields()` — third-party behaviour a
        /// version bump could flip silently under a security-adjacent check.
        #[tokio::test]
        async fn a_dotted_path_is_not_a_field_of_the_struct() {
            let Err(err) = try_extract("_q=name.first%3D%3Dx").await else {
                panic!("the derivation returns top-level names, so a dotted path is not one")
            };
            assert_eq!(err.status, 422);
            assert!(err.message.contains("name.first"), "{}", err.message);
        }

        /// `#[serde(alias)]` is the one serde attribute that breaks the equivalence, and
        /// this pins the breakage rather than papering over it.
        ///
        /// serde builds a struct's `FIELDS` alias-expanded, so the derived set admits the
        /// alias — but `_q` never passes through serde: the name it carries goes straight
        /// into the AST and out through the `FieldMapper` as written. So `?label=x`
        /// deserialises into `name` and filters on `name`, while `_q=label==x` filters on
        /// `label`, a column that does not exist. `rename` is fine: it moves both sides.
        ///
        /// Until the allowlist can be the serialize-side names, an alias on a query type
        /// is unsupported — see ADR-0002.
        #[tokio::test]
        async fn a_serde_alias_reaches_q_under_its_own_name_which_is_why_it_is_unsupported() {
            #[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
            struct AliasedQuery {
                #[serde(alias = "label")]
                name: Option<String>,
            }

            impl Query for AliasedQuery {}

            async fn extract_aliased(
                query: &str,
            ) -> Result<CqrsHttpQuery<AliasedQuery>, CqrsError> {
                let req = http::Request::builder()
                    .uri(format!("/items?{query}"))
                    .body(())
                    .unwrap();
                let (mut parts, _) = req.into_parts();
                CqrsHttpQuery::<AliasedQuery>::from_request_parts(&mut parts, &())
                    .await
                    .map_err(CqrsError::from)
            }

            let by_field = extract_aliased("_q=name%3D%3Dx").await.expect("the field");
            assert_eq!(by_field.filter().expect("a filter").fields(), vec!["name"]);

            // The alias is admitted and reaches the storage under its own name. That is
            // the defect; the assertion exists so a fix has to change it deliberately.
            let by_alias = extract_aliased("_q=label%3D%3Dx").await.expect("the alias");
            assert_eq!(
                by_alias.filter().expect("a filter").fields(),
                vec!["label"],
                "the alias is not canonicalised, so it names a column serde would never \
                 have produced — hence unsupported on a query type"
            );

            // A name serde does not accept either way is still refused.
            let Err(err) = extract_aliased("_q=nickname%3D%3Dx").await else {
                panic!("`nickname` is neither the field nor its alias")
            };
            assert_eq!(err.status, 422);
        }

        // ── `sort` is bounded by the declared list, when there is one ────────────

        /// The default is an empty list, and an empty list means the view offers no sort
        /// — the same reading as an empty filterable set. A field is sortable because the
        /// view says so, never by default.
        #[tokio::test]
        async fn an_undeclared_sortable_list_refuses_every_sort() {
            let Err(err) = try_extract("sort=whatever_the_view_stores").await else {
                panic!("TestQuery declares no sortable field, so it offers no sort")
            };

            assert_eq!(err.status, 422);
            assert!(
                err.message.contains("no sortable field"),
                "the endpoint offers none — that is the fact, not a bad field name: {}",
                err.message
            );
        }

        /// `?sort=` is an absent param, not an empty sort, so it does not trip the
        /// refusal above on a view that never asked to sort.
        #[tokio::test]
        async fn an_empty_sort_value_is_not_a_sort_at_all() {
            let q = extract("sort=").await;
            assert!(q.sort().is_none(), "TestQuery declares no default sort");
        }

        #[tokio::test]
        async fn a_sort_field_in_the_declared_list_is_accepted() {
            let q = try_extract_guarded("sort=-title,name")
                .await
                .expect("both fields are declared sortable");
            let sorters = q.sort().expect("a sort was requested");
            assert_eq!(sorters.len(), 2);
            assert_eq!(sorters[0].field, "title");
            assert_eq!(sorters[1].field, "name");
        }

        /// `title` is sortable but is not a field of `GuardedQuery`, which is the whole
        /// point of the list being separate from the `_q` derivation.
        #[tokio::test]
        async fn a_sortable_field_need_not_be_a_filterable_one() {
            let q = try_extract_guarded("sort=title").await.expect("declared");
            assert_eq!(q.sort().expect("a sort")[0].field, "title");

            let Err(err) = try_extract_guarded("_q=title%3D%3DCatan").await else {
                panic!("title is sortable but is not a field of GuardedQuery")
            };
            assert_eq!(err.status, 422);
        }

        #[tokio::test]
        async fn a_sort_field_outside_the_declared_list_is_rejected_and_named() {
            for query in [
                "sort=internal_rank",
                "sort=-internal_rank",
                "sort=name,internal_rank",
            ] {
                let Err(err) = try_extract_guarded(query).await else {
                    panic!("{query} names a field the view does not sort on")
                };

                assert_eq!(err.status, 422, "{query}");
                assert!(
                    err.message.contains("internal_rank"),
                    "the error must name the field; for {query} it said: {}",
                    err.message
                );
            }
        }

        /// Offering no sort to the caller is not the same as having none: a view can
        /// order its own results and expose no choice at all. With the empty default now
        /// meaning "no sortable field", this is the combination that has to keep working.
        #[tokio::test]
        async fn a_view_that_offers_no_sort_can_still_sort_itself() {
            #[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
            struct FixedOrderQuery {
                name: Option<String>,
            }

            impl Query for FixedOrderQuery {
                // No `sortable_fields`: the caller gets no say.
                fn default_sort() -> Option<Vec<Sorter>> {
                    Some(vec![Sorter {
                        field: "created_at".into(),
                        direction: SortDirection::Desc,
                    }])
                }
            }

            let req = http::Request::builder().uri("/items").body(()).unwrap();
            let (mut parts, _) = req.into_parts();
            let q = CqrsHttpQuery::<FixedOrderQuery>::from_request_parts(&mut parts, &())
                .await
                .expect("no caller-supplied sort to refuse");

            let sorters = q.sort().expect("the view orders its own results");
            assert_eq!(sorters[0].field, "created_at");
        }

        /// The list constrains the *caller*. `Query::default_sort()` is written in Rust
        /// and is not filtered by it — nothing else would notice if the check moved into
        /// `Query::sort`.
        #[tokio::test]
        async fn a_declared_list_does_not_constrain_default_sort() {
            #[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
            struct InternallySortedQuery {
                name: Option<String>,
            }

            impl Query for InternallySortedQuery {
                fn sortable_fields(&self) -> Vec<&str> {
                    vec!["name"]
                }
                fn default_sort() -> Option<Vec<Sorter>> {
                    // Outside the list on purpose: written in Rust, not by a caller.
                    Some(vec![Sorter {
                        field: "internal_rank".into(),
                        direction: SortDirection::Asc,
                    }])
                }
            }

            let req = http::Request::builder().uri("/items").body(()).unwrap();
            let (mut parts, _) = req.into_parts();
            let q = CqrsHttpQuery::<InternallySortedQuery>::from_request_parts(&mut parts, &())
                .await
                .expect("no caller-supplied params to reject");

            let sorters = q.sort().expect("the view's own default sort applies");
            assert_eq!(sorters[0].field, "internal_rank");
        }

        /// `CqrsHttpQuery<Q>` is the query type the storage layer sees under
        /// `CQRSCodexReadRouter`, so a wrapper that inherited the empty default would
        /// report "no restriction" for a view that declares one.
        #[tokio::test]
        async fn the_wrapper_reports_the_inner_types_sortable_fields() {
            let guarded = try_extract_guarded("").await.expect("no params");
            assert_eq!(guarded.sortable_fields(), vec!["name", "title"]);

            let plain = extract("").await;
            assert!(
                plain.sortable_fields().is_empty(),
                "and passes the undeclared default through unchanged"
            );
        }

        // ── an empty value is an absent param ────────────────────────────────────

        /// `?_q=&limit=10` is how a form or a serialized object writes "no filter". It
        /// never parsed as RSQL and it always worked; rejecting it would break callers
        /// carrying no bad value at all. The extractor already treated an empty `sort`
        /// this way — these pin the same rule for the rest.
        #[tokio::test]
        async fn an_empty_q_means_no_filter_not_a_bad_one() {
            for query in ["_q=", "_q", "_q=&limit=10"] {
                let q = extract(query).await;
                assert!(q.filter().is_none(), "{query} must apply no filter");
            }
        }

        #[tokio::test]
        async fn an_empty_pagination_param_is_absent_not_unreadable() {
            let q = extract("skip=&limit=10").await;
            let pagination = q.pagination().expect("limit is still honoured");
            assert_eq!(pagination.limit, Some(10));
            assert_eq!(pagination.skip, Some(0), "an unset skip is the first page");

            for query in ["limit=", "page=", "page_size=", "pageSize="] {
                let q = extract(query).await;
                assert!(
                    q.pagination().is_none(),
                    "{query} must leave pagination to the typed query"
                );
            }
        }

        /// `page * page_size` is the one product an unauthenticated query string can
        /// overflow: it panics under `overflow-checks` and wraps to a negative skip without
        /// them — the state the non-negative rule exists to prevent.
        #[tokio::test]
        async fn a_page_times_page_size_overflow_is_rejected() {
            let err = try_extract("page=4611686018427387904&page_size=4")
                .await
                .expect_err("the product overflows i64");

            assert_eq!(err.status, 422);
            assert!(err.message.contains("overflow"), "{}", err.message);
        }

        #[tokio::test]
        async fn valid_pagination_params_still_extract() {
            let q = extract("skip=20&limit=10").await;
            let pagination = q.pagination().expect("pagination present");
            assert_eq!(pagination.skip, Some(20));
            assert_eq!(pagination.limit, Some(10));
        }
    }
}

#[cfg(all(test, feature = "utoipa"))]
mod utoipa_tests {
    use super::CqrsHttpQuery;
    use crate::read::Query;
    use utoipa::openapi::path::ParameterIn;
    use utoipa::IntoParams;

    #[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, utoipa::IntoParams)]
    struct TypedQuery {
        /// A field the view itself declares — it must survive alongside the codex ones.
        name: Option<String>,
    }

    impl Query for TypedQuery {}

    fn params() -> Vec<utoipa::openapi::path::Parameter> {
        CqrsHttpQuery::<TypedQuery>::into_params(|| Some(ParameterIn::Query))
    }

    #[test]
    fn the_typed_params_and_the_codex_params_are_both_documented() {
        let names: Vec<String> = params().into_iter().map(|p| p.name).collect();
        assert_eq!(
            names,
            ["name", "_q", "skip", "limit", "page", "page_size", "sort"],
            "the typed query's own params come first, then the codex ones"
        );
    }

    /// A caller learns the 422 from the API document or not at all — the same reason
    /// the `sort` grammar is asserted below.
    #[test]
    fn every_extractor_parsed_param_documents_its_422() {
        for name in ["_q", "skip", "limit", "page", "page_size"] {
            let param = params()
                .into_iter()
                .find(|p| p.name == name)
                .unwrap_or_else(|| panic!("{name} is a codex param"));
            let description = param.description.unwrap_or_default();
            assert!(
                description.contains("422"),
                "{name} is rejected with 422 but does not say so: {description}"
            );
        }
    }

    /// A field of `Q` the extractor consumes is not a reachable parameter, so publishing
    /// it would promise something that silently becomes pagination — and emit the name
    /// twice, once from `Q` and once from the codex block below.
    #[test]
    fn a_field_shadowed_by_a_codex_param_is_published_once_as_the_codex_one() {
        #[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, IntoParams)]
        struct ShadowedQuery {
            name: Option<String>,
            limit: Option<i64>,
        }

        impl Query for ShadowedQuery {}

        let params = CqrsHttpQuery::<ShadowedQuery>::into_params(|| Some(ParameterIn::Query));
        let limits: Vec<_> = params.iter().filter(|p| p.name == "limit").collect();

        assert_eq!(limits.len(), 1, "`limit` must appear once, not twice");
        assert!(
            limits[0]
                .description
                .as_deref()
                .unwrap_or_default()
                .contains("Maximum number of items"),
            "and it must be the codex parameter, which is the one that works"
        );
        assert!(params.iter().any(|p| p.name == "name"), "the rest survives");
    }

    /// `_q` is bounded by the query's own fields, and the document already lists those
    /// one by one — as typed parameters, which is the point of deriving the surface from
    /// the struct rather than from a list of strings. What the description has to carry
    /// is the *rule*, so a consumer knows the two facts relate.
    ///
    /// The sortable list is deliberately absent: `Query::sortable_fields` takes `&self`,
    /// so a per-instance answer cannot be rendered into a static document.
    #[test]
    fn the_q_param_documents_that_it_is_bounded_by_the_query_fields() {
        let params = CqrsHttpQuery::<TypedQuery>::into_params(|| Some(ParameterIn::Query));
        let description = params
            .iter()
            .find(|p| p.name == "_q")
            .expect("_q is a codex param")
            .description
            .clone()
            .unwrap_or_default();

        assert!(
            description.contains("only name this endpoint's own typed parameters"),
            "the rule must be discoverable from the document, got: {description}"
        );
        assert!(
            params.iter().any(|p| p.name == "name"),
            "and the fields themselves are the query's own typed params"
        );
    }

    /// The `sort` grammar is a documented part of the contract: a caller reading the
    /// OpenAPI document is how they learn that a hyphenated field now answers 400.
    #[test]
    fn the_sort_param_documents_the_field_name_grammar() {
        let sort = params()
            .into_iter()
            .find(|p| p.name == "sort")
            .expect("sort is a codex param");
        let description = sort.description.unwrap_or_default();
        assert!(
            description.contains("[A-Za-z_][A-Za-z0-9_]*"),
            "the grammar must be discoverable from the API document, got: {description}"
        );
        assert!(
            description.contains("400"),
            "and so must the status it answers"
        );
    }
}
