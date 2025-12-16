use crate::event::Event;
use crate::CqrsContext;
use http::StatusCode;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fmt::Debug;
#[cfg(feature = "utoipa")]
use utoipa::ToSchema;

#[async_trait::async_trait]
pub trait Aggregate: Default + Debug + Clone + Serialize + DeserializeOwned + Sync + Send {
    const TYPE: &'static str;

    #[cfg(feature = "utoipa")]
    type Event: Event + ToSchema;
    #[cfg(not(feature = "utoipa"))]
    type Event: Event;

    type Error: std::error::Error + Send + Sync + 'static;

    fn aggregate_id(&self) -> String;
    fn with_aggregate_id(self, id: String) -> Self;

    fn apply(&mut self, event: Self::Event) -> Result<(), Self::Error>;

    fn error(status: StatusCode, details: &str) -> Self::Error;
}

#[async_trait::async_trait]
pub trait CommandHandler: Aggregate {
    #[cfg(feature = "utoipa")]
    type CreateCommand: DeserializeOwned + Sync + Send + ToSchema;
    #[cfg(not(feature = "utoipa"))]
    type CreateCommand: DeserializeOwned + Sync + Send;

    #[cfg(feature = "utoipa")]
    type UpdateCommand: DeserializeOwned + Sync + Send + ToSchema;
    #[cfg(not(feature = "utoipa"))]
    type UpdateCommand: DeserializeOwned + Sync + Send;

    type Services: Send + Sync;

    async fn handle_create(
        &self,
        command: Self::CreateCommand,
        services: &Self::Services,
        context: &CqrsContext,
    ) -> Result<Vec<Self::Event>, Self::Error>;

    async fn handle_update(
        &self,
        command: Self::UpdateCommand,
        services: &Self::Services,
        context: &CqrsContext,
    ) -> Result<Vec<Self::Event>, Self::Error>;
}
