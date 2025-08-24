use clap::{Parser, Subcommand};
use todolist::api;
use tracing::Level;
use tracing_subscriber::fmt::layer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Registry};

#[derive(Subcommand, Clone, Debug)]
pub enum Commands {
    Start {
        #[clap(short, long, env = "PG_URL_SECRET")]
        pg_uri: String,
        #[clap(long, default_value = "8081")]
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
            Commands::Start { pg_uri, http_port } => {
                let config = api::AppConfig {
                    pg_uri: pg_uri.clone(),
                    http_port: *http_port,
                };

                api::start(config).await
            }
        }
    }
}

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
    let cli = Cli::parse();
    init_tracing(cli.log_level.unwrap_or(Level::INFO));

    cli.execute().await?;
    Ok(())
}
