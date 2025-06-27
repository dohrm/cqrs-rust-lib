use crate::read::storage::Storage;
use crate::read::Paged;
use crate::rest::helpers;
use crate::{Aggregate, CqrsContext, View};
use axum::extract::{Path, Query, State};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Extension, Json};
use http::StatusCode;
use serde::de::DeserializeOwned;
use serde_json::json;
use std::fmt::Debug;
use std::sync::Arc;
use utoipa::openapi::path::ParameterIn;
use utoipa::openapi::{HttpMethod, Ref, RefOr, Schema};
use utoipa::{IntoParams, PartialSchema, ToSchema};
use utoipa_axum::router::{OpenApiRouter, UtoipaMethodRouter};

#[derive(Clone)]
pub struct CQRSReadRouter<A, V, S, Q>
where
    A: Aggregate,
    V: View<A> + ToSchema,
    Q: Clone + Debug + DeserializeOwned + Send + Sync + IntoParams,
    S: Storage<V, Q>,
{
    _phantom: std::marker::PhantomData<(A, V, Q)>,
    storage: Arc<S>,
}

impl<A, V, S, Q> CQRSReadRouter<A, V, S, Q>
where
    A: Aggregate + 'static,
    V: View<A> + ToSchema + 'static,
    Q: Clone + Debug + DeserializeOwned + Send + Sync + IntoParams + 'static,
    S: Storage<V, Q> + 'static,
{
    #[must_use]
    fn new(storage: Arc<S>) -> Self {
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
            "".to_string()
        }
    }

    fn base_path_parameters() -> Vec<(String, RefOr<Schema>)> {
        if V::IS_CHILD_OF_AGGREGATE {
            vec![(Self::path_parent_id_field(), String::schema())]
        } else {
            vec![]
        }
    }

    fn find_many(
        router: OpenApiRouter<CQRSReadRouter<A, V, S, Q>>,
        tag: &str,
    ) -> OpenApiRouter<CQRSReadRouter<A, V, S, Q>> {
        let path = Self::base_path();
        let response_schema_name = format!("{}_{}", Paged::<V>::name(), V::name());
        let schemas = vec![(response_schema_name.to_string(), Paged::<V>::schema())];

        let paths = helpers::generate_route(
            tag,
            HttpMethod::Get,
            &path,
            RefOr::Ref(Ref::from_schema_name(response_schema_name)),
            Self::base_path_parameters(),
            Q::into_params(|| Some(ParameterIn::Query)),
            None,
        );

        let find_many_handler = if V::IS_CHILD_OF_AGGREGATE {
            get(
                move |State(router): State<CQRSReadRouter<A, V, S, Q>>,
                      Path(parent_id): Path<String>,
                      Query(query): Query<Q>,
                      Extension(context): Extension<CqrsContext>| async {
                    Self::search(router, Some(parent_id), query, context).await
                },
            )
        } else {
            get(
                move |State(router): State<CQRSReadRouter<A, V, S, Q>>,
                      Query(query): Query<Q>,
                      Extension(context): Extension<CqrsContext>| async {
                    Self::search(router, None, query, context).await
                },
            )
        };

        router.routes(UtoipaMethodRouter::<CQRSReadRouter<A, V, S, Q>>::from((
            schemas,
            paths,
            find_many_handler,
        )))
    }

    fn find_one(
        router: OpenApiRouter<CQRSReadRouter<A, V, S, Q>>,
        tag: &str,
    ) -> OpenApiRouter<CQRSReadRouter<A, V, S, Q>> {
        let path = Self::base_path();
        let response_schema_name = V::name();
        let schemas = vec![(response_schema_name.to_string(), V::schema())];

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
        );

        let find_one_handler = if V::IS_CHILD_OF_AGGREGATE {
            get(
                move |State(router): State<CQRSReadRouter<A, V, S, Q>>,
                      Path(parent_id): Path<String>,
                      Path(id): Path<String>,
                      Extension(context): Extension<CqrsContext>| async {
                    Self::by_id(router, Some(parent_id), id, context).await
                },
            )
        } else {
            get(
                move |State(router): State<CQRSReadRouter<A, V, S, Q>>,
                      Path(id): Path<String>,
                      Extension(context): Extension<CqrsContext>| async {
                    Self::by_id(router, None, id, context).await
                },
            )
        };

        router.routes(UtoipaMethodRouter::<CQRSReadRouter<A, V, S, Q>>::from((
            schemas,
            paths,
            find_one_handler,
        )))
    }

    pub fn routes(storage: Arc<S>, tag: &'static str) -> OpenApiRouter {
        let state = Self::new(storage);

        let mut result = OpenApiRouter::<CQRSReadRouter<A, V, S, Q>>::new();
        // Find many
        result = Self::find_many(result, tag);
        result = Self::find_one(result, tag);

        result.with_state(state)
    }

    async fn search(
        router: CQRSReadRouter<A, V, S, Q>,
        parent_id: Option<String>,
        query: Q,
        context: CqrsContext,
    ) -> impl IntoResponse {
        match router.storage.filter(parent_id, query, context).await {
            Ok(result) => (StatusCode::OK, Json(result)).into_response(),
            Err(err) => helpers::aggregate_error_to_json(err).into_response(),
        }
    }

    async fn by_id(
        router: CQRSReadRouter<A, V, S, Q>,
        parent_id: Option<String>,
        id: String,
        context: CqrsContext,
    ) -> impl IntoResponse {
        match router.storage.find_by_id(parent_id, &id, context).await {
            Ok(Some(x)) => (StatusCode::OK, Json(x)).into_response(),
            Ok(None) => (StatusCode::NOT_FOUND, Json(json!({ "id": id}))).into_response(),
            Err(err) => helpers::aggregate_error_to_json(err).into_response(),
        }
    }
}
