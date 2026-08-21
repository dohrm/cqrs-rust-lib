use crate::CqrsError;
use serde::{Deserialize, Serialize};
#[cfg(feature = "utoipa")]
use utoipa::ToSchema;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
pub enum SortDirection {
    Asc,
    Desc,
}

/// One sort term: a logical field name and a direction.
///
/// `field` is interpolated into the generated query, never bound as a parameter, so
/// **any backend building a sort clause must go through [`Sorter::validated_field`]**
/// rather than reading `field` directly.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "utoipa", derive(ToSchema))]
pub struct Sorter {
    pub field: String,
    pub direction: SortDirection,
}

impl Sorter {
    /// Checks that [`Sorter::field`] is a plain logical field name, and returns it.
    ///
    /// Sort fields are interpolated into the generated query — `ORDER BY {field}` in
    /// SQL and SurrealQL, a `Document` key in MongoDB — so nothing but an identifier
    /// may reach that string. The grammar admitted is one or more `.`-separated
    /// segments, each matching `[A-Za-z_][A-Za-z0-9_]*`; the dot is there because
    /// MongoDB and SurrealDB address nested paths with it. Anything else — a space, a
    /// quote, a comment marker, a parenthesis — is a [`CqrsError::validation`] naming
    /// the offending field.
    ///
    /// This validates the **logical** name, before a [`rest_sql::FieldMapper`] runs.
    /// After mapping, a JSONB mapper yields `data->>'field'`, which is no longer an
    /// identifier: a post-mapping check would reject its own legitimate output.
    pub fn validated_field(&self) -> Result<&str, CqrsError> {
        let field = self.field.as_str();
        let valid = !field.is_empty()
            && field.split('.').all(|segment| {
                let mut chars = segment.chars();
                matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
                    && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
            });

        if valid {
            Ok(field)
        } else {
            Err(CqrsError::validation(format!(
                "sort field {field:?} is not a valid field name: expected `.`-separated \
                 segments of [A-Za-z_][A-Za-z0-9_]*"
            )))
        }
    }
}

/// Compiles sorters into an SQL/SurrealQL ` ORDER BY` clause, or `""` when there is no
/// sort. The leading space belongs to the clause so that a caller can interpolate the
/// result unconditionally, whether or not a sort is in effect.
///
/// Fallible, and that is the point: the clause is interpolated into the query string,
/// so every field goes through [`Sorter::validated_field`] first. This is the sink both
/// SQL-shaped backends converge on — a `Sorter` handed straight to `Storage::filter`
/// passes through here too, not only one that came off an HTTP param.
#[cfg(any(feature = "postgres", feature = "surrealdb"))]
pub(crate) fn order_by_clause(
    sort: Option<Vec<Sorter>>,
    mapper: &impl rest_sql::FieldMapper,
) -> Result<String, CqrsError> {
    let sorters = match sort {
        Some(s) if !s.is_empty() => s,
        _ => return Ok(String::new()),
    };
    let parts: Vec<String> = sorters
        .iter()
        .map(|s| {
            let field = mapper.map(s.validated_field()?);
            let dir = match s.direction {
                SortDirection::Asc => "ASC",
                SortDirection::Desc => "DESC",
            };
            Ok(format!("{} {}", field, dir))
        })
        .collect::<Result<_, CqrsError>>()?;
    Ok(format!(" ORDER BY {}", parts.join(", ")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sorter(field: &str) -> Sorter {
        Sorter {
            field: field.to_string(),
            direction: SortDirection::Asc,
        }
    }

    #[test]
    fn plain_identifiers_are_accepted() {
        for field in ["id", "created_at", "_private", "a", "f1", "A_1"] {
            assert_eq!(
                sorter(field).validated_field().unwrap(),
                field,
                "{field} is a plain identifier"
            );
        }
    }

    #[test]
    fn dotted_paths_are_accepted() {
        // MongoDB and SurrealDB address nested documents this way.
        for field in ["muscle.primary", "a.b.c", "_a._b"] {
            assert_eq!(sorter(field).validated_field().unwrap(), field);
        }
    }

    /// The payload measured on `main`, which produced:
    ///   SELECT data FROM articles ORDER BY 1 UNION ALL SELECT data
    ///   FROM secrets-- DESC OFFSET $1 LIMIT $2
    #[test]
    fn the_measured_injection_payload_is_rejected() {
        let hostile = "1 UNION ALL SELECT data FROM secrets--";
        let err = sorter(hostile).validated_field().unwrap_err();
        assert!(
            err.message.contains(hostile),
            "the error must name the offending field, got: {}",
            err.message
        );
    }

    #[test]
    fn anything_that_is_not_an_identifier_is_rejected() {
        let hostile = [
            "",                // empty
            " ",               // whitespace only
            "id DESC",         // internal space — what parse_sort's trim leaves through
            "id--",            // SQL comment marker
            "\"id\"",          // quotes
            "id;DROP TABLE t", // statement separator
            "count(*)",        // call syntax
            "1",               // leading digit
            "1id",             // leading digit
            "-id",             // the direction prefix, which parse_sort strips first
            "id.",             // trailing dot: empty segment
            ".id",             // leading dot: empty segment
            "a..b",            // empty inner segment
            "data->>'x'",      // a mapper's output, not a logical name
            "id\nDESC",        // newline
            "café",            // non-ASCII
        ];
        for field in hostile {
            let Err(err) = sorter(field).validated_field() else {
                panic!("{field:?} must be rejected")
            };
            assert!(
                err.message.contains(&format!("{field:?}")),
                "the error must name {field:?}, got: {}",
                err.message
            );
        }
    }

    #[test]
    fn the_error_is_a_client_error_not_a_server_error() {
        let err = sorter("id DESC").validated_field().unwrap_err();
        assert_eq!(err.status, 400, "a bad sort field is the caller's mistake");
    }

    /// The clause builder's tests carry the same gate as the builder itself, and reuse
    /// the parent module's `sorter()` rather than keeping a second copy of it.
    #[cfg(any(feature = "postgres", feature = "surrealdb"))]
    mod order_by_clause_tests {
        use super::{sorter, *};
        use rest_sql::IdentityMapper;

        #[test]
        fn a_valid_sort_compiles_to_the_expected_clause() {
            assert_eq!(
                order_by_clause(Some(vec![sorter("id")]), &IdentityMapper).unwrap(),
                " ORDER BY id ASC"
            );
            assert_eq!(
                order_by_clause(
                    Some(vec![
                        sorter("created_at"),
                        Sorter {
                            field: "title".into(),
                            direction: SortDirection::Desc,
                        },
                    ]),
                    &IdentityMapper
                )
                .unwrap(),
                " ORDER BY created_at ASC, title DESC"
            );
        }

        /// Empty, not `" ORDER BY "`. The leading space rides with the clause precisely so
        /// that the callers can interpolate it without a branch of their own.
        ///
        /// This also pins a decision: with no sort the library emits **no** `ORDER BY`, and
        /// invents no fallback order of its own. An imposed `ORDER BY id` on a filtered
        /// query that does not use the id index forces a sort of the whole set, and that
        /// cost belongs to the caller. Changing this has to be deliberate — see
        /// `read::page_order::warn_if_page_order_undefined`, which makes the case audible instead.
        #[test]
        fn no_sort_compiles_to_no_clause() {
            assert_eq!(order_by_clause(None, &IdentityMapper).unwrap(), "");
            assert_eq!(order_by_clause(Some(vec![]), &IdentityMapper).unwrap(), "");
        }

        #[test]
        fn a_hostile_sort_field_never_reaches_the_clause() {
            let hostile = "1 UNION ALL SELECT data FROM secrets--";
            let err = order_by_clause(Some(vec![sorter(hostile)]), &IdentityMapper).unwrap_err();
            assert_eq!(err.code, "GENERIC_VALIDATION_FAILED");
            assert!(err.message.contains(hostile));
        }
    }
}
