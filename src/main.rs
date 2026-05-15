use clap::Parser as _;
use jsond::{CliArgs, Server};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = CliArgs::parse();
    Server::run(&args).await
}
