use crate::account::{Account, Amount, Events};
use chrono::{DateTime, Utc};
use cqrs_rust_lib::read::storage::HasId;
use cqrs_rust_lib::read::{SortDirection, Sorter};
use cqrs_rust_lib::{EventEnvelope, View};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
pub struct Movement {
    pub id: String,
    pub account_id: String,
    pub amount: Amount,
    pub date: DateTime<Utc>,
}

impl From<&EventEnvelope<Account>> for Movement {
    fn from(value: &EventEnvelope<Account>) -> Self {
        Self {
            id: Self::view_id(value),
            account_id: value.aggregate_id.to_string(),
            date: value.at,
            ..Default::default()
        }
    }
}

impl HasId for Movement {
    fn field_id() -> &'static str {
        "id"
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn parent_field_id() -> Option<&'static str> {
        Some("account_id")
    }

    fn parent_id(&self) -> Option<&str> {
        Some(&self.account_id)
    }
}

impl View<Account> for Movement {
    const TYPE: &'static str = "movement";
    const IS_CHILD_OF_AGGREGATE: bool = true;

    fn view_id(event: &EventEnvelope<Account>) -> String {
        format!("{}-{}", event.aggregate_id, event.version)
    }

    fn update(&self, event: &EventEnvelope<Account>) -> Option<Self> {
        match &event.payload {
            Events::AccountCreated { .. } => Some(event.into()),
            Events::Withdrawn { amount } => {
                let mut res: Movement = event.into();
                res.amount = amount.clone() * -1f64;
                Some(res)
            }
            Events::Deposited { amount } => {
                let mut res: Movement = event.into();
                res.amount = amount.clone();
                Some(res)
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, IntoParams)]
pub struct MovementQuery {
    /// The filter this endpoint offers. `_q` may only name fields of this struct, so
    /// declaring `date` here is what makes `_q=date>=2026-01-01` legal — and what gives
    /// it a typed parameter in the OpenAPI document.
    ///
    /// Not `account_id`: `Movement` is a child view, so the account is already the path
    /// parameter and the storage ANDs it into every filter. A caller-supplied one could
    /// only restate the path id or force an empty page.
    pub date: Option<DateTime<Utc>>,
}

impl cqrs_rust_lib::read::Query for MovementQuery {
    /// `Movement` also stores `amount`, deliberately not a field here: `Amount` is a
    /// nested value, not a scalar this endpoint offers to compare on. `_q` is bounded by
    /// this struct, so it cannot be filtered on.
    ///
    /// `id` and `account_id` are sortable without being filterable — which is the point
    /// of the list being separate from the `_q` derivation.
    fn sortable_fields(&self) -> Vec<&str> {
        vec!["id", "account_id", "date"]
    }

    /// The route is paginated (`CQRSCodexReadRouter` exposes `skip`/`limit`), and a
    /// statement paged over an undefined order can show the same movement twice and
    /// skip another. Most recent first, ending in `id` so ties within a timestamp are
    /// ordered too — see `Pagination`.
    fn default_sort() -> Option<Vec<Sorter>> {
        Some(vec![
            Sorter {
                field: "date".into(),
                direction: SortDirection::Desc,
            },
            Sorter {
                field: "id".into(),
                direction: SortDirection::Asc,
            },
        ])
    }
}
