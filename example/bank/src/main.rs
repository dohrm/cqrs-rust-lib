use clap::{Parser, Subcommand};
use bank::api;
use tracing::Level;
use tracing_subscriber::fmt::layer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Registry};

#[derive(Subcommand, Clone, Debug)]
pub enum Commands {
    Start {
        #[clap(short, long, env = "MONGO_URL_SECRET")]
        mongo_uri: String,
        #[clap(long, default_value = "8080")]
        http_port: u16,
    },
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[clap(long, global = true)]
    pub log_level: Option<Level>,
    #[clap(subcommand)]
    pub command: Commands,
}

impl Cli {
    pub async fn execute(&self) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        match &self.command {
            Commands::Start {
                mongo_uri: mongodb_url,
                http_port,
            } => {
                let config = api::AppConfig {
                    mongo_uri: mongodb_url.clone(),
                    http_port: *http_port,
                };

                api::start(config).await
            }
        }
    }
}

fn init_tracing(
    log_level: Level,
) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
    let crate_name = env!("CARGO_CRATE_NAME");
    let filter = EnvFilter::from_default_env()
        .add_directive("debug".parse()?)
        // .add_directive("cqrs_rust_lib=debug".parse()?)
        .add_directive(format!("{crate_name}={log_level}").parse()?);
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

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
    let cli = Cli::parse();
    init_tracing(cli.log_level.unwrap_or(Level::INFO))?;

    cli.execute().await?;
    Ok(())
}
