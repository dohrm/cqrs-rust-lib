use crate::read::storage::DynStorage;
use crate::read::{Paged, Query};
use crate::rest::codex::CqrsHttpQuery;
use crate::rest::helpers;
use crate::{Aggregate, CqrsContext, CqrsError, View};
use axum::extract::{Path, State};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Extension, Json};
use http::StatusCode;
use serde::de::DeserializeOwned;
use serde_json::json;
use std::fmt::Debug;
use utoipa::openapi::path::ParameterIn;
use utoipa::openapi::{HttpMethod, Ref, RefOr, Schema};
use utoipa::{IntoParams, PartialSchema, ToSchema};
use utoipa_axum::router::{OpenApiRouter, UtoipaMethodRouter};

/// Like `CQRSReadRouter` but uses the HTTP Codex convention for query params:
/// `_q` (RSQL), `page`, `page_size`, `sort` in addition to typed `Q` fields.
#[derive(Clone)]
pub struct CQRSCodexReadRouter<A, V, Q>
where
    A: Aggregate,
    V: View<A> + ToSchema,
    Q: Clone + Debug + DeserializeOwned + Send + Sync + IntoParams + Query,
{
    _phantom: std::marker::PhantomData<(A, V, Q)>,
    storage: DynStorage<V, CqrsHttpQuery<Q>>,
}

impl<A, V, Q> CQRSCodexReadRouter<A, V, Q>
where
    A: Aggregate + 'static,
    V: View<A> + ToSchema + 'static,
    Q: Clone + Debug + DeserializeOwned + Send + Sync + IntoParams + Query + 'static,
{
    #[must_use]
    fn new(storage: DynStorage<V, CqrsHttpQuery<Q>>) -> Self {
        Self {
            _phantom: std::marker::PhantomData,
            storage,
        }
    }

    fn path_parent_id_field() -> String {
        format!("{}_id", A::TYPE)
    }
    fn path_id_field() -> String {
        format!("{}_id", V::TYPE)
    }
    fn base_path() -> String {
        if V::IS_CHILD_OF_AGGREGATE {
            format!("/{{{}}}/{}", Self::path_parent_id_field(), V::TYPE)
        } else {
            String::new()
        }
    }
    fn base_path_parameters() -> Vec<(String, RefOr<Schema>)> {
        if V::IS_CHILD_OF_AGGREGATE {
            vec![(Self::path_parent_id_field(), String::schema())]
        } else {
            vec![]
        }
    }

    fn find_many(router: OpenApiRouter<Self>, tag: &str) -> OpenApiRouter<Self> {
        let path = Self::base_path();
        let response_schema_name = format!("{}_{}", Paged::<V>::name(), V::name());
        let schemas = vec![
            (response_schema_name.to_string(), Paged::<V>::schema()),
            helpers::error_schema(),
        ];

        let paths = helpers::generate_route(
            tag,
            HttpMethod::Get,
            &path,
            RefOr::Ref(Ref::from_schema_name(response_schema_name)),
            Self::base_path_parameters(),
            CqrsHttpQuery::<Q>::into_params(|| Some(ParameterIn::Query)),
            None,
            // 422 is what the extractor answers for a malformed `_q` or pagination
            // param; 400 stays reachable because `sort` is validated at the storage
            // layer. Both, not one or the other.
            &[
                StatusCode::BAD_REQUEST,
                StatusCode::UNPROCESSABLE_ENTITY,
                StatusCode::INTERNAL_SERVER_ERROR,
            ],
        );

        let find_many_handler = if V::IS_CHILD_OF_AGGREGATE {
            get(
                move |State(router): State<Self>,
                      Path(parent_id): Path<String>,
                      query: CqrsHttpQuery<Q>,
                      Extension(context): Extension<CqrsContext>| async {
                    Self::search(router, Some(parent_id), query, context).await
                },
            )
        } else {
            get(
                move |State(router): State<Self>,
                      query: CqrsHttpQuery<Q>,
                      Extension(context): Extension<CqrsContext>| async {
                    Self::search(router, None, query, context).await
                },
            )
        };

        router.routes(UtoipaMethodRouter::<Self>::from((
            schemas,
            paths,
            find_many_handler,
        )))
    }

    fn find_one(router: OpenApiRouter<Self>, tag: &str) -> OpenApiRouter<Self> {
        let path = Self::base_path();
        let response_schema_name = V::name();
        let schemas = vec![
            (response_schema_name.to_string(), V::schema()),
            helpers::error_schema(),
        ];

        let mut path_parameters = Self::base_path_parameters();
        path_parameters.push((Self::path_id_field(), String::schema()));

        let paths = helpers::generate_route(
            tag,
            HttpMethod::Get,
            &format!("{path}/{{{}}}", Self::path_id_field()),
            RefOr::Ref(Ref::from_schema_name(response_schema_name)),
            path_parameters,
            vec![],
            None,
            &[StatusCode::NOT_FOUND, StatusCode::INTERNAL_SERVER_ERROR],
        );

        let find_one_handler = if V::IS_CHILD_OF_AGGREGATE {
            get(
                move |State(router): State<Self>,
                      Path(parent_id): Path<String>,
                      Path(id): Path<String>,
                      Extension(context): Extension<CqrsContext>| async {
                    Self::by_id(router, Some(parent_id), id, context).await
                },
            )
        } else {
            get(
                move |State(router): State<Self>,
                      Path(id): Path<String>,
                      Extension(context): Extension<CqrsContext>| async {
                    Self::by_id(router, None, id, context).await
                },
            )
        };

        router.routes(UtoipaMethodRouter::<Self>::from((
            schemas,
            paths,
            find_one_handler,
        )))
    }

    pub fn routes(storage: DynStorage<V, CqrsHttpQuery<Q>>, tag: &'static str) -> OpenApiRouter {
        let state = Self::new(storage);
        let mut result = OpenApiRouter::<Self>::new();
        result = Self::find_many(result, tag);
        result = Self::find_one(result, tag);
        result.with_state(state)
    }

    async fn search(
        router: Self,
        parent_id: Option<String>,
        query: CqrsHttpQuery<Q>,
        context: CqrsContext,
    ) -> impl IntoResponse {
        let request_id = context.request_id();
        match router.storage.filter(parent_id, query, context).await {
            Ok(result) => (StatusCode::OK, Json(result)).into_response(),
            Err(err) => err.with_request_id_if_absent(request_id).into_response(),
        }
    }

    async fn by_id(
        router: Self,
        parent_id: Option<String>,
        id: String,
        context: CqrsContext,
    ) -> impl IntoResponse {
        let request_id = context.request_id();
        match router.storage.find_by_id(parent_id, &id, context).await {
            Ok(Some(x)) => (StatusCode::OK, Json(x)).into_response(),
            Ok(None) => CqrsError::not_found(format!("{} '{}' not found", V::TYPE, id))
                .with_details(json!({ "id": id }))
                .with_request_id_if_absent(request_id)
                .into_response(),
            Err(err) => err.with_request_id_if_absent(request_id).into_response(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::read::storage::Storage;
    use crate::testing::{TestAggregate, TestView};
    use crate::{MaybeSend, MaybeSync};
    use std::sync::Arc;

    #[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize, IntoParams)]
    struct TestQuery {
        name: Option<String>,
    }

    impl Query for TestQuery {}

    /// The router only needs *a* storage to build its routes; nothing here calls it.
    #[derive(Debug, Clone)]
    struct NoopStorage;

    cqrs_async_trait! {
    impl<V, Q> Storage<V, Q> for NoopStorage
    where
        V: Debug
            + Clone
            + Default
            + serde::Serialize
            + DeserializeOwned
            + MaybeSend
            + MaybeSync
            + 'static,
        Q: Clone + Debug + MaybeSend + MaybeSync + 'static,
    {
        fn type_name(&self) -> &str {
            "test"
        }
        async fn filter(
            &self,
            _parent_id: Option<String>,
            _query: Q,
            _context: CqrsContext,
        ) -> Result<Paged<V>, CqrsError> {
            Ok(Paged::new(Vec::new(), 0, 0, 20))
        }
        async fn find_by_id(
            &self,
            _parent_id: Option<String>,
            _id: &str,
            _context: CqrsContext,
        ) -> Result<Option<V>, CqrsError> {
            Ok(None)
        }
        async fn save(&self, _view: V, _context: CqrsContext) -> Result<(), CqrsError> {
            Ok(())
        }
    }
    }

    fn generated_openapi() -> utoipa::openapi::OpenApi {
        let storage: DynStorage<TestView, CqrsHttpQuery<TestQuery>> = Arc::new(NoopStorage);
        let (_router, api) =
            CQRSCodexReadRouter::<TestAggregate, TestView, TestQuery>::routes(storage, "test")
                .split_for_parts();
        api
    }

    /// A generated client learns the status from the document or not at all, and 422 is
    /// now the answer to the commonest client mistake — a malformed `_q` or pagination
    /// param. 400 stays: `sort` is validated at the storage layer.
    /// Both routes are GETs, so the list route is named, not guessed: `find_one` carries
    /// the id path segment and `find_many` does not. Picking "the first GET" would let
    /// this pass while `find_many` silently lost a status.
    fn list_route_responses() -> utoipa::openapi::Responses {
        let api = generated_openapi();
        let id_segment = format!("{{{}_id}}", TestView::TYPE);
        let (_, item) = api
            .paths
            .paths
            .iter()
            .find(|(path, item)| item.get.is_some() && !path.ends_with(&id_segment))
            .expect("the list route is the GET without the id segment");
        item.get
            .as_ref()
            .expect("a GET operation")
            .responses
            .clone()
    }

    #[test]
    fn the_list_route_declares_both_client_error_statuses() {
        let responses = list_route_responses().responses;

        for status in ["400", "422", "500"] {
            assert!(
                responses.contains_key(status),
                "the list route must declare {status}; it declares {:?}",
                responses.keys().collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn the_router_generates_both_read_routes() {
        let api = generated_openapi();
        assert_eq!(
            api.paths.paths.len(),
            2,
            "one list route and one find-by-id route, got {:?}",
            api.paths.paths.keys().collect::<Vec<_>>()
        );
    }
}
