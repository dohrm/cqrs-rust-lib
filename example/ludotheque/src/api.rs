use crate::game::{Game, GameQuery, GameView};
use axum::body::Body;
use axum::extract::Request;
use axum::middleware::Next;
use axum::response::{Redirect, Response};
use axum::routing::get;
use axum::{middleware, Json, Router};
use cqrs_rust_lib::dispatchers::ViewDispatcher;
use cqrs_rust_lib::es::EventStoreImpl;
use cqrs_rust_lib::prelude::surrealdb as db;
use cqrs_rust_lib::rest::{CQRSAuditLogRouter, CQRSReadRouter, CQRSWriteRouter};
use cqrs_rust_lib::{Aggregate, CqrsCommandEngine, CqrsContext, Dispatcher};
use http::header::CONTENT_TYPE;
use http::StatusCode;
use std::sync::Arc;
use surrealdb::engine::any::connect;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace;
use tower_http::trace::TraceLayer;
use tracing::{error, info, Level};
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_swagger_ui::SwaggerUi;

const DOC_PATH: &str = "/@/doc";
const OPENAPI_PATH: &str = "/openapi.json";
const GAMES_PATH: &str = "/games";

#[derive(OpenApi)]
#[openapi(paths(openapi_json), nest(), components(schemas()))]
pub struct ApiDoc;

#[utoipa::path(
    get,
    path = format!("{DOC_PATH}{OPENAPI_PATH}"),
    responses((status = 200, description = "JSON file", body = ()))
)]
#[allow(dead_code)]
async fn openapi_json() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::openapi())
}

pub struct AppConfig {
    pub http_port: u16,
    pub surreal_uri: String,
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

pub async fn start(config: AppConfig) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let cors = CorsLayer::new()
        .allow_methods(Any)
        .allow_origin(Any)
        .allow_headers([CONTENT_TYPE]);

    let context_middleware = middleware::from_fn(context_middleware);

    info!("Starting server on http://localhost:{}", config.http_port);

    // Connect to SurrealDB (embedded mem:// or any URI)
    let db = connect(&config.surreal_uri).await?;
    db.use_ns("ludotheque").use_db("ludotheque").await?;

    // Ensure event store tables exist
    db.query(db::EventStorePersist::<Game>::schema())
        .await?
        .check()?;

    // Ensure view table exists
    db.query("DEFINE TABLE IF NOT EXISTS game_view SCHEMALESS;")
        .await?
        .check()?;

    // Event store
    let es_persist = db::EventStorePersist::<Game>::new(db.clone());
    let event_store = EventStoreImpl::new(es_persist);

    // View storage (shared between read router and dispatcher)
    let view_storage: Arc<db::ReadStorage<GameView, GameQuery>> =
        Arc::new(db::ReadStorage::new(db.clone(), Game::TYPE, "game_view"));

    // Dispatcher: keeps GameView in sync with Game events
    let view_dispatcher = ViewDispatcher::<Game, GameView, GameQuery>::new(view_storage.clone());

    // CQRS engine
    let effects: Vec<Box<dyn Dispatcher<Game> + Send + Sync>> = vec![Box::new(view_dispatcher)];
    let engine = Arc::new(CqrsCommandEngine::new(
        event_store.clone(),
        effects,
        (),
        Box::new(|e| {
            error!("something went wrong: {}", e);
        }),
    ));

    // Routers
    let read_router = CQRSReadRouter::routes(view_storage, Game::TYPE);
    let write_router = CQRSWriteRouter::routes(engine);
    let audit_router = CQRSAuditLogRouter::routes(event_store, Game::TYPE);

    // Assemble
    let (router, api) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(trace::DefaultMakeSpan::new().level(Level::INFO))
                .on_response(trace::DefaultOnResponse::new().level(Level::INFO)),
        )
        .nest(GAMES_PATH, read_router)
        .nest(GAMES_PATH, write_router)
        .nest(GAMES_PATH, audit_router)
        .split_for_parts();

    let router = router
        .layer(context_middleware)
        .merge(SwaggerUi::new(DOC_PATH).url(format!("{DOC_PATH}{OPENAPI_PATH}"), api))
        .fallback_service(Router::new().route("/", get(|| async { Redirect::to(DOC_PATH) })))
        .layer(cors);

    let listener = tokio::net::TcpListener::bind(("0.0.0.0", config.http_port)).await?;

    Ok(axum::serve(listener, router).await?)
}
