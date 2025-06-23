use crate::account::Amount;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type")]
pub enum CreateCommands {
    Create { owner: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type")]
pub enum UpdateCommands {
    Deposit { amount: Amount },
    Withdraw { amount: Amount },
    Close,
}
