use crate::account::Amount;
use cqrs_rust_lib::Event;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub enum Events {
    AccountCreated { owner: String },
    Deposited { amount: Amount },
    Withdrawn { amount: Amount },
    Closed,
}

impl Event for Events {
    fn event_type(&self) -> String {
        match self {
            Events::AccountCreated { .. } => "account_created".to_string(),
            Events::Deposited { .. } => "amount_deposited".to_string(),
            Events::Withdrawn { .. } => "amount_withdrawn".to_string(),
            Events::Closed => "account_closed".to_string(),
        }
    }
}
