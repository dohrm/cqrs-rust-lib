use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type")]
pub enum CreateCommands {
    Register {
        title: String,
        description: String,
        category: String,
        min_players: u8,
        max_players: u8,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type")]
pub enum UpdateCommands {
    Borrow {
        borrower: String,
        until: DateTime<Utc>,
    },
    Return,
    Update {
        title: Option<String>,
        description: Option<String>,
    },
}
