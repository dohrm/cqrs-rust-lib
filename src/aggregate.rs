use crate::event::Event;
use crate::CqrsContext;
use crate::{MaybeSend, MaybeSync};
use http::StatusCode;
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::fmt::Debug;
#[cfg(feature = "utoipa")]
use utoipa::ToSchema;

/// Trait for generating aggregate IDs during creation.
///
/// The default implementation (`DefaultIdGenerator`) generates a UUID v4.
/// Implement this trait to derive the aggregate ID from the create command,
/// which is useful for association-as-aggregate patterns where the ID
/// must be deterministic (e.g., composed from two entity IDs).
pub trait AggregateIdGenerator<A: Aggregate + CommandHandler>: MaybeSend + MaybeSync {
    fn next_id(&self, cmd: &A::CreateCommand, ctx: &CqrsContext) -> String;
}

/// Default ID generator that produces a UUID v4 via `CqrsContext::next_uuid()`.
pub struct DefaultIdGenerator;

impl<A: Aggregate + CommandHandler> AggregateIdGenerator<A> for DefaultIdGenerator {
    fn next_id(&self, _cmd: &A::CreateCommand, ctx: &CqrsContext) -> String {
        ctx.next_uuid()
    }
}

cqrs_async_trait! {
pub trait Aggregate: Default + Debug + Clone + Serialize + DeserializeOwned + MaybeSync + MaybeSend {
    const TYPE: &'static str;

    #[cfg(feature = "utoipa")]
    type Event: Event + ToSchema;
    #[cfg(not(feature = "utoipa"))]
    type Event: Event;

    type Error: std::error::Error + MaybeSend + MaybeSync + 'static;

    fn aggregate_id(&self) -> String;
    fn with_aggregate_id(self, id: String) -> Self;

    fn apply(&mut self, event: Self::Event) -> Result<(), Self::Error>;

    fn error(status: StatusCode, details: &str) -> Self::Error;
}
}

cqrs_async_trait! {
pub trait CommandHandler: Aggregate {
    #[cfg(feature = "utoipa")]
    type CreateCommand: DeserializeOwned + MaybeSync + MaybeSend + ToSchema;
    #[cfg(not(feature = "utoipa"))]
    type CreateCommand: DeserializeOwned + MaybeSync + MaybeSend;

    #[cfg(feature = "utoipa")]
    type UpdateCommand: DeserializeOwned + MaybeSync + MaybeSend + ToSchema;
    #[cfg(not(feature = "utoipa"))]
    type UpdateCommand: DeserializeOwned + MaybeSync + MaybeSend;

    type Services: MaybeSend + MaybeSync;

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
}
