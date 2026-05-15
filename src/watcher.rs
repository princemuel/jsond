use std::path::Path;

use crate::db::Database;

pub(crate) fn spawn(path: &Path, db: &Database) {
    let path = path.to_path_buf();
    let db = db.clone();
    tokio::spawn(async move {
        if let Err(e) = run(&path, &db).await {
            tracing::warn!("file watcher stopped: {e}");
        }
    });
}

pub(crate) async fn run(path: &Path, db: &Database) -> anyhow::Result<()> {
    use notify::{Config, EventKind, RecommendedWatcher, RecursiveMode, Watcher as _};
    use tokio::sync::mpsc;
    use tokio::time::{Duration, sleep};

    let (tx, mut rx) = mpsc::channel(32);

    let mut watcher = RecommendedWatcher::new(
        move |res| {
            tx.blocking_send(res).ok();
        },
        Config::default(),
    )?;

    watcher.watch(path, RecursiveMode::NonRecursive)?;
    tracing::info!("watching {path:?} for changes");

    // Debounce: collect events for 50ms before acting
    while let Some(event) = rx.recv().await {
        match event {
            Ok(e) if matches!(e.kind, EventKind::Create(_) | EventKind::Modify(_)) => {
                sleep(Duration::from_millis(50)).await;

                if let Err(e) = db.reload().await {
                    tracing::warn!("Reload failed {e}");
                } else {
                    tracing::info!("Database reloaded");
                }
            }
            Err(e) => tracing::warn!("Watcher error: {e}"),
            _ => (),
        }
    }

    Ok(())
}
