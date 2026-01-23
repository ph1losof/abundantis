#[cfg(all(feature = "watch", feature = "async"))]
use crate::events::AbundantisEvent;

#[cfg(all(feature = "watch", feature = "async"))]
use crate::source::{EnvSource, FileSource};

#[cfg(all(feature = "watch", feature = "async"))]
use compact_str::CompactString;

#[cfg(all(feature = "watch", feature = "async"))]
use parking_lot::Mutex;

#[cfg(all(feature = "watch", feature = "async"))]
use std::path::{Path, PathBuf};

#[cfg(all(feature = "watch", feature = "async"))]
use std::sync::Arc;

#[cfg(all(feature = "watch", feature = "async"))]
use crate::watch::{ChangeKind, FileChanged, FileWatcher};

#[cfg(all(feature = "watch", feature = "async"))]
pub struct WatchManager {
    watcher: Arc<FileWatcher>,
    file_sources: Arc<Mutex<std::collections::HashMap<PathBuf, Arc<FileSource>>>>,
    event_bus: Arc<crate::events::EventBus>,
}

#[cfg(all(feature = "watch", feature = "async"))]
impl WatchManager {
    pub fn new(event_bus: Arc<crate::events::EventBus>) -> Result<Self, notify::Error> {
        let watcher = Arc::new(FileWatcher::new()?);

        Ok(Self {
            watcher,
            file_sources: Arc::new(Mutex::new(std::collections::HashMap::new())),
            event_bus,
        })
    }

    pub fn watch_file(&self, source: Arc<FileSource>) {
        let path = source.get_path().to_path_buf();
        let source_id = source.as_ref().id().as_str();

        self.watcher.watch(&path, source_id);
        self.file_sources.lock().insert(path, source);
    }

    pub fn unwatch_file(&self, path: impl AsRef<Path>) {
        let path_buf = path.as_ref().to_path_buf();
        self.watcher.unwatch(&path_buf);
        self.file_sources.lock().remove(&path_buf);
    }

    pub fn start(&self) {
        let sources = Arc::clone(&self.file_sources);
        let event_bus = Arc::clone(&self.event_bus);

        self.watcher
            .register_callback(Arc::new(move |change: FileChanged| {
                let path = &change.path;

                let source_opt = {
                    let sources = sources.lock();
                    sources.get(path).cloned()
                };

                if let Some(source) = source_opt {
                    match change.kind {
                        ChangeKind::Created => {
                            tracing::debug!("File created: {:?}", path);
                            if let Err(e) = Self::handle_file_create(&source, &event_bus) {
                                tracing::error!(
                                    "Failed to handle file create for {:?}: {}",
                                    path,
                                    e
                                );
                            }
                        }
                        ChangeKind::Modified => {
                            tracing::debug!("File modified: {:?}", path);
                            if let Err(e) = Self::handle_file_change(&source, &event_bus) {
                                tracing::error!(
                                    "Failed to handle file change for {:?}: {}",
                                    path,
                                    e
                                );
                            }
                        }
                        ChangeKind::Deleted => {
                            tracing::debug!("File deleted: {:?}", path);
                            if let Err(e) = Self::handle_file_delete(&source, &event_bus) {
                                tracing::error!(
                                    "Failed to handle file delete for {:?}: {}",
                                    path,
                                    e
                                );
                            }
                        }
                    }
                }
            }));
    }

    fn handle_file_change(
        source: &Arc<FileSource>,
        event_bus: &Arc<crate::events::EventBus>,
    ) -> Result<(), String> {
        let before_snapshot = source
            .as_ref()
            .load()
            .map_err(|e| format!("Failed to load before reload: {}", e))?;

        source
            .as_ref()
            .reload()
            .map_err(|e| format!("Failed to reload file: {}", e))?;

        let after_snapshot = source
            .as_ref()
            .load()
            .map_err(|e| format!("Failed to load after reload: {}", e))?;

        let before_vars: std::collections::HashSet<CompactString> = before_snapshot
            .variables
            .iter()
            .map(|v| v.key.clone())
            .collect();

        let after_vars: std::collections::HashSet<CompactString> = after_snapshot
            .variables
            .iter()
            .map(|v| v.key.clone())
            .collect();

        let added: Vec<CompactString> = after_vars.difference(&before_vars).cloned().collect();

        let removed: Vec<CompactString> = before_vars.difference(&after_vars).cloned().collect();

        event_bus.publish(AbundantisEvent::VariablesChanged {
            source_id: source.as_ref().id().clone(),
            added,
            removed,
        });

        event_bus.publish(AbundantisEvent::CacheInvalidated { scope: None });

        Ok(())
    }

    fn handle_file_create(
        source: &Arc<FileSource>,
        event_bus: &Arc<crate::events::EventBus>,
    ) -> Result<(), String> {
        let snapshot = source
            .as_ref()
            .load()
            .map_err(|e| format!("Failed to load created file: {}", e))?;

        let vars: Vec<CompactString> = snapshot.variables.iter().map(|v| v.key.clone()).collect();

        event_bus.publish(AbundantisEvent::VariablesChanged {
            source_id: source.as_ref().id().clone(),
            added: vars,
            removed: Vec::new(),
        });

        event_bus.publish(AbundantisEvent::CacheInvalidated { scope: None });

        Ok(())
    }

    fn handle_file_delete(
        source: &Arc<FileSource>,
        event_bus: &Arc<crate::events::EventBus>,
    ) -> Result<(), String> {
        let snapshot = source
            .as_ref()
            .load()
            .map_err(|e| format!("Failed to load deleted file (cached): {}", e))?;

        let vars: Vec<CompactString> = snapshot.variables.iter().map(|v| v.key.clone()).collect();

        event_bus.publish(AbundantisEvent::VariablesChanged {
            source_id: source.as_ref().id().clone(),
            added: Vec::new(),
            removed: vars,
        });

        event_bus.publish(AbundantisEvent::CacheInvalidated { scope: None });

        Ok(())
    }

    pub fn watched_files(&self) -> Vec<PathBuf> {
        self.watcher.paths()
    }

    pub fn is_watching(&self, path: impl AsRef<Path>) -> bool {
        self.watcher.is_watching(path)
    }
}

#[cfg(all(test, feature = "watch", feature = "async"))]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[tokio::test]
    async fn test_watch_manager() {
        let event_bus = Arc::new(crate::events::EventBus::new(100));
        let manager = WatchManager::new(event_bus.clone()).unwrap();

        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "KEY=value1").unwrap();

        let source = Arc::new(FileSource::new(file.path()).unwrap());
        manager.watch_file(source.clone());

        assert!(manager.is_watching(file.path()));

        manager.unwatch_file(file.path());
        assert!(!manager.is_watching(file.path()));
    }
}
