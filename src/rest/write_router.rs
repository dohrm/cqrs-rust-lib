use crate::engine::CqrsCommandEngine;
use crate::event_store::EventStore;
use crate::rest::helpers;
use crate::{Aggregate, AggregateError, CqrsContext};
use axum::extract::{Path, State};
use axum::response::IntoResponse;
use axum::routing::{post, put};
use axum::{Extension, Json};
use http::StatusCode;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use utoipa::openapi::{HttpMethod, RefOr, Schema};
use utoipa::{PartialSchema, ToSchema};
use utoipa_axum::router::{OpenApiRouter, UtoipaMethodRouter};

#[derive(Clone)]
pub struct CQRSWriteRouter<A, ES>
where
    A: Aggregate + ToSchema,
    ES: EventStore<A>,
{
    _phantom: std::marker::PhantomData<(A, ES)>,
    engine: Arc<CqrsCommandEngine<A, ES>>,
}

struct CommandData {
    name: String,
    schema: Schema,
    discriminator: Option<(String, String)>,
}

impl CommandData {
    fn new(name: String, schema: Schema, discriminator: Option<(String, String)>) -> Self {
        Self {
            name,
            schema,
            discriminator,
        }
    }
}

impl<A, ES> CQRSWriteRouter<A, ES>
where
    A: Aggregate + ToSchema + 'static,
    ES: EventStore<A> + 'static,
{
    #[must_use]
    fn new(engine: CqrsCommandEngine<A, ES>) -> Self {
        Self {
            _phantom: std::marker::PhantomData,
            engine: Arc::new(engine),
        }
    }

    fn read_commands(name: &str, schema: RefOr<Schema>) -> Vec<CommandData> {
        let mut result = vec![];
        if let RefOr::T(t) = &schema {
            match t {
                Schema::Object(o) => {
                    let discriminator = o.properties.iter().find_map(|c| match c.1 {
                        RefOr::T(Schema::Object(o)) => {
                            if let Some(enums) = &o.enum_values {
                                if enums.len() == 1 {
                                    return match &enums[0] {
                                        Value::String(s) => Some((c.0.to_string(), s.clone())),
                                        _ => None,
                                    };
                                }
                            }
                            None
                        }
                        _ => None,
                    });
                    let (current_name, schema_body) = if let Some((f, value)) = &discriminator {
                        match t {
                            Schema::Object(o) => {
                                let mut body = o.clone();
                                body.properties.remove(f);
                                (value.to_string(), Schema::Object(body))
                            }
                            _ => (name.to_string(), t.clone()),
                        }
                    } else {
                        (name.to_string(), t.clone())
                    };
                    result.push(CommandData::new(current_name, schema_body, discriminator));
                }
                Schema::OneOf(items) => {
                    for item in &items.items {
                        result.extend(Self::read_commands(name, item.clone()));
                    }
                }
                Schema::AnyOf(items) => {
                    for item in &items.items {
                        result.extend(Self::read_commands(name, item.clone()));
                    }
                }
                _ => (),
            }
        }
        result
    }

    fn sanitize_route_name(name: &str) -> String {
        let mut result = String::new();
        let mut prev_char: Option<char> = None;
        let mut name_to_process = if let Some(next) = name.strip_suffix("Command") {
            next
        } else {
            name
        };
        name_to_process = if let Some(next) = name_to_process.strip_suffix("Commands") {
            next
        } else {
            name_to_process
        };

        for (i, c) in name_to_process.chars().enumerate() {
            if i == 0 {
                result.push(c.to_ascii_lowercase());
                prev_char = Some(c);
                continue;
            }

            if c.is_uppercase() && prev_char.is_some_and(|pc| pc.is_lowercase()) {
                result.push('-');
            }

            result.push(c.to_ascii_lowercase());
            prev_char = Some(c);
        }
        result
    }

    pub fn routes(engine: CqrsCommandEngine<A, ES>) -> OpenApiRouter {
        let context = CQRSWriteRouter::new(engine);
        let mut schemas = vec![];
        A::schemas(&mut schemas);
        A::CreateCommand::schemas(&mut schemas);
        A::UpdateCommand::schemas(&mut schemas);

        let mut result = OpenApiRouter::<CQRSWriteRouter<A, ES>>::new();

        for CommandData {
            name,
            schema,
            discriminator,
        } in Self::read_commands(&A::CreateCommand::name(), A::CreateCommand::schema())
        {
            let paths = helpers::generate_route(
                A::TYPE,
                HttpMethod::Post,
                format!("/commands/{}", Self::sanitize_route_name(&name)).as_str(),
                A::schema(),
                vec![],
                vec![],
                Some(schema),
            );
            let current_discriminator = discriminator.clone();
            result = result.routes(UtoipaMethodRouter::<CQRSWriteRouter<A, ES>>::from((
                schemas.clone(),
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

        for CommandData {
            name,
            schema,
            discriminator,
        } in Self::read_commands(&A::UpdateCommand::name(), A::UpdateCommand::schema())
        {
            let paths = helpers::generate_route(
                A::TYPE,
                HttpMethod::Put,
                format!(
                    "/{{aggregate_id}}/commands/{}",
                    Self::sanitize_route_name(&name)
                )
                .as_str(),
                A::schema(),
                vec![("aggregate_id", String::schema())],
                vec![],
                Some(schema.clone()),
            );
            let current_discriminator = discriminator.clone();
            result = result.routes(UtoipaMethodRouter::<CQRSWriteRouter<A, ES>>::from((
                schemas.clone(),
                paths,
                put(
                    move |State(router): State<CQRSWriteRouter<A, ES>>,
                          Path(aggregate_id): Path<String>,
                          Extension(context): Extension<CqrsContext>,
                          Json(command): Json<Value>| async {
                        Self::update(
                            router,
                            aggregate_id,
                            command,
                            current_discriminator,
                            context,
                        )
                        .await
                    },
                ),
            )))
        }

        result.with_state(context)
    }

    fn add_discriminator(command: &mut Value, discriminator: Option<(String, String)>) {
        if let Some((name, value)) = discriminator {
            if let Some(obj) = command.as_object_mut() {
                obj.insert(name, value.into());
            }
        }
    }

    fn aggregate_error_to_json(err: AggregateError) -> impl IntoResponse {
        match err {
            AggregateError::UserError(err) => match Value::from_str(err.to_string().as_str()) {
                Ok(value) => (StatusCode::BAD_REQUEST, Json(value)).into_response(),
                Err(_) => (
                    StatusCode::BAD_REQUEST,
                    Json(json!({"error":err.to_string()})),
                )
                    .into_response(),
            },
            AggregateError::Conflict => {
                (StatusCode::CONFLICT, Json(json!({"error": "conflict"}))).into_response()
            }
            AggregateError::DatabaseError(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": err.to_string(), "type": "database" })),
            )
                .into_response(),
            AggregateError::SerializationError(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": err.to_string(), "type": "serialization" })),
            )
                .into_response(),
            AggregateError::UnexpectedError(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "error": err.to_string(), "type": "unexpected" })),
            )
                .into_response(),
        }
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
        Self::add_discriminator(&mut command, discriminator);
        match serde_json::from_value::<A::CreateCommand>(command) {
            Ok(cmd) => match router
                .engine
                .execute_create_with_metadata(cmd, Self::metadata(&context), &context)
                .await
            {
                Ok(result) => {
                    (StatusCode::CREATED, Json(json ! ({"aggregate_id": result}))).into_response()
                }
                Err(err) => Self::aggregate_error_to_json(err).into_response(),
            },
            Err(err) => {
                Self::aggregate_error_to_json(AggregateError::SerializationError(err.into()))
                    .into_response()
            }
        }
    }

    pub async fn update(
        router: CQRSWriteRouter<A, ES>,
        aggregate_id: String,
        mut command: Value,
        discriminator: Option<(String, String)>,
        context: CqrsContext,
    ) -> impl IntoResponse {
        Self::add_discriminator(&mut command, discriminator);
        match serde_json::from_value::<A::UpdateCommand>(command) {
            Ok(cmd) => match router
                .engine
                .execute_update_with_metadata(
                    &aggregate_id,
                    cmd,
                    Self::metadata(&context),
                    &context,
                )
                .await
            {
                Ok(_) => StatusCode::NO_CONTENT.into_response(),
                Err(err) => Self::aggregate_error_to_json(err).into_response(),
            },
            Err(err) => {
                Self::aggregate_error_to_json(AggregateError::SerializationError(err.into()))
                    .into_response()
            }
        }
    }
}
