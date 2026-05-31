use jsond::{Error, Server};

#[tokio::main]
async fn main() -> Result<(), Error> { Server::run().await }
