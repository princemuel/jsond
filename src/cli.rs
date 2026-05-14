use core::net::Ipv4Addr;
use std::path::PathBuf;

use clap::Parser;

#[derive(Clone, Debug, Parser)]
#[command(version, about, long_about = None)]
pub struct Args {
    /// Path to the JSON or JSON5 database file
    #[arg(default_value = "db.json")]
    pub db: PathBuf,

    /// Port to listen on (0 = random available port)
    #[arg(short, long, default_value_t = 3000, env = "PORT")]
    pub port: u16,

    /// Host address to bind to
    #[arg(long, default_value = "127.0.0.1", env = "HOST")]
    pub host: Ipv4Addr,

    /// Serve static files from this directory
    #[arg(short, long, default_value = "public")]
    pub static_dir: PathBuf,

    /// Add artificial delay in milliseconds to all responses
    #[arg(long, default_value_t = 0)]
    pub delay: u64,

    /// Watch the database file for changes and reload automatically
    #[arg(short, long, default_value_t = false)]
    pub watch: bool,

    /// Disable CORS headers
    #[arg(long, default_value_t = false)]
    pub no_cors: bool,

    /// Readonly mode: disable POST, PUT, PATCH, DELETE
    #[arg(long, default_value_t = false)]
    pub readonly: bool,

    /// Number of items per page for pagination (default 10)
    #[arg(long, default_value_t = 10)]
    pub per_page: usize,
}

#[cfg(test)]
impl Default for Args {
    fn default() -> Self {
        Self {
            db: PathBuf::from("/tmp/db.json"),
            port: 3000,
            host: Ipv4Addr::LOCALHOST,
            static_dir: PathBuf::from("public"),
            delay: 0,
            watch: false,
            no_cors: false,
            readonly: false,
            per_page: 10,
        }
    }
}
