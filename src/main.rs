use anyhow::Result;
use clap::Parser as _;
use jsond::Server;
use jsond::cli::Args;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialise structured logging; respect RUST_LOG env var.
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "jsond=info,tower_http=info".into()),
        )
        .init();

    let args = Args::parse();
    Server::run(&args).await
}
