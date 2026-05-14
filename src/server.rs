//! Top-level server: binds to a port, starts Axum, optionally watches the
//! database file for changes and reloads it live.

use core::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::net::TcpListener;
use tokio::sync::{RwLock, RwLockReadGuard};

use crate::cli::Args;
use crate::db::Database;
use crate::router::build_router;

pub struct Server;
impl Server {
    pub async fn run(args: &Args) -> anyhow::Result<()> {
        let db = Database::load(&args.db, args.readonly)?;
        let db = Arc::new(RwLock::new(db));

        if args.watch {
            let db = Arc::clone(&db);
            let path = args.db.to_owned();
            tokio::spawn(async move {
                watch_file(db, path).await;
            });
        }

        let router = build_router(Arc::clone(&db), &args);

        let addr = format!("{}:{}", args.host, args.port);
        let tcp = TcpListener::bind(&addr).await?;

        // Print the banner json-server users expect
        print_banner(tcp.local_addr()?, &db.read().await);

        axum::serve(tcp, router).await?;

        Ok(())
    }
}

fn print_banner(addr: SocketAddr, db: &RwLockReadGuard<'_, Database>) {
    println!();
    println!("  \x1b[1;32mjson-server-rs\x1b[0m started on PORT :{}", addr.port());
    println!("  \x1b[36mhttp://{}\x1b[0m", addr);
    println!();

    let resources = db.resource_names();
    if resources.is_empty() {
        println!("  (no resources found)");
    } else {
        println!("  Resources:");
        for name in &resources {
            if db.is_collection(name) {
                println!("  \x1b[33mhttp://{}/{}\x1b[0m", addr, name);
            } else {
                println!("  \x1b[35mhttp://{}/{}\x1b[0m  (singleton)", addr, name);
            }
        }
    }
    println!();

    if db.read_only {
        println!("  \x1b[31mReadonly mode: write operations are disabled\x1b[0m");
    }
    println!("  Press Ctrl+C to stop");
    println!();
}

/// Watch the database file for modifications and reload.
async fn watch_file(db: Arc<RwLock<Database>>, path: PathBuf) {
    use core::time::Duration;
    use std::sync::mpsc;
    use std::time::Instant;

    use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

    let (tx, rx) = mpsc::channel::<notify::Result<Event>>();
    let mut watcher = match RecommendedWatcher::new(tx, Config::default()) {
        Ok(w) => w,
        Err(e) => {
            tracing::error!("File watcher error: {e}");
            return;
        }
    };

    if let Err(e) = watcher.watch(&path, RecursiveMode::NonRecursive) {
        tracing::error!("Could not watch {path:?}: {e}");
        return;
    }

    tracing::info!("Watching {path:?} for changes");

    // Debounce: collect events for 50ms before acting
    let mut last_event = Instant::now();
    loop {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(Ok(event)) => {
                if matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_)) {
                    last_event = Instant::now();
                }
            }

            Ok(Err(e)) => tracing::warn!("Watch error: {e}"),

            Err(mpsc::RecvTimeoutError::Timeout) => {
                // Flush debounce
                if last_event.elapsed() > Duration::from_millis(50)
                    && last_event.elapsed() < Duration::from_millis(500)
                {
                    let mut guard = db.write().await;
                    match guard.reload() {
                        Ok(()) => tracing::info!("Database reloaded"),
                        Err(e) => tracing::warn!("Reload failed: {e}"),
                    }
                }
            }

            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
    }
}
