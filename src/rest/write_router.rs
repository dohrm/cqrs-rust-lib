use crate::engine::CqrsCommandEngine;
use crate::event_store::EventStore;
use crate::rest::helpers;
use crate::rest::helpers::SchemaData;
use crate::{Aggregate, AggregateError, CqrsContext};
use axum::extract::{Path, State};
use axum::response::IntoResponse;
use axum::routing::{post, put};
use axum::{Extension, Json};
use http::StatusCode;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use utoipa::openapi::{HttpMethod, Ref, RefOr};
use utoipa::{PartialSchema, ToSchema};
use utoipa_axum::router::{OpenApiRouter, UtoipaMethodRouter};

#[derive(Clone)]
pub struct CQRSWriteRouter<A, ES>
where
    A: Aggregate + ToSchema,
    ES: EventStore<A>,
{
    engine: Arc<CqrsCommandEngine<A, ES>>,
}

impl<A, ES> CQRSWriteRouter<A, ES>
where
    A: Aggregate + ToSchema + 'static,
    ES: EventStore<A> + 'static,
{
    #[must_use]
    fn new(engine: CqrsCommandEngine<A, ES>) -> Self {
        Self {
            engine: Arc::new(engine),
        }
    }

    pub fn routes(engine: CqrsCommandEngine<A, ES>) -> OpenApiRouter {
        let context = CQRSWriteRouter::new(engine);

        let mut result = OpenApiRouter::<CQRSWriteRouter<A, ES>>::new();
        let mut base_schema = vec![];
        A::schemas(&mut base_schema);

        let aggregate_name = A::name().to_string();
        let create_command_name = A::CreateCommand::name().to_string();
        let update_command_name = A::UpdateCommand::name().to_string();

        let result_schema_ref = RefOr::Ref(Ref::from_schema_name(&aggregate_name));

        for SchemaData {
            name,
            schema,
            discriminator,
        } in helpers::read_schema(&A::CreateCommand::name(), A::CreateCommand::schema())
        {
            let schema_name = format!("{aggregate_name}_{create_command_name}_{name}");

            let mut schemas = base_schema.clone();
            schemas.push((schema_name.clone(), RefOr::T(schema.clone())));
            A::CreateCommand::schemas(&mut schemas);
            A::schemas(&mut schemas);

            let paths = helpers::generate_route(
                A::TYPE,
                HttpMethod::Post,
                format!("/commands/{}", helpers::sanitize_schema_name(&name)).as_str(),
                result_schema_ref.clone(),
                vec![],
                vec![],
                Some(RefOr::Ref(Ref::from_schema_name(&schema_name))),
            );

            let current_discriminator = discriminator.clone();
            result = result.routes(UtoipaMethodRouter::<CQRSWriteRouter<A, ES>>::from((
                schemas,
                paths,
                post(
                    move |State(router): State<CQRSWriteRouter<A, ES>>,
                          Extension(context): Extension<CqrsContext>,
                          Json(command): Json<Value>| async {
                        Self::create(router, command, current_discriminator, context).await
                    },
                ),
            )))
        }

        for SchemaData {
            name,
            schema,
            discriminator,
        } in helpers::read_schema(&A::UpdateCommand::name(), A::UpdateCommand::schema())
        {
            let schema_name = format!("{aggregate_name}_{update_command_name}_{name}");

            let mut schemas = base_schema.clone();
            schemas.push((schema_name.clone(), RefOr::T(schema.clone())));
            A::UpdateCommand::schemas(&mut schemas);

            let paths = helpers::generate_route(
                A::TYPE,
                HttpMethod::Put,
                format!("/{{id}}/commands/{}", helpers::sanitize_schema_name(&name)).as_str(),
                result_schema_ref.clone(),
                vec![("id".to_string(), String::schema())],
                vec![],
                Some(RefOr::Ref(Ref::from_schema_name(&schema_name))),
            );

            let current_discriminator = discriminator.clone();
            result = result.routes(UtoipaMethodRouter::<CQRSWriteRouter<A, ES>>::from((
                schemas.clone(),
                paths,
                put(
                    move |State(router): State<CQRSWriteRouter<A, ES>>,
                          Path(id): Path<String>,
                          Extension(context): Extension<CqrsContext>,
                          Json(command): Json<Value>| async {
                        Self::update(router, id, command, current_discriminator, context).await
                    },
                ),
            )))
        }

        result.with_state(context)
    }

    fn metadata(context: &CqrsContext) -> HashMap<String, String> {
        HashMap::from_iter(vec![
            ("user_id".to_string(), context.current_user()),
            ("request_id".to_string(), context.request_id()),
        ])
    }

    pub async fn create(
        router: CQRSWriteRouter<A, ES>,
        mut command: Value,
        discriminator: Option<(String, String)>,
        context: CqrsContext,
    ) -> impl IntoResponse {
        helpers::add_discriminator(&mut command, discriminator);
        match serde_json::from_value::<A::CreateCommand>(command) {
            Ok(cmd) => match router
                .engine
                .execute_create_with_metadata(cmd, Self::metadata(&context), &context)
                .await
            {
                Ok(result) => (StatusCode::CREATED, Json(json ! ({"id": result}))).into_response(),
                Err(err) => helpers::aggregate_error_to_json(err).into_response(),
            },
            Err(err) => {
                helpers::aggregate_error_to_json(AggregateError::SerializationError(err.into()))
                    .into_response()
            }
        }
    }

    pub async fn update(
        router: CQRSWriteRouter<A, ES>,
        id: String,
        mut command: Value,
        discriminator: Option<(String, String)>,
        context: CqrsContext,
    ) -> impl IntoResponse {
        helpers::add_discriminator(&mut command, discriminator);
        match serde_json::from_value::<A::UpdateCommand>(command) {
            Ok(cmd) => match router
                .engine
                .execute_update_with_metadata(&id, cmd, Self::metadata(&context), &context)
                .await
            {
                Ok(_) => StatusCode::NO_CONTENT.into_response(),
                Err(err) => helpers::aggregate_error_to_json(err).into_response(),
            },
            Err(err) => {
                helpers::aggregate_error_to_json(AggregateError::SerializationError(err.into()))
                    .into_response()
            }
        }
    }
}
