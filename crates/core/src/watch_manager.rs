use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::Mutex;
use std::time::Duration;

use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};

use crate::local_indexer::LocalIndexer;

static WATCHERS: LazyLock<Mutex<HashMap<String, RecommendedWatcher>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
static PENDING: LazyLock<Mutex<HashMap<String, Vec<PathBuf>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Start watching a path for changes and auto-reindex on file events.
pub fn start_watching(path: &Path, repo_id: &str) -> anyhow::Result<()> {
    start_watching_with_invalidator(path, repo_id, |_| {})
}

pub fn start_watching_with_invalidator(
    path: &Path,
    repo_id: &str,
    invalidate: impl Fn(&Path) + Send + Sync + 'static,
) -> anyhow::Result<()> {
    {
        let watchers = WATCHERS.lock().unwrap_or_else(|e| e.into_inner());
        if watchers.contains_key(repo_id) {
            return Ok(());
        }
    }

    PENDING
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(repo_id.to_string(), Vec::new());

    let repo_id_owned = repo_id.to_string();
    let invalidate = Arc::new(invalidate);
    let mut watcher = RecommendedWatcher::new(
        move |res: Result<notify::Event, notify::Error>| match res {
            Ok(event) => {
                for p in event.paths {
                    if crate::watcher::is_ignorable(&p) {
                        continue;
                    }
                    invalidate(&p);
                    if let Ok(mut pending) = PENDING.lock() {
                        if let Some(vec) = pending.get_mut(&repo_id_owned) {
                            vec.push(p);
                        }
                    }
                }
            }
            Err(e) => {
                tracing::warn!("watch error for {}: {}", repo_id_owned, e);
            }
        },
        Config::default().with_poll_interval(Duration::from_secs(1)),
    )?;

    watcher.watch(path, RecursiveMode::Recursive)?;

    WATCHERS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(repo_id.to_string(), watcher);

    let bg_repo_id = repo_id.to_string();
    let bg_path = path.to_path_buf();
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            let drained = {
                let mut pending = PENDING.lock().unwrap_or_else(|e| e.into_inner());
                pending.remove(&bg_repo_id).unwrap_or_default()
            };
            // Re-insert empty vec so we keep the entry alive
            PENDING
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .insert(bg_repo_id.clone(), Vec::new());
            if !drained.is_empty() {
                let path = bg_path.clone();
                let repo_id = bg_repo_id.clone();
                tokio::task::spawn_blocking(move || match LocalIndexer::open(None) {
                    Ok(mut indexer) => {
                        if let Err(e) = indexer.index_repository(&path, &repo_id, false) {
                            tracing::warn!("reindex error for {}: {}", repo_id, e);
                        }
                    }
                    Err(e) => {
                        tracing::warn!("open indexer for reindex {}: {}", repo_id, e);
                    }
                })
                .await
                .unwrap_or_else(|e| {
                    tracing::warn!("reindex task join error for {}: {}", bg_repo_id, e);
                });
            }
        }
    });

    Ok(())
}

/// Stop watching a repo.
pub fn stop_watching(repo_id: &str) -> anyhow::Result<()> {
    WATCHERS
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(repo_id);
    PENDING
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .remove(repo_id);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stop_watching_nonexistent() {
        // Should succeed even if repo_id was never watched
        assert!(stop_watching("nonexistent-repo").is_ok());
    }
}
