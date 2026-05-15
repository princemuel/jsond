use core::net::Ipv4Addr;
use std::path::PathBuf;

use crate::ids::IdStrategy;

#[derive(Clone, Debug, clap::Parser)]
#[command(version, about, long_about = None)]
pub struct CliArgs {
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
    pub r#static: PathBuf,

    /// Add artificial delay in milliseconds to all responses
    #[arg(long, default_value_t = 0)]
    pub delay: u64,

    /// Watch the database file for changes and reload automatically
    #[arg(short, long, default_value_t = true)]
    pub watch: bool,

    /// Enable CORS headers
    #[arg(long, default_value_t = true)]
    pub cors: bool,

    /// Readonly mode: disable POST, PUT, PATCH, DELETE
    #[arg(long, default_value_t = false)]
    pub readonly: bool,

    #[arg(long, value_enum, default_value_t = CliIdStrategy::Uuidv7)]
    pub id_strategy: CliIdStrategy,

    /// Number of items per page for pagination (default 10)
    #[arg(long, default_value_t = 10)]
    pub per_page: usize,
}

#[cfg(test)]
impl Default for CliArgs {
    fn default() -> Self {
        Self {
            db: PathBuf::from("/tmp/db.json"),
            port: 3000,
            host: Ipv4Addr::LOCALHOST,
            r#static: PathBuf::from("public"),
            delay: 0,
            watch: false,
            cors: false,
            readonly: false,
            id_strategy: CliIdStrategy::Int,
            per_page: 10,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum CliIdStrategy {
    Int,
    Uuidv4,
    Uuidv7,
}

impl From<CliIdStrategy> for IdStrategy {
    fn from(value: CliIdStrategy) -> Self {
        match value {
            CliIdStrategy::Uuidv4 => Self::Uuidv4,
            CliIdStrategy::Uuidv7 => Self::Uuidv7,
            CliIdStrategy::Int => Self::Int,
        }
    }
}
