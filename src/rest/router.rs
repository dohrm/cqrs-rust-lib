use crate::engine::CqrsCommandEngine;
use crate::event_store::EventStore;
use crate::{Aggregate, AggregateError, CqrsContext};
use axum::extract::{Path, State};
use axum::response::IntoResponse;
use axum::routing::{post, put};
use axum::{Extension, Json};
use http::StatusCode;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use utoipa::openapi::path::{OperationBuilder, ParameterBuilder, ParameterIn};
use utoipa::openapi::request_body::RequestBody;
use utoipa::openapi::{
    Content, HttpMethod, PathItem, Paths, PathsBuilder, RefOr, Required, ResponseBuilder, Schema,
};
use utoipa::{PartialSchema, ToSchema};
use utoipa_axum::router::{OpenApiRouter, UtoipaMethodRouter};

#[derive(Clone)]
pub struct CQRSRouter<A, ES>
where
    A: Aggregate + ToSchema,
    ES: EventStore<A>,
{
    _phantom: std::marker::PhantomData<(A, ES)>,
    engine: Arc<CqrsCommandEngine<A, ES>>,
}

impl<A, ES> CQRSRouter<A, ES>
where
    A: Aggregate + ToSchema + 'static,
    ES: EventStore<A> + 'static,
{
    const TYPE: &'static str = A::TYPE;
    #[must_use]
    fn new(engine: CqrsCommandEngine<A, ES>) -> Self {
        Self {
            _phantom: std::marker::PhantomData,
            engine: Arc::new(engine),
        }
    }

    fn method_to_string(method: &HttpMethod) -> &'static str {
        match method {
            HttpMethod::Get => "get",
            HttpMethod::Post => "post",
            HttpMethod::Put => "put",
            HttpMethod::Delete => "delete",
            HttpMethod::Patch => "patch",
            HttpMethod::Options => "options",
            HttpMethod::Head => "head",
            HttpMethod::Trace => "trace",
        }
    }

    fn generate_route(
        method: HttpMethod,
        path: &str,
        response: RefOr<Schema>,
        path_parameters: Vec<(&str, RefOr<Schema>)>,
        query_parameters: Vec<(&str, RefOr<Schema>, bool)>,
        body: Option<Schema>,
    ) -> Paths {
        let code = match &method {
            HttpMethod::Post => "201",
            _ => "200",
        };
        let mut operation = OperationBuilder::new()
            .response(
                code,
                ResponseBuilder::new().content("application/json", Content::new(Some(response))),
            )
            .operation_id(Some(format!(
                "{}-{}-{}",
                Self::TYPE,
                Self::method_to_string(&method),
                path.replace("/", "-")
            )))
            // .deprecated(Some(Deprecated::False))
            // .summary(Some("Summary"))
            // .description(Some("Description"))
            .tag(Self::TYPE);

        for (name, schema) in path_parameters {
            operation = operation.parameter(
                ParameterBuilder::new()
                    .name(name)
                    .parameter_in(ParameterIn::Path)
                    .required(Required::True)
                    // .deprecated(Some(Deprecated::False))
                    // .description(Some("xxx"))
                    .schema(Some(schema)),
            );
        }

        for (name, schema, required) in query_parameters {
            operation = operation.parameter(
                ParameterBuilder::new()
                    .name(name)
                    .parameter_in(ParameterIn::Query)
                    .required(if required {
                        Required::True
                    } else {
                        Required::False
                    })
                    // .deprecated(Some(Deprecated::False))
                    // .description(Some("xxx"))
                    .schema(Some(schema)),
            );
        }
        if let Some(body) = body {
            operation = operation.request_body(Some(
                RequestBody::builder()
                    .content("application/json", Content::new(Some(body)))
                    .build(),
            ));
        }
        PathsBuilder::new()
            .path(path, PathItem::new(method, operation.build()))
            .build()
    }

    fn read_commands(
        name: &str,
        schema: RefOr<Schema>,
    ) -> Vec<(String, Schema, Option<(String, String)>)> {
        let mut result = vec![];
        match &schema {
            RefOr::T(t) => match t {
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
                    result.push((current_name, schema_body, discriminator));
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
            },
            _ => (),
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

            if c.is_uppercase() && prev_char.map_or(false, |pc| pc.is_lowercase()) {
                result.push('-');
            }

            result.push(c.to_ascii_lowercase());
            prev_char = Some(c);
        }
        result
    }

    pub fn routes(engine: CqrsCommandEngine<A, ES>) -> OpenApiRouter {
        let context = CQRSRouter::new(engine);
        let mut schemas = vec![];
        A::schemas(&mut schemas);
        A::CreateCommand::schemas(&mut schemas);
        A::UpdateCommand::schemas(&mut schemas);

        let mut result = OpenApiRouter::<CQRSRouter<A, ES>>::new();

        for (name, schema, discriminator) in
            Self::read_commands(&A::CreateCommand::name(), A::CreateCommand::schema())
        {
            let paths = Self::generate_route(
                HttpMethod::Post,
                format!("/commands/{}", Self::sanitize_route_name(&name)).as_str(),
                A::schema(),
                vec![],
                vec![],
                Some(schema),
            );
            let current_discriminator = discriminator.clone();
            result = result.routes(UtoipaMethodRouter::<CQRSRouter<A, ES>>::from((
                schemas.clone(),
                paths,
                post(
                    move |State(router): State<CQRSRouter<A, ES>>,
                          Extension(context): Extension<CqrsContext>,
                          Json(command): Json<Value>| async {
                        Self::create(router, command, current_discriminator, context).await
                    },
                ),
            )))
        }

        for (name, schema, discriminator) in
            Self::read_commands(&A::UpdateCommand::name(), A::UpdateCommand::schema())
        {
            let paths = Self::generate_route(
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
            result = result.routes(UtoipaMethodRouter::<CQRSRouter<A, ES>>::from((
                schemas.clone(),
                paths,
                put(
                    move |State(router): State<CQRSRouter<A, ES>>,
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
        router: CQRSRouter<A, ES>,
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
        router: CQRSRouter<A, ES>,
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
