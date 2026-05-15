use clap::Parser as _;
use jsond::{CliArgs, Server};
use tracing::info;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "jsond=info,tower_http=info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let args = CliArgs::parse();
    Server::run(&args).await
}
