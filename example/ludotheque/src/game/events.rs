use chrono::{DateTime, Utc};
use cqrs_rust_lib::Event;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub enum GameEvent {
    Registered {
        title: String,
        description: String,
        category: String,
        min_players: u8,
        max_players: u8,
    },
    Borrowed {
        borrower: String,
        until: DateTime<Utc>,
    },
    Returned,
    Updated {
        title: Option<String>,
        description: Option<String>,
    },
}

impl Event for GameEvent {
    fn event_type(&self) -> String {
        match self {
            GameEvent::Registered { .. } => "registered".into(),
            GameEvent::Borrowed { .. } => "borrowed".into(),
            GameEvent::Returned => "returned".into(),
            GameEvent::Updated { .. } => "updated".into(),
        }
    }
}
