use core::net::Ipv4Addr;
use std::path::PathBuf;

use crate::error::Error;
use crate::id::IdStrategy;

#[derive(Clone, Debug)]
pub struct Args {
    /// Path to the JSON or JSON5 database file
    pub db: PathBuf,

    /// Port to listen on (0 = random available port)
    pub port: u16,

    /// Host address to bind to
    pub host: Ipv4Addr,

    /// Serve static files from this directory
    pub r#static: PathBuf,

    /// Add artificial delay in milliseconds to all responses
    pub delay: u64,

    /// Watch the database file for changes and reload automatically
    pub watch: bool,

    /// Enable or disable CORS headers
    pub cors: bool,

    /// Readonly mode: disable POST, PUT, PATCH, DELETE
    pub readonly: bool,

    pub ids: IdStrategy,

    /// Number of items per page for pagination (default 10)
    pub per_page: usize,
}

impl Args {
    pub fn parse() -> Result<Self, Error> {
        use lexopt::Parser;
        use lexopt::prelude::*;

        let mut db: Option<PathBuf> = None;
        let mut args = Self::default();

        // Environment variables act as defaults (before command-line parsing)
        if let Ok(port_env) = std::env::var("PORT") {
            if let Ok(port) = port_env.parse() {
                args.port = port;
            }
        }
        if let Ok(host_env) = std::env::var("HOST") {
            if let Ok(host) = host_env.parse() {
                args.host = host;
            }
        }

        let mut parser = Parser::from_env();

        while let Some(arg) = parser.next()? {
            match arg {
                Short('p') | Long("port") => {
                    args.port = parser.value()?.parse()?;
                }
                Long("host") => {
                    args.host = parser.value()?.parse()?;
                }
                Short('s') | Long("static") => {
                    args.r#static = parser.value()?.into();
                }
                Long("delay") => {
                    args.delay = parser.value()?.parse()?;
                }
                Short('w') | Long("watch") => {
                    args.watch = true;
                }
                Long("cors") => {
                    args.cors = true;
                }
                Long("readonly") => {
                    args.readonly = true;
                }
                Long("ids") => {
                    let val = parser.value()?.string()?;
                    args.ids = match val.as_ref() {
                        "int" => IdStrategy::Int,
                        "v4" => IdStrategy::Uuidv4,
                        "v7" => IdStrategy::Uuidv7,
                        _ => {
                            return Err(Error::BadRequest(
                                "ids must be one of: int, uuidv4, uuidv7".to_owned(),
                            ));
                        }
                    };
                }
                Long("per-page") => {
                    args.per_page = parser.value()?.parse()?;
                }
                Value(val) if db.is_none() => {
                    db = Some(val.string()?.into());
                }
                Short('h') | Long("help") => {
                    print_help();
                    std::process::exit(0);
                }
                Short('V') | Long("version") => {
                    println!("jsond {}", env!("CARGO_PKG_VERSION"));
                    std::process::exit(0);
                }
                _ => return Err(Error::Cli(arg.unexpected())),
            }
        }

        args.db = db.unwrap_or_else(|| {
            std::env::current_dir().map_or_else(|_| PathBuf::from("db.json"), |p| p.join("db.json"))
        });

        Ok(args)
    }
}

fn print_help() {
    eprint!(
        "\
jsond - Fast mock JSON REST server

Usage:
  jsond [OPTIONS] [DB_PATH]

Arguments:
  [DB_PATH] Path to the JSON or JSON5 database file [default: <cwd>/db.json]

Options:
  -p, --port <PORT>                Port to listen on (0 = random) [env: PORT=] [default: 3000]
      --host <HOST>                Host address to bind to [env: HOST=] [default: 127.0.0.1]
  -s, --static <PATH>              Serve static files from this directory [default: public]
      --delay <DELAY>              Add artificial delay in milliseconds to all responses [default: \
         0]
  -w, --watch                      Watch the database file for changes and reload automatically
      --cors                       Enable/Disable CORS headers [default: false]
      --readonly                   Readonly mode: disable POST, PUT, PATCH, DELETE
      --ids <ID_STRATEGY>          The id strategy to use [default: uuidv7] [possible values: int, \
         v4, v7]
      --per-page <PER_PAGE>        Number of items per page [default: 10]
  -h, --help                       Print help
  -V, --version                    Print version
"
    );
}

impl Default for Args {
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
            ids: IdStrategy::Uuidv7,
            per_page: 10,
        }
    }
}
