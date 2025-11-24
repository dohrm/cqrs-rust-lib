use crate::query_builder_todolist::QueryBuilderTodoList;
use crate::todolist::query::TodoListQuery;
use crate::todolist::TodoList;
use axum::body::Body;
use axum::extract::Request;
use axum::middleware::Next;
use axum::response::{Redirect, Response};
use axum::routing::get;
use axum::{middleware, Json, Router};
use cqrs_rust_lib::es::postgres::PostgresPersist;
use cqrs_rust_lib::es::EventStoreImpl;
use cqrs_rust_lib::read::postgres::{PostgresFromSnapshotStorage, PostgresStorage};
use cqrs_rust_lib::rest::{CQRSAuditLogRouter, CQRSReadRouter, CQRSWriteRouter};
use cqrs_rust_lib::{Aggregate, CqrsCommandEngine, CqrsContext, Dispatcher};
use http::header::CONTENT_TYPE;
use http::StatusCode;
use std::sync::Arc;
use tokio_postgres::NoTls;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace;
use tower_http::trace::TraceLayer;
use tracing::{error, info, Level};
use utoipa::OpenApi;
use utoipa_axum::router::OpenApiRouter;
use utoipa_swagger_ui::SwaggerUi;

const DOC_PATH: &str = "/@/doc";
const OPENAPI_PATH: &str = "/openapi.json";
const LISTS_PATH: &str = "/lists";

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
    pub pg_uri: String,
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

    // Connect to Postgres
    let (client, connection) = tokio_postgres::connect(&config.pg_uri, NoTls).await?;
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("Postgres connection error: {}", e);
        }
    });
    let client = Arc::new(client);

    // Ensure tables exist
    client
        .batch_execute(
            r#"
            CREATE TABLE IF NOT EXISTS todolist_snapshots (
                aggregate_id TEXT PRIMARY KEY,
                data JSONB NOT NULL,
                version BIGINT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS todolist_journal (
                event_id TEXT PRIMARY KEY,
                aggregate_id TEXT NOT NULL,
                version BIGINT NOT NULL,
                payload JSONB NOT NULL,
                metadata JSONB NOT NULL,
                at TIMESTAMPTZ NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_todolist_journal_agg_ver ON todolist_journal(aggregate_id, version);
            "#,
        )
        .await?;

    // Storages
    let es_store = PostgresPersist::<TodoList>::new(client.clone());
    let repository = Arc::new(PostgresFromSnapshotStorage::<
        TodoList,
        TodoListQuery,
        QueryBuilderTodoList,
    >::new(Arc::new(PostgresStorage::new(
        client.clone(),
        TodoList::TYPE,
        QueryBuilderTodoList,
        es_store.snapshot_table_name(),
    ))));

    // CQRS Command
    let event_store = EventStoreImpl::new(es_store);
    let effects: Vec<Box<dyn Dispatcher<TodoList>>> = vec![];
    let engine = Arc::new(CqrsCommandEngine::new(
        event_store.clone(),
        effects,
        (),
        Box::new(|e| {
            error!("something went wrong: {}", e);
        }),
    ));

    // Routers
    let read_router = CQRSReadRouter::routes(repository, TodoList::TYPE);
    let write_router = CQRSWriteRouter::routes(engine);
    let audit_router = CQRSAuditLogRouter::routes(event_store, TodoList::TYPE);

    // Prepare router
    let (router, api) = OpenApiRouter::with_openapi(ApiDoc::openapi())
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(trace::DefaultMakeSpan::new().level(Level::INFO))
                .on_response(trace::DefaultOnResponse::new().level(Level::INFO)),
        )
        .nest(LISTS_PATH, read_router)
        .nest(LISTS_PATH, write_router)
        .nest(LISTS_PATH, audit_router)
        .split_for_parts();

    let router = router
        .layer(context_middleware)
        .merge(SwaggerUi::new(DOC_PATH).url(format!("{DOC_PATH}{OPENAPI_PATH}"), api))
        .fallback_service(Router::new().route("/", get(|| async { Redirect::to(DOC_PATH) })))
        .layer(cors);

    let listener = tokio::net::TcpListener::bind(("0.0.0.0", config.http_port)).await?;

    Ok(axum::serve(listener, router).await?)
}
