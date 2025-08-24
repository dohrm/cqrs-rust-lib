use crate::account::views::{Movement, MovementQuery};
use crate::account::{Account, AccountQuery};
use crate::query_builder_account::QueryBuilderAccount;
use crate::query_builder_movement::QueryBuilderMovement;
use axum::body::Body;
use axum::extract::Request;
use axum::middleware::Next;
use axum::response::{Redirect, Response};
use axum::routing::get;
use axum::{middleware, Json, Router};
use cqrs_rust_lib::dispatchers::ViewDispatcher;
use cqrs_rust_lib::es::mongodb::MongoDBPersist;
use cqrs_rust_lib::es::EventStoreImpl;
use cqrs_rust_lib::read::mongodb::{MongoDBFromSnapshotStorage, MongoDbStorage};
use cqrs_rust_lib::rest::{CQRSReadRouter, CQRSWriteRouter};
use cqrs_rust_lib::{Aggregate, CqrsCommandEngine, CqrsContext, Dispatcher, Snapshot};
use http::header::CONTENT_TYPE;
use http::StatusCode;
use mongodb::options::ClientOptions;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace;
use tower_http::trace::TraceLayer;
use tracing::{error, info, Level};
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_swagger_ui::SwaggerUi;

const DOC_PATH: &str = "/@/doc";
const OPENAPI_PATH: &str = "/openapi.json";
const ACCOUNTS_PATH: &str = "/accounts";

#[derive(OpenApi)]
#[openapi(paths(openapi_json), nest(), components(schemas()))]
pub struct ApiDoc;

#[utoipa::path(
    get,
    path = format!("{DOC_PATH}{OPENAPI_PATH}"),
    responses(
            (status = 200, description = "JSON file", body = ())
    )
)]
#[allow(dead_code)]
async fn openapi_json() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::openapi())
}

pub struct AppConfig {
    pub http_port: u16,
    pub mongo_uri: String,
}

#[derive(Debug, Clone)]
pub enum AuthenticatedUser {
    #[allow(dead_code)]
    User(String),
    Anonymous,
}

impl AuthenticatedUser {
    pub fn username(&self) -> String {
        match self {
            AuthenticatedUser::User(username) => username.clone(),
            AuthenticatedUser::Anonymous => "anonymous".to_string(),
        }
    }
}

pub async fn context_middleware(
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let extensions = req.extensions_mut();
    let current_user = extensions
        .get::<AuthenticatedUser>()
        .unwrap_or(&AuthenticatedUser::Anonymous);

    let context = CqrsContext::new(Some(current_user.username())).with_next_request_id();
    extensions.insert(context);

    Ok(next.run(req).await)
}

async fn mongo_database(
    uri: &str,
) -> Result<mongodb::Database, Box<dyn std::error::Error + Send + Sync>> {
    let options = ClientOptions::parse(uri).await?;
    let client = mongodb::Client::with_options(options.clone())?;

    Ok(client.database(&options.default_database.unwrap()))
}

pub async fn start(config: AppConfig) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cors = CorsLayer::new()
        .allow_methods(Any)
        .allow_origin(Any)
        .allow_headers([CONTENT_TYPE]);

    // Can create middleware to extract the current user from JWT or Basic.
    let context_middleware = middleware::from_fn(context_middleware);

    // Prepare dependencies.

    info!("Starting server on  http://localhost:{}", config.http_port);
    // Initialize Dependency Injection.
    let database = mongo_database(&config.mongo_uri).await?;
    // Storages
    let account_es_store = MongoDBPersist::<Account>::new(database.clone());

    let account_repository = Arc::new(MongoDBFromSnapshotStorage::new(Arc::new(MongoDbStorage::<
        Snapshot<Account>,
        AccountQuery,
        QueryBuilderAccount,
    >::new(
        database.clone(),
        "accounts",
        QueryBuilderAccount,
        account_es_store.snapshot_collection_name(),
    ))));

    let movement_repository = Arc::new(MongoDbStorage::<
        Movement,
        MovementQuery,
        QueryBuilderMovement,
    >::new(
        database.clone(),
        "movements",
        QueryBuilderMovement,
        "movements_view",
    ));
    let movement_dispatcher = ViewDispatcher::new(movement_repository.clone());

    // CQRS Command
    let accounts_event_store = EventStoreImpl::new(account_es_store);
    let accounts_effects: Vec<Box<dyn Dispatcher<Account>>> = vec![Box::new(movement_dispatcher)];
    let accounts_engine = Arc::new(CqrsCommandEngine::new(
        accounts_event_store,
        accounts_effects,
        (),
        Box::new(|e| {
            error!("something went wrong: {}", e);
        }),
    ));

    // Initialize routers
    let accounts_read_router = CQRSReadRouter::routes(account_repository, Account::TYPE);
    let movements_read_router = CQRSReadRouter::routes(movement_repository, Account::TYPE);
    let accounts_write_router = CQRSWriteRouter::routes(accounts_engine);

    // Prepare router
    let (router, api) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(trace::DefaultMakeSpan::new().level(Level::INFO))
                .on_response(trace::DefaultOnResponse::new().level(Level::INFO)),
        )
        // .routes(routes!(health))
        .nest(ACCOUNTS_PATH, accounts_read_router)
        .nest(ACCOUNTS_PATH, movements_read_router)
        .nest(ACCOUNTS_PATH, accounts_write_router)
        .split_for_parts();

    let router = router
        .layer(context_middleware)
        // .layer(auth_middleware)
        .merge(SwaggerUi::new(DOC_PATH).url(format!("{DOC_PATH}{OPENAPI_PATH}"), api))
        .fallback_service(Router::new().route("/", get(|| async { Redirect::to(DOC_PATH) })))
        .layer(cors);

    let listener = tokio::net::TcpListener::bind(("0.0.0.0", config.http_port)).await?;

    Ok(axum::serve(listener, router).await?)
}
