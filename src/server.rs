//! Top-level server: binds to a port, starts Axum, optionally watches the
//! database file for changes and reloads it live.

use core::net::SocketAddr;

use tokio::net::TcpListener;

use crate::cli::CliArgs;
use crate::db::Database;
use crate::router::build_router;
use crate::watcher;

pub struct Server;
impl Server {
    pub async fn run(args: &CliArgs) -> anyhow::Result<()> {
        let db = Database::load(&args.db, args.id_strategy.into(), args.readonly)?;
        let resources = db.resources().await;

        tracing::info!(
            "loaded '{}' — {} resource(s)",
            args.db.display(),
            resources.len()
        );

        if args.watch {
            watcher::spawn(&args.db.canonicalize()?, &db);
        }

        let router = build_router(&db, &args);

        let addr: SocketAddr = format!("{}:{}", args.host, args.port).parse()?;
        let tcp = TcpListener::bind(&addr).await?;

        println!();
        println!("  \x1b[1;32mjsond\x1b[0m started on PORT :{}", addr.port());
        println!("  \x1b[36mhttp://{}\x1b[0m", addr);
        println!();

        tracing::info!("");
        tracing::info!("  ┌──────────────────────────────────────────┐");
        tracing::info!("  │   jsond                                  │");
        tracing::info!("  │   http://{}:{:<25}│", args.host, addr.port());
        tracing::info!(
            "  │   id strategy: {:<25}│",
            format!("{:?}", args.id_strategy)
        );
        tracing::info!("  ├──────────────────────────────────────────┤");

        for resource in &resources {
            tracing::info!("  │   /{:<40}│", resource);
        }

        tracing::info!("  └──────────────────────────────────────────┘");
        tracing::info!("");

        if args.readonly {
            tracing::info!("  \x1b[31mReadonly mode: write operations are disabled\x1b[0m");
        }
        tracing::info!("  Press Ctrl+C to stop");

        axum::serve(tcp, router).await?;

        Ok(())
    }
}

// async fn print_banner(addr: SocketAddr, db: &RwLockReadGuard<'_, Database>) {
//     println!();
//     println!("  \x1b[1;32mjsond\x1b[0m started on PORT :{}", addr.port());
//     println!("  \x1b[36mhttp://{}\x1b[0m", addr);
//     println!();

//     let resources = db.resources().await;
//     if resources.is_empty() {
//         println!("  (no resources found)");
//     } else {
//         println!("  Resources:");
//         for name in &resources {
//             if db.is_collection(name).await {
//                 println!("  \x1b[33mhttp://{}/{}\x1b[0m", addr, name);
//             } else {
//                 println!("  \x1b[35mhttp://{}/{}\x1b[0m  (singleton)", addr, name);
//             }
//         }
//     }
//     println!();

//     if db {
//         println!("  \x1b[31mReadonly mode: write operations are
// disabled\x1b[0m");     }
//     println!("  Press Ctrl+C to stop");
//     println!();
// }
