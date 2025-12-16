use crate::account::amount::Amount;
use crate::account::{CreateCommands, Events, UpdateCommands};
use cqrs_rust_lib::{Aggregate, CommandHandler, CqrsContext, EventEnvelope, View};
use http::StatusCode;
use serde::{Deserialize, Serialize};
use std::io::ErrorKind;
use utoipa::ToSchema;

const AGGREGATE_TYPE: &str = "account";

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
pub struct Account {
    pub id: String,
    pub owner: String,
    pub amount: Amount,
}

#[async_trait::async_trait]
impl Aggregate for Account {
    const TYPE: &'static str = AGGREGATE_TYPE;

    type Event = Events;
    type Error = std::io::Error;

    fn aggregate_id(&self) -> String {
        self.id.clone()
    }

    fn with_aggregate_id(mut self, id: String) -> Self {
        self.id = id;
        self
    }

    fn apply(&mut self, event: Self::Event) -> Result<(), Self::Error> {
        match event {
            Events::AccountCreated { owner } => {
                self.owner = owner;
            }
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

#[async_trait::async_trait]
impl CommandHandler for Account {
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
            CreateCommands::Create { owner } => Ok(vec![Self::Event::AccountCreated { owner }]),
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
                if amount.value < 0f64 {
                    Ok(vec![Self::Event::Withdrawn {
                        amount: amount.abs(),
                    }])
                } else {
                    Ok(vec![Self::Event::Deposited { amount }])
                }
            }
            UpdateCommands::Withdraw { amount } => {
                if amount.value < 0f64 {
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
}

impl View<Account> for Account {
    const TYPE: &'static str = AGGREGATE_TYPE;
    const IS_CHILD_OF_AGGREGATE: bool = false;

    fn view_id(event: &EventEnvelope<Account>) -> String {
        event.aggregate_id.to_string()
    }

    fn update(&self, _event: &EventEnvelope<Account>) -> Option<Self> {
        None
    }
}
