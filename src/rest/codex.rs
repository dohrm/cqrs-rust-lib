use crate::read::{Pagination, Query, SortDirection, Sorter};
use rest_sql::RestSql;
use std::fmt::Debug;

/// HTTP Codex convention: parses `_q` (RSQL), `page`, `page_size`, `sort` from
/// HTTP query params alongside any typed fields in `Q`. The resulting filter is
/// `_q AND Q::filter()`.
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
    #[serde(skip)]
    raw_q: Option<String>,
    #[serde(skip)]
    page: Option<i64>,
    #[serde(skip)]
    page_size: Option<i64>,
    #[serde(skip)]
    sort: Option<String>,
    #[serde(flatten)]
    typed: Q,
}

impl<Q: serde::Serialize> CqrsHttpQuery<Q> {
    pub fn typed(&self) -> &Q {
        &self.typed
    }
}

#[cfg(feature = "rest")]
mod axum_impl {
    use super::CqrsHttpQuery;
    use axum::extract::FromRequestParts;
    use axum::http::request::Parts;
    use axum::response::{IntoResponse, Response};
    use http::StatusCode;
    use percent_encoding::percent_decode_str;
    use serde::de::DeserializeOwned;
    use std::fmt;

    #[derive(Debug)]
    pub struct CodexRejection(String);

    impl fmt::Display for CodexRejection {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "invalid query parameters: {}", self.0)
        }
    }

    impl IntoResponse for CodexRejection {
        fn into_response(self) -> Response {
            (StatusCode::BAD_REQUEST, self.0).into_response()
        }
    }

    impl<S, Q> FromRequestParts<S> for CqrsHttpQuery<Q>
    where
        Q: serde::Serialize + DeserializeOwned + Send,
        S: Send + Sync,
    {
        type Rejection = CodexRejection;

        async fn from_request_parts(
            parts: &mut Parts,
            _state: &S,
        ) -> Result<Self, Self::Rejection> {
            let raw = parts.uri.query().unwrap_or("");

            let mut raw_q: Option<String> = None;
            let mut page: Option<i64> = None;
            let mut page_size: Option<i64> = None;
            let mut sort: Option<String> = None;
            let mut rest: Vec<(&str, &str)> = Vec::new();

            for pair in raw.split('&').filter(|s| !s.is_empty()) {
                let (k, v) = pair.split_once('=').unwrap_or((pair, ""));

                match k {
                    "_q" => {
                        raw_q = Some(percent_decode_str(v).decode_utf8_lossy().into_owned());
                    }
                    "page" => {
                        page = v.parse().ok();
                    }
                    "page_size" => {
                        page_size = v.parse().ok();
                    }
                    "sort" => {
                        sort = Some(percent_decode_str(v).decode_utf8_lossy().into_owned());
                    }
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

            let typed = serde_urlencoded::from_str::<Q>(&rest_qs)
                .map_err(|e| CodexRejection(e.to_string()))?;

            Ok(CqrsHttpQuery {
                raw_q,
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
        let from_raw = self.raw_q.as_deref().and_then(|q| RestSql::new(q).ok());
        let from_typed = self.typed.filter();
        match (from_raw, from_typed) {
            (Some(a), Some(b)) => RestSql::from_ast(a.ast().clone() & b.ast().clone()).ok(),
            (Some(r), None) | (None, Some(r)) => Some(r),
            (None, None) => None,
        }
    }

    fn pagination(&self) -> Option<Pagination> {
        match self.page_size {
            Some(size) => Some(Pagination {
                limit: Some(size),
                skip: Some(self.page.unwrap_or(0) * size),
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
            let mut params = Q::into_params(&parameter_in_provider);
            params.push(
                ParameterBuilder::new()
                    .name("_q")
                    .parameter_in(ParameterIn::Query)
                    .description(Some(
                        "RSQL filter string, ANDed with typed query params. \
                         Example: title==Catan;available==true",
                    ))
                    .required(Required::False)
                    .schema(Some(String::schema()))
                    .build(),
            );
            params.push(
                ParameterBuilder::new()
                    .name("page")
                    .parameter_in(ParameterIn::Query)
                    .description(Some("Zero-based page number (used with page_size)"))
                    .required(Required::False)
                    .schema(Some(i64::schema()))
                    .build(),
            );
            params.push(
                ParameterBuilder::new()
                    .name("page_size")
                    .parameter_in(ParameterIn::Query)
                    .description(Some("Items per page"))
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
                         Example: -created_at,name",
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
    use rest_sql::{Ast, Constraint, Operator, RestSql, Value};

    #[derive(Debug, Clone, Default, serde::Serialize)]
    #[allow(dead_code)]
    struct TestQuery {
        name: Option<String>,
        count: Option<i64>,
        active: Option<bool>,
    }

    impl Query for TestQuery {
        fn filter(&self) -> Option<RestSql> {
            let name = self.name.as_ref()?;
            RestSql::from_ast(Ast::Constraint(Constraint {
                field: "name".into(),
                operator: Operator::Eq,
                value: Value::String(name.clone()),
            }))
            .ok()
        }
        fn pagination(&self) -> Option<Pagination> {
            None
        }
        fn sort(&self) -> Option<Vec<Sorter>> {
            None
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
            raw_q: raw_q.map(String::from),
            page,
            page_size,
            sort: sort.map(String::from),
            typed,
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
}
