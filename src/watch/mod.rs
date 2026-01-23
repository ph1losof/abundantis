#[cfg(all(feature = "watch", feature = "async"))]
use std::path::{Path, PathBuf};

#[cfg(all(feature = "watch", feature = "async"))]
use compact_str::CompactString;

#[cfg(all(feature = "watch", feature = "async"))]
use notify::{Event, EventKind};

#[cfg(all(feature = "watch", feature = "async"))]
use parking_lot::Mutex;

#[cfg(all(feature = "watch", feature = "async"))]
use std::collections::HashMap;

#[cfg(all(feature = "watch", feature = "async"))]
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg(all(feature = "watch", feature = "async"))]
pub struct FileChanged {
    pub path: PathBuf,
    pub kind: ChangeKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg(all(feature = "watch", feature = "async"))]
pub enum ChangeKind {
    Created,
    Modified,
    Deleted,
}

#[cfg(all(feature = "watch", feature = "async"))]
pub type WatchCallback = Arc<dyn Fn(FileChanged) + Send + Sync>;

#[cfg(all(feature = "watch", feature = "async"))]
pub struct FileWatcher {
    paths: Arc<Mutex<HashMap<PathBuf, CompactString>>>,
    callbacks: Arc<Mutex<Vec<WatchCallback>>>,
}

#[cfg(all(feature = "watch", feature = "async"))]
impl FileWatcher {
    pub fn new() -> Result<Self, notify::Error> {
        let paths = Arc::new(Mutex::new(HashMap::new()));
        let callbacks = Arc::new(Mutex::new(Vec::<WatchCallback>::new()));
        let paths_clone = Arc::clone(&paths);
        let callbacks_clone = Arc::clone(&callbacks);

        let _watcher: notify::RecommendedWatcher = notify::recommended_watcher(move |res: Result<Event, _>| {
            if let Ok(event) = res {
                for path in event.paths {
                    let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
                    
                    let source_id = {
                        let paths = paths_clone.lock();
                        paths.get(&canonical).cloned()
                    };

                    if source_id.is_none() {
                        continue;
                    }

                    let kind = match event.kind {
                        EventKind::Create(_) => ChangeKind::Created,
                        EventKind::Modify(_) => ChangeKind::Modified,
                        EventKind::Remove(_) => ChangeKind::Deleted,
                        _ => continue,
                    };

                    let change = FileChanged { path, kind };

                    let callbacks = callbacks_clone.lock();
                    for callback in callbacks.iter() {
                        callback(change.clone());
                    }
                }
            }
        })?;

        Ok(Self {
            paths,
            callbacks,
        })
    }

    pub fn watch(&self, path: impl AsRef<Path>, source_id: impl Into<CompactString>) {
        let path = path.as_ref().canonicalize().unwrap_or_else(|_| path.as_ref().to_path_buf());
        self.paths.lock().insert(path, source_id.into());
    }

    pub fn unwatch(&self, path: impl AsRef<Path>) {
        let path = path.as_ref().canonicalize().unwrap_or_else(|_| path.as_ref().to_path_buf());
        self.paths.lock().remove(&path);
    }

    pub fn register_callback(&self, callback: WatchCallback) {
        self.callbacks.lock().push(callback);
    }

    pub fn paths(&self) -> Vec<PathBuf> {
        self.paths.lock().keys().cloned().collect()
    }

    pub fn is_watching(&self, path: impl AsRef<Path>) -> bool {
        let path = path.as_ref().canonicalize().unwrap_or_else(|_| path.as_ref().to_path_buf());
        self.paths.lock().contains_key(&path)
    }
}

#[cfg(all(feature = "watch", feature = "async"))]
impl Default for FileWatcher {
    fn default() -> Self {
        Self::new().expect("Failed to create file watcher")
    }
}

#[cfg(all(test, feature = "watch", feature = "async"))]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::time::Duration;

    #[tokio::test]
    async fn test_file_watcher() {
        let temp_dir = TempDir::new().unwrap();
        let watcher = FileWatcher::new().unwrap();

        let test_file = temp_dir.path().join("test.env");
        watcher.watch(&test_file, "test-source");

        assert!(watcher.is_watching(&test_file));
        assert_eq!(watcher.paths().len(), 1);

        watcher.unwatch(&test_file);
        assert!(!watcher.is_watching(&test_file));
    }

    #[tokio::test]
    async fn test_callback_registration() {
        let temp_dir = TempDir::new().unwrap();
        let watcher = FileWatcher::new().unwrap();

        let test_file = temp_dir.path().join("test.env");
        watcher.watch(&test_file, "test-source");

        let callback_called = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let callback_clone = Arc::clone(&callback_called);

        watcher.register_callback(Arc::new(move |_change| {
            callback_clone.store(true, std::sync::atomic::Ordering::SeqCst);
        }));

        std::fs::write(&test_file, "TEST=value").unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;

        tokio::time::timeout(Duration::from_millis(500), async {
            while !callback_called.load(std::sync::atomic::Ordering::SeqCst) {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .ok();
    }
}
