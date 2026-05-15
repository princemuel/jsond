use jsond::Server;

#[tokio::main]
async fn main() -> anyhow::Result<()> { Server::run().await }
