use crate::account::{Account, Amount, Events};
use chrono::{DateTime, Utc};
use cqrs_rust_lib::read::storage::HasId;
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skip: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limit: Option<i64>,
}
