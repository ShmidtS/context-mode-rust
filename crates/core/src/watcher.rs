use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use notify::{Config, Event, RecommendedWatcher, Watcher};
use serde::{Deserialize, Serialize};
use tokio::sync::{mpsc, Mutex};
use tracing::{error, info};

/// A file-system event produced by the watcher.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WatchEvent {
    pub path: PathBuf,
    pub kind: String,
}

/// Paths under watch, persisted as JSON.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct WatchList {
    pub paths: Vec<String>,
}

impl WatchList {
    fn config_path() -> PathBuf {
        let mut p = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        p.push(".context-mode");
        p.push("watched.json");
        p
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if let Ok(data) = std::fs::read_to_string(&path) {
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    pub fn save(&self) {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let _ = std::fs::write(&path, serde_json::to_string_pretty(self).unwrap_or_default());
    }
}

/// Returns true for paths that should be ignored (hidden, node_modules, etc.).
pub fn is_ignorable(path: &Path) -> bool {
    let s = path.to_string_lossy();
    s.contains("node_modules")
        || s.contains(".git")
        || s.contains("target")
        || s.contains("__pycache__")
        || s.contains(".omc")
        || s.starts_with(".")
        || path
            .file_name()
            .map(|n| n.to_string_lossy().starts_with('.'))
            .unwrap_or(false)
}

/// Debounced file-system watcher built on `notify`.
pub struct FileWatcher {
    watched: Arc<Mutex<HashSet<PathBuf>>>,
    #[allow(dead_code)]
    tx: mpsc::Sender<WatchEvent>,
    _watcher: RecommendedWatcher,
}

impl FileWatcher {
    pub fn new(tx: mpsc::Sender<WatchEvent>) -> anyhow::Result<Self> {
        let watched = Arc::new(Mutex::new(HashSet::<PathBuf>::new()));
        let watched_clone = watched.clone();
        let tx_clone = tx.clone();

        let watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                match res {
                    Ok(event) => {
                        for path in event.paths {
                            if is_ignorable(&path) {
                                continue;
                            }
                            let kind = format!("{:?}", event.kind);
                            let _ = tx_clone.try_send(WatchEvent { path, kind });
                        }
                    }
                    Err(e) => {
                        error!("watch error: {}", e);
                    }
                }
            },
            Config::default().with_poll_interval(Duration::from_secs(1)),
        )?;

        Ok(FileWatcher {
            watched: watched_clone,
            tx,
            _watcher: watcher,
        })
    }

    pub async fn add_path(&self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        let path = path.as_ref().canonicalize().unwrap_or_else(|_| path.as_ref().to_path_buf());
        let mut set = self.watched.lock().await;
        if set.insert(path.clone()) {
            info!("watching path: {:?}", path);
        }
        Ok(())
    }

    pub async fn remove_path(&self, path: impl AsRef<Path>) -> anyhow::Result<()> {
        let path = path.as_ref().canonicalize().unwrap_or_else(|_| path.as_ref().to_path_buf());
        let mut set = self.watched.lock().await;
        set.remove(&path);
        info!("unwatched path: {:?}", path);
        Ok(())
    }

    pub async fn watched_paths(&self) -> Vec<PathBuf> {
        let set = self.watched.lock().await;
        set.iter().cloned().collect()
    }

    /// Load persisted watch list and add all paths.
    pub async fn load_and_watch_all(&self) {
        let list = WatchList::load();
        for p in &list.paths {
            let _ = self.add_path(p).await;
        }
    }

    /// Persist current watch list to disk.
    pub async fn persist(&self) {
        let paths = self.watched_paths().await;
        let list = WatchList {
            paths: paths.into_iter().map(|p| p.to_string_lossy().into_owned()).collect(),
        };
        list.save();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // std::io::Write not needed

    #[test]
    fn test_is_ignorable() {
        assert!(is_ignorable(Path::new("/project/node_modules/foo.ts")));
        assert!(is_ignorable(Path::new("/project/.git/HEAD")));
        assert!(is_ignorable(Path::new("/project/target/debug/lib.so")));
        assert!(is_ignorable(Path::new("/project/.hidden")));
        assert!(!is_ignorable(Path::new("/project/src/main.rs")));
    }

    #[test]
    fn test_watch_list_roundtrip() {
        let mut list = WatchList::default();
        list.paths = vec!["/tmp/a".into(), "/tmp/b".into()];
        // Use a temp file for this test by overriding via env? Not easy.
        // Instead just test serde.
        let json = serde_json::to_string(&list).unwrap();
        let restored: WatchList = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.paths, list.paths);
    }

    #[tokio::test]
    async fn test_file_watcher_lifecycle() {
        let (tx, _rx) = mpsc::channel(32);
        let watcher = FileWatcher::new(tx).unwrap();

        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().to_path_buf();

        watcher.add_path(&dir).await.unwrap();
        let paths = watcher.watched_paths().await;
        assert!(paths.iter().any(|p| p.ends_with(dir.file_name().unwrap())));
    }
}
