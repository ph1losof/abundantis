use crate::error::SourceError;
use crate::source::traits::*;
use compact_str::CompactString;
use hashbrown::HashMap;
use parking_lot::RwLock;
use std::sync::Arc;

pub struct SourceRegistry {
    sync_sources: RwLock<HashMap<SourceId, Arc<dyn EnvSource>>>,
    #[cfg(feature = "async")]
    async_sources: RwLock<HashMap<SourceId, Arc<dyn AsyncEnvSource>>>,
    path_index: RwLock<HashMap<std::path::PathBuf, SourceId>>,
    factories: RwLock<HashMap<CompactString, Arc<dyn SourceFactory>>>,
}

impl SourceRegistry {
    pub fn new() -> Self {
        let mut factories: HashMap<CompactString, Arc<dyn SourceFactory>> = HashMap::new();

        #[cfg(feature = "file")]
        factories.insert(
            CompactString::new("file"),
            Arc::new(FileSourceFactory) as Arc<dyn SourceFactory>,
        );
        #[cfg(feature = "shell")]
        factories.insert(
            CompactString::new("shell"),
            Arc::new(ShellSourceFactory) as Arc<dyn SourceFactory>,
        );
        factories.insert(
            CompactString::new("memory"),
            Arc::new(MemorySourceFactory) as Arc<dyn SourceFactory>,
        );

        Self {
            sync_sources: RwLock::new(HashMap::new()),
            #[cfg(feature = "async")]
            async_sources: RwLock::new(HashMap::new()),
            path_index: RwLock::new(HashMap::new()),
            factories: RwLock::new(factories),
        }
    }

    pub fn register_factory<F: SourceFactory + 'static>(&self, source_type: &str, factory: F) {
        self.factories
            .write()
            .insert(CompactString::new(source_type), Arc::new(factory));
    }

    pub fn register_sync(&self, source: Arc<dyn EnvSource>) -> SourceId {
        let id = source.id().clone();
        self.sync_sources.write().insert(id.clone(), source.clone());

        if source.source_type() == SourceType::File {
            if let Some(path) = id.as_str().strip_prefix("file:") {
                let path_buf = std::path::PathBuf::from(path);
                self.path_index.write().insert(path_buf, id.clone());
            }
        }

        id
    }

    #[cfg(feature = "async")]
    pub fn register_async(&self, source: Arc<dyn AsyncEnvSource>) -> SourceId {
        let id = source.id().clone();
        self.async_sources.write().insert(id.clone(), source);
        id
    }

    pub fn sync_sources_by_priority(&self) -> Vec<Arc<dyn EnvSource>> {
        let sources = self.sync_sources.read();
        let mut sorted: Vec<_> = sources.values().cloned().collect();
        sorted.sort_by_key(|a| std::cmp::Reverse(a.priority()));
        sorted
    }

    #[cfg(feature = "async")]
    pub fn async_sources(&self) -> Vec<Arc<dyn AsyncEnvSource>> {
        self.async_sources.read().values().cloned().collect()
    }

    #[cfg(feature = "async")]
    pub fn has_async_sources(&self) -> bool {
        !self.async_sources.read().is_empty()
    }

    #[cfg(feature = "async")]
    pub async fn load_all(&self) -> Result<Vec<SourceSnapshot>, SourceError> {
        let snapshots = {
            let mut snapshots = Vec::new();
            let sources_guard = self.sync_sources.read();
            for (_id, source) in sources_guard.iter() {
                let snapshot = source.load()?;
                snapshots.push(snapshot);
            }
            snapshots
        };
        let mut snapshots = snapshots;

        if self.has_async_sources() {
            let async_sources = self.async_sources.read().clone();
            let futures: Vec<_> = async_sources.values().map(|s| s.load()).collect();

            let results = futures::future::join_all(futures).await;
            for result in results {
                snapshots.push(result?);
            }
        }

        Ok(snapshots)
    }

    #[cfg(feature = "async")]
    pub async fn refresh_async(&self) -> Result<(), SourceError> {
        let async_sources = self.async_sources.read().clone();
        let futures: Vec<_> = async_sources.values().map(|s| s.refresh()).collect();

        let results = futures::future::try_join_all(futures).await?;
        for result in results {
            if result {
                tracing::info!("Async source refreshed");
            }
        }

        Ok(())
    }

    pub fn sources_of_type(&self, source_type: SourceType) -> Vec<Arc<dyn EnvSource>> {
        let sources = self.sync_sources.read();
        sources
            .values()
            .filter(|s| s.source_type() == source_type)
            .cloned()
            .collect()
    }

    pub fn sources_for_paths(&self, paths: &[std::path::PathBuf]) -> Vec<Arc<dyn EnvSource>> {
        let sources = self.sync_sources.read();
        let path_index = self.path_index.read();
        let mut result = Vec::new();
        let mut seen_ids = std::collections::HashSet::new();

        for path in paths {
            if let Some(source_id) = path_index.get(path) {
                if !seen_ids.contains(source_id) {
                    if let Some(source) = sources.get(source_id) {
                        result.push(Arc::clone(source));
                        seen_ids.insert(source_id.clone());
                    }
                }
            }
        }

        result
    }

    pub fn invalidate_file(&self, _path: &std::path::Path) {
        for source in self.sync_sources.read().values() {
            if source.source_type() == SourceType::File {
                source.invalidate();
            }
        }
    }

    pub fn is_registered(&self, id: &SourceId) -> bool {
        self.sync_sources.read().contains_key(id)
    }

    pub fn unregister_sync(&self, id: &SourceId) {
        self.sync_sources.write().remove(id);

        if let Some(path) = id.as_str().strip_prefix("file:") {
            let path_buf = std::path::PathBuf::from(path);
            self.path_index.write().remove(&path_buf);
        }
    }

    pub fn registered_file_paths(&self) -> Vec<std::path::PathBuf> {
        self.path_index.read().keys().cloned().collect()
    }

    pub fn source_count(&self) -> usize {
        let count = self.sync_sources.read().len();
        #[cfg(feature = "async")]
        let count = count + self.async_sources.read().len();
        count
    }
}

impl Default for SourceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(not(feature = "async"))]
impl SourceRegistry {
    pub fn has_async_sources(&self) -> bool {
        false
    }

    pub fn load_all(&self) -> Result<Vec<SourceSnapshot>, SourceError> {
        let mut snapshots = Vec::new();
        for source in self.sync_sources.read().values() {
            snapshots.push(source.load()?);
        }
        Ok(snapshots)
    }
}

pub trait SourceFactory: Send + Sync {
    fn create(&self, config: &SourceConfig) -> Result<Arc<dyn EnvSource>, SourceError>;
    fn source_type(&self) -> &'static str;
}

#[derive(Debug, Clone)]
pub struct SourceConfig {
    pub source_type: String,
    pub path: Option<std::path::PathBuf>,
    pub enabled: bool,
}

struct FileSourceFactory;
impl SourceFactory for FileSourceFactory {
    fn create(&self, config: &SourceConfig) -> Result<Arc<dyn EnvSource>, SourceError> {
        if let Some(path) = &config.path {
            crate::source::file::FileSource::new(path)
                .map(|s| Arc::new(s) as Arc<dyn EnvSource>)
                .map_err(|e| SourceError::SourceRead {
                    source_name: path.display().to_string(),
                    reason: e.to_string(),
                })
        } else {
            Err(SourceError::SourceRead {
                source_name: "file".into(),
                reason: "No path specified".into(),
            })
        }
    }

    fn source_type(&self) -> &'static str {
        "file"
    }
}

struct ShellSourceFactory;
impl SourceFactory for ShellSourceFactory {
    fn create(&self, _config: &SourceConfig) -> Result<Arc<dyn EnvSource>, SourceError> {
        Ok(Arc::new(crate::source::shell::ShellSource::new()) as Arc<dyn EnvSource>)
    }

    fn source_type(&self) -> &'static str {
        "shell"
    }
}

struct MemorySourceFactory;
impl SourceFactory for MemorySourceFactory {
    fn create(&self, _config: &SourceConfig) -> Result<Arc<dyn EnvSource>, SourceError> {
        Ok(Arc::new(crate::source::memory::MemorySource::new()) as Arc<dyn EnvSource>)
    }

    fn source_type(&self) -> &'static str {
        "memory"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_basics() {
        let registry = SourceRegistry::new();
        assert_eq!(registry.source_count(), 0);
    }
}
