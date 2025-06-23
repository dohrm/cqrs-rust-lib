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
    pub owner: String,
    pub amount: Amount,
    pub date: DateTime<Utc>,
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
            Events::AccountCreated { owner } => Some(Movement {
                id: Self::view_id(event),
                account_id: event.aggregate_id.clone(),
                owner: owner.clone(),
                amount: Amount::default(),
                date: event.at.clone(),
            }),
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
