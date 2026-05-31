use tokio::net::TcpListener;

use crate::cli::Args;
use crate::db::Database;
use crate::router::build_router;
use crate::{Error, telemetry, watcher};

#[derive(Clone, Copy)]
pub struct Server;

impl Server {
    pub async fn run() -> Result<(), Error> {
        telemetry::init();
        let args = Args::parse()?;

        let db = Database::load(&args.db, args.ids, args.readonly)?;
        let resources = db.resources().await;

        tracing::info!(
            database = %args.db.display(),
            resources = resources.len(),
            "database loaded"
        );

        if args.watch {
            watcher::spawn(&args.db.canonicalize()?, &db);
            tracing::info!("watching {} for changes", args.db.display());
        }

        let router = build_router(&db, &args);

        let tcp = {
            let mut port = args.port;
            loop {
                let addr = format!("{}:{}", args.host, port);
                match TcpListener::bind(&addr).await {
                    Ok(listener) => break listener,
                    Err(_) if port != 0 => {
                        tracing::warn!("port {port} in use, trying {next}", next = port + 1);
                        port += 1;
                    }
                    Err(e) => return Err(e.into()),
                }
            }
        };

        let addr = tcp.local_addr()?;

        tracing::info!("");
        tracing::info!("  ┌──────────────────────────────────────────┐");
        tracing::info!("  │   jsond                         │");
        tracing::info!("  │   http://{}:{:<25}│", addr.ip(), addr.port());
        tracing::info!("  │   id strategy: {:<25}│", format!("{:?}", args.ids));
        tracing::info!("  ├──────────────────────────────────────────┤");
        for r in &resources {
            tracing::info!("  │   /{:<40}│", r);
        }
        tracing::info!("  └──────────────────────────────────────────┘");

        axum::serve(tcp, router).with_graceful_shutdown(shutdown_signal()).await?;

        println!("\n\x1b[33m  Shutting down...\x1b[0m\n");
        tracing::info!("server stopped");

        Ok(())
    }
}

// fn print_banner(addr: &SocketAddr, args: &Args, resources: &[String]) {
//     println!();
//     println!("\x1b[1;32m  JSOND Http Server started\x1b[0m");
//     println!();
//     println!(
//         "\x1b[90m  >\x1b[0m Local:   \x1b[36mhttp://{}:{}/\x1b[0m",
//         args.host,
//         addr.port()
//     );
//     println!("\x1b[90m  >\x1b[0m Network: \x1b[36mhttp://{addr}/\x1b[0m");
//     println!();

//     if resources.is_empty() {
//         println!("  \x1b[33m  No resources found.\x1b[0m");
//     } else {
//         println!("  \x1b[1mEndpoints:\x1b[0m");
//         println!();

//         for name in resources {
//             println!(
//                 "\x1b[90m  >\x1b[0m \x1b[36mhttp://{}:{}/{}\x1b[0m",
//                 args.host,
//                 addr.port(),
//                 name
//             );
//         }
//     }

//     println!();

//     if args.watch {
//         println!("  \x1b[90m>\x1b[0m Watching for file changes...");
//     }

//     if args.readonly {
//         println!("  \x1b[33m>\x1b[0m \x1b[1mReadonly mode\x1b[0m — write
// operations disabled");     }

//     println!();
//     println!("  Press Ctrl+C to stop");
//     println!();
// }

/// Waits for a shutdown signal, then allows a brief grace period before
/// returning.
///
///
/// # Panics
///
/// Panics if the OS refuses to install the signal handler. This should only
/// occur if the process has already registered the maximum number of signal
/// handlers, which is exceptionally rare in practice.
#[expect(clippy::expect_used, clippy::integer_division_remainder_used)]
async fn shutdown_signal() {
    use tokio::{signal, time};

    let ctrl_c = async {
        signal::ctrl_c().await.expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix;

        unix::signal(unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        () = ctrl_c => {},
        () = terminate => {},
    }

    tracing::info!("Shutdown signal received, starting graceful shutdown...");

    // Allow time for load balancer to detect
    time::sleep(time::Duration::from_secs(5)).await;
}
