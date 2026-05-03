use ludotheque::api;
use tracing::Level;
use tracing_subscriber::fmt::layer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Registry};

fn init_tracing(log_level: Level) {
    let crate_name = env!("CARGO_CRATE_NAME");
    let filter = EnvFilter::from_default_env()
        .add_directive("debug".parse().unwrap())
        .add_directive(format!("{crate_name}={log_level}").parse().unwrap());
    let l = layer::<Registry>();
    let registry = tracing_subscriber::registry();

    registry
        .with(
            l.with_file(true)
                .with_line_number(true)
                .with_target(false)
                .compact(),
        )
        .with(filter)
        .init();
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
    init_tracing(Level::INFO);

    let http_port: u16 = std::env::var("HTTP_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(8080);

    let surreal_uri = std::env::var("SURREAL_URI").unwrap_or_else(|_| "mem://".to_string());

    api::start(api::AppConfig {
        http_port,
        surreal_uri,
    })
    .await?;
    Ok(())
}
