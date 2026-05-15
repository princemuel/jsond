//! Top-level server: binds to a port, starts Axum, optionally watches the
//! database file for changes and reloads it live.

use core::net::SocketAddr;

use tokio::net::TcpListener;
use tokio::signal;

use crate::cli::CliArgs;
use crate::db::Database;
use crate::router::build_router;
use crate::{telemetry, watcher};

#[derive(Clone, Copy)]
pub struct Server;

impl Server {
    pub async fn run(args: &CliArgs) -> anyhow::Result<()> {
        telemetry::init();
        let db = Database::load(&args.db, args.id_strategy.into(), args.readonly)?;
        let resources = db.resources().await;

        tracing::info!(
            file = %args.db.display(),
            resource_count = resources.len(),
            "database loaded"
        );

        if args.watch {
            watcher::spawn(&args.db.canonicalize()?, &db);
            tracing::info!("watching {} for changes", args.db.display());
        }

        let router = build_router(&db, args);

        let addr: SocketAddr = format!("{}:{}", args.host, args.port).parse()?;
        let tcp = TcpListener::bind(&addr).await?;

        print_banner(&addr, args, &resources);

        axum::serve(tcp, router)
            .with_graceful_shutdown(shutdown_signal())
            .await?;

        println!("\n\x1b[33m  Shutting down...\x1b[0m\n");
        tracing::info!("server stopped");

        Ok(())
    }
}

fn print_banner(addr: &SocketAddr, args: &CliArgs, resources: &[String]) {
    println!();
    println!("\x1b[1;32m  JSON Server started\x1b[0m");
    println!();
    println!(
        "\x1b[90m  >\x1b[0m Local:   \x1b[36mhttp://{}:{}/\x1b[0m",
        args.host,
        addr.port()
    );
    println!("\x1b[90m  >\x1b[0m Network: \x1b[36mhttp://{addr}/\x1b[0m");
    println!();

    if resources.is_empty() {
        println!("  \x1b[33m  No resources found.\x1b[0m");
    } else {
        println!("  \x1b[1mEndpoints:\x1b[0m");
        println!();
        for name in resources {
            println!(
                "\x1b[90m  >\x1b[0m \x1b[36mhttp://{}:{}/{}\x1b[0m",
                args.host,
                addr.port(),
                name
            );
        }
    }

    println!();

    if args.watch {
        println!("  \x1b[90m>\x1b[0m Watching for file changes...");
    }

    if args.readonly {
        println!("  \x1b[33m>\x1b[0m \x1b[1mReadonly mode\x1b[0m — write operations disabled");
    }

    println!();
    println!("  Press Ctrl+C to stop");
    println!();
}

#[expect(clippy::integer_division_remainder_used)]
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
