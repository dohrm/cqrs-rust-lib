use cqrs_rust_lib::CqrsContext;
use cqrs_rust_lib::{Aggregate, Event};
use http::StatusCode;
use serde::{Deserialize, Serialize};
use std::io::ErrorKind;
use utoipa::ToSchema;

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
pub struct Account {
    pub id: String,
    pub amount: f64,
}

#[async_trait::async_trait]
impl Aggregate for Account {
    const TYPE: &'static str = "";

    type CreateCommand = CreateCommands;
    type UpdateCommand = UpdateCommands;
    type Event = Events;
    type Services = ();
    type Error = std::io::Error;

    fn with_aggregate_id(mut self, id: String) -> Self {
        self.id = id;
        self
    }
    fn aggregate_id(&self) -> String {
        self.id.clone()
    }

    async fn handle_create(
        &self,
        command: Self::CreateCommand,
        _services: &Self::Services,
        _context: &CqrsContext,
    ) -> Result<Vec<Self::Event>, Self::Error> {
        match command {
            CreateCommands::Create => Ok(vec![Self::Event::AccountCreated]),
        }
    }

    async fn handle_update(
        &self,
        command: Self::UpdateCommand,
        _services: &Self::Services,
        _context: &CqrsContext,
    ) -> Result<Vec<Self::Event>, Self::Error> {
        match command {
            UpdateCommands::Deposit { amount } => {
                if amount < 0f64 {
                    Ok(vec![Self::Event::Withdrawn {
                        amount: amount.abs(),
                    }])
                } else {
                    Ok(vec![Self::Event::Deposited { amount }])
                }
            }
            UpdateCommands::Withdraw { amount } => {
                if amount < 0f64 {
                    Ok(vec![Self::Event::Deposited {
                        amount: amount.abs(),
                    }])
                } else {
                    Ok(vec![Self::Event::Withdrawn { amount }])
                }
            }
            UpdateCommands::Close => Ok(vec![Self::Event::Closed]),
        }
    }

    fn apply(&mut self, event: Self::Event) -> Result<(), Self::Error> {
        match event {
            Events::AccountCreated => {}
            Events::Deposited { amount } => {
                self.amount += amount;
            }
            Events::Withdrawn { amount } => {
                self.amount -= amount;
            }
            Events::Closed => {}
        }
        Ok(())
    }

    fn error(_status: StatusCode, details: &str) -> Self::Error {
        std::io::Error::new(ErrorKind::AddrInUse, details.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
pub enum Events {
    AccountCreated,
    Deposited { amount: f64 },
    Withdrawn { amount: f64 },
    Closed,
}

impl Event for Events {
    fn event_type(&self) -> String {
        "accounts".to_string()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub enum CreateCommands {
    Create,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub enum UpdateCommands {
    Deposit { amount: f64 },
    Withdraw { amount: f64 },
    Close,
}
