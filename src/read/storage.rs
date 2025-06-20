use crate::read::paged::Paged;
use crate::read::sorter::Sorter;
use crate::{Aggregate, AggregateError, CqrsContext, View};
use serde::de::DeserializeOwned;
use std::fmt::Debug;
use utoipa::PartialSchema;
#[cfg(feature = "utoipa")]
use utoipa::ToSchema;

#[async_trait::async_trait]
pub trait Storage<A>: Clone + Debug + Send + Sync
where
    A: Aggregate,
{
    const TYPE: &'static str;
    #[cfg(feature = "utoipa")]
    type View: View<A> + ToSchema;
    #[cfg(not(feature = "utoipa"))]
    type View: View<A>;

    #[cfg(feature = "utoipa")]
    type Query: Clone + Debug + DeserializeOwned + Send + Sync + ToSchema;

    #[cfg(not(feature = "utoipa"))]
    type Query: Clone + Debug + DeserializeOwned + Send + Sync;

    fn name(&self) -> &'static str {
        Self::TYPE
    }

    #[cfg(feature = "utoipa")]
    fn result_schema(&self) -> utoipa::openapi::RefOr<utoipa::openapi::Schema> {
        Self::View::schema()
    }

    #[cfg(feature = "utoipa")]
    fn paged_result_schema(&self) -> utoipa::openapi::RefOr<utoipa::openapi::Schema> {
        Paged::<Self::View>::schema()
    }

    #[cfg(feature = "utoipa")]
    fn query_schema(&self) -> utoipa::openapi::RefOr<utoipa::openapi::Schema> {
        Self::Query::schema()
    }

    #[cfg(feature = "utoipa")]
    fn schemas(
        &self,
        schemas: &mut Vec<(String, utoipa::openapi::RefOr<utoipa::openapi::Schema>)>,
    ) {
        Paged::<Self::View>::schemas(schemas);
        Self::View::schemas(schemas);
        Self::Query::schemas(schemas);
    }

    fn filter(
        &self,
        query: Self::Query,
        page_limit: Option<i64>,
        page_size: Option<i64>,
        sorts: Vec<Sorter>,
        context: CqrsContext,
    ) -> Result<Paged<Self::View>, AggregateError>;

    fn find_by_id(
        &self,
        id: &str,
        context: CqrsContext,
    ) -> Result<Option<Self::View>, AggregateError>;
}
