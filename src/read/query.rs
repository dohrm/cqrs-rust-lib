use crate::read::Sorter;
use crate::{MaybeSend, MaybeSync};
use serde::Serialize;
use serde_json::Value as JsonValue;
use std::fmt::Debug;

#[derive(Debug, Clone, Default)]
pub struct Pagination {
    pub skip: Option<i64>,
    pub limit: Option<i64>,
}

/// Query abstraction for read-side storage.
///
/// All three methods have default implementations so minimal structs need zero
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
}

/// Converts every non-`null` scalar field of a serializable struct into an
/// equality constraint and ANDs them together.
///
/// Useful in `Query::filter()` overrides that need to combine the auto-derived
/// filter with custom logic.
pub fn derive_filter_from_serde<T: Serialize + ?Sized>(val: &T) -> Option<rest_sql::RestSql> {
    use rest_sql::{Ast, Constraint, Operator};

    let json = serde_json::to_value(val).ok()?;
    let JsonValue::Object(map) = json else {
        return None;
    };

    let constraints: Vec<Ast> = map
        .into_iter()
        .filter(|(_, v)| !v.is_null())
        .filter_map(|(k, v)| {
            json_to_rsql_value(v).map(|rv| {
                Ast::Constraint(Constraint {
                    field: k,
                    operator: Operator::Eq,
                    value: rv,
                })
            })
        })
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
