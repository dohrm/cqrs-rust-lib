use super::commands::{CreateCommands, UpdateCommands};
use super::errors::ErrorCode;
use super::events::GameEvent;
use chrono::{DateTime, Utc};
use cqrs_rust_lib::read::storage::HasId;
use cqrs_rust_lib::{
    cqrs_async_trait, Aggregate, CommandHandler, CqrsContext, CqrsError, CqrsErrorCode,
    EventEnvelope, View,
};
use http::StatusCode;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

const AGGREGATE_TYPE: &str = "game";

// ─── Aggregate ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
pub struct Game {
    pub id: String,
    pub title: String,
    pub description: String,
    pub category: String,
    pub min_players: u8,
    pub max_players: u8,
    pub available: bool,
}

cqrs_async_trait! {
impl Aggregate for Game {
    const TYPE: &'static str = AGGREGATE_TYPE;

    type Event = GameEvent;
    type Error = CqrsError;

    fn aggregate_id(&self) -> String {
        self.id.clone()
    }

    fn with_aggregate_id(mut self, id: String) -> Self {
        self.id = id;
        self
    }

    fn apply(&mut self, event: Self::Event) -> Result<(), Self::Error> {
        match event {
            GameEvent::Registered { title, description, category, min_players, max_players } => {
                self.title = title;
                self.description = description;
                self.category = category;
                self.min_players = min_players;
                self.max_players = max_players;
                self.available = true;
            }
            GameEvent::Borrowed { .. } => {
                self.available = false;
            }
            GameEvent::Returned => {
                self.available = true;
            }
            GameEvent::Updated { title, description } => {
                if let Some(t) = title {
                    self.title = t;
                }
                if let Some(d) = description {
                    self.description = d;
                }
            }
        }
        Ok(())
    }

    fn error(status: StatusCode, details: &str) -> Self::Error {
        CqrsError::from_status(status, details)
    }
}
}

cqrs_async_trait! {
impl CommandHandler for Game {
    type CreateCommand = CreateCommands;
    type UpdateCommand = UpdateCommands;
    type Services = ();

    async fn handle_create(
        &self,
        command: Self::CreateCommand,
        _services: &Self::Services,
        _context: &CqrsContext,
    ) -> Result<Vec<Self::Event>, Self::Error> {
        match command {
            CreateCommands::Register { title, description, category, min_players, max_players } => {
                if title.trim().is_empty() {
                    return Err(ErrorCode::EmptyTitle.error("Le titre ne peut pas être vide"));
                }
                if min_players == 0 || min_players > max_players {
                    return Err(ErrorCode::InvalidPlayerCount
                        .error("Le nombre de joueurs est invalide"));
                }
                Ok(vec![GameEvent::Registered { title, description, category, min_players, max_players }])
            }
        }
    }

    async fn handle_update(
        &self,
        command: Self::UpdateCommand,
        _services: &Self::Services,
        _context: &CqrsContext,
    ) -> Result<Vec<Self::Event>, Self::Error> {
        match command {
            UpdateCommands::Borrow { borrower, until } => {
                if !self.available {
                    return Err(ErrorCode::AlreadyBorrowed
                        .error("Ce jeu est déjà emprunté"));
                }
                Ok(vec![GameEvent::Borrowed { borrower, until }])
            }
            UpdateCommands::Return => {
                if self.available {
                    return Err(ErrorCode::NotBorrowed
                        .error("Ce jeu n'est pas emprunté"));
                }
                Ok(vec![GameEvent::Returned])
            }
            UpdateCommands::Update { title, description } => {
                if let Some(t) = &title
                    && t.trim().is_empty()
                {
                    return Err(ErrorCode::EmptyTitle.error("Le titre ne peut pas être vide"));
                }
                Ok(vec![GameEvent::Updated { title, description }])
            }
        }
    }
}
}

// ─── View ─────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct GameView {
    pub id: String,
    pub title: String,
    pub description: String,
    pub category: String,
    pub min_players: u8,
    pub max_players: u8,
    pub available: bool,
    pub borrower: Option<String>,
    pub borrow_until: Option<DateTime<Utc>>,
    pub version: usize,
}

impl HasId for GameView {
    fn field_id() -> &'static str {
        "id"
    }

    fn id(&self) -> &str {
        &self.id
    }

    fn parent_field_id() -> Option<&'static str> {
        None
    }

    fn parent_id(&self) -> Option<&str> {
        None
    }
}

impl View<Game> for GameView {
    const TYPE: &'static str = AGGREGATE_TYPE;
    const IS_CHILD_OF_AGGREGATE: bool = false;

    fn view_id(event: &EventEnvelope<Game>) -> String {
        event.aggregate_id.clone()
    }

    fn update(&self, event: &EventEnvelope<Game>) -> Option<Self> {
        let mut view = self.clone();
        view.id = event.aggregate_id.clone();
        view.version = event.version;
        match &event.payload {
            GameEvent::Registered {
                title,
                description,
                category,
                min_players,
                max_players,
            } => {
                view.title = title.clone();
                view.description = description.clone();
                view.category = category.clone();
                view.min_players = *min_players;
                view.max_players = *max_players;
                view.available = true;
                view.borrower = None;
                view.borrow_until = None;
            }
            GameEvent::Borrowed { borrower, until } => {
                view.available = false;
                view.borrower = Some(borrower.clone());
                view.borrow_until = Some(*until);
            }
            GameEvent::Returned => {
                view.available = true;
                view.borrower = None;
                view.borrow_until = None;
            }
            GameEvent::Updated { title, description } => {
                if let Some(t) = title {
                    view.title = t.clone();
                }
                if let Some(d) = description {
                    view.description = d.clone();
                }
            }
        }
        Some(view)
    }
}
