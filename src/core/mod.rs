use compact_str::CompactString;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Default)]
pub struct AbundantisBuilder {
    config: super::AbundantisConfig,
    custom_sources: Vec<Arc<dyn super::source::EnvSource>>,
    subscribers: Vec<Arc<dyn super::events::EventSubscriber>>,
    root: Option<PathBuf>,
    _event_buffer_size: Option<usize>,
    active_files: Option<Vec<String>>,
    active_files_for_directory: HashMap<PathBuf, Vec<String>>,
}

impl AbundantisBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn root(mut self, root: impl AsRef<Path>) -> Self {
        self.root = Some(root.as_ref().to_path_buf());
        self
    }

    pub fn provider(mut self, provider: super::config::MonorepoProviderType) -> Self {
        self.config.workspace.provider = Some(provider);
        self
    }

    pub fn roots(mut self, roots: Vec<impl Into<CompactString>>) -> Self {
        self.config.workspace.roots = roots.into_iter().map(|r| r.into()).collect();
        self
    }

    pub fn cascading(mut self, enabled: bool) -> Self {
        self.config.workspace.cascading = enabled;
        self
    }

    pub fn env_files(mut self, patterns: Vec<impl Into<CompactString>>) -> Self {
        self.config.workspace.env_files = patterns.into_iter().map(|p| p.into()).collect();
        self
    }

    pub fn ignores(mut self, patterns: Vec<impl Into<CompactString>>) -> Self {
        self.config.workspace.ignores = patterns.into_iter().map(|p| p.into()).collect();
        self
    }

    pub fn with_shell(mut self) -> Self {
        #[cfg(feature = "shell")]
        {
            if !self
                .config
                .resolution
                .precedence
                .contains(&super::config::SourcePrecedence::Shell)
            {
                self.config
                    .resolution
                    .precedence
                    .insert(0, super::config::SourcePrecedence::Shell);
            }
        }
        self
    }

    pub fn precedence(mut self, precedence: Vec<super::config::SourcePrecedence>) -> Self {
        self.config.resolution.precedence = precedence;
        self
    }

    pub fn interpolation(mut self, enabled: bool) -> Self {
        self.config.interpolation.enabled = enabled;
        self
    }

    pub fn max_interpolation_depth(mut self, depth: u32) -> Self {
        self.config.interpolation.max_depth = depth;
        self
    }

    pub fn interpolation_features(
        mut self,
        features: super::config::InterpolationFeatures,
    ) -> Self {
        self.config.interpolation.features = features;
        self
    }

    pub fn cache_enabled(mut self, enabled: bool) -> Self {
        self.config.cache.enabled = enabled;
        self
    }

    pub fn cache_size(mut self, size: usize) -> Self {
        self.config.cache.hot_cache_size = size;
        self
    }

    pub fn cache_ttl(mut self, ttl: std::time::Duration) -> Self {
        self.config.cache.ttl = ttl;
        self
    }

    pub fn source_defaults(mut self, defaults: super::config::SourceDefaults) -> Self {
        self.config.sources.defaults = defaults;
        self
    }

    pub fn with_source(mut self, source: Arc<dyn super::source::EnvSource>) -> Self {
        self.custom_sources.push(source);
        self
    }

    pub fn subscribe(mut self, subscriber: Arc<dyn super::events::EventSubscriber>) -> Self {
        self.subscribers.push(subscriber);
        self
    }

    pub fn event_buffer_size(mut self, size: usize) -> Self {
        self._event_buffer_size = Some(size);
        self
    }

    pub fn active_files(mut self, patterns: Vec<impl AsRef<str>>) -> Self {
        self.active_files = Some(patterns.iter().map(|p| p.as_ref().to_string()).collect());
        self
    }

    pub fn active_files_for_directory(
        mut self,
        directory: impl AsRef<Path>,
        patterns: Vec<impl AsRef<str>>,
    ) -> Self {
        let dir_path = directory.as_ref().to_path_buf();
        let canonical_dir = match dir_path.canonicalize() {
            Ok(c) => c,
            Err(_) => dir_path,
        };
        self.active_files_for_directory.insert(
            canonical_dir,
            patterns.iter().map(|p| p.as_ref().to_string()).collect(),
        );
        self
    }

    #[cfg(feature = "async")]
    pub async fn build(self) -> Result<super::Abundantis, super::AbundantisError> {
        let mut config = self.config.clone();

        let root = if let Some(ref r) = self.root {
            r.canonicalize().map_err(super::AbundantisError::Io)?
        } else {
            std::env::current_dir()
                .map_err(super::AbundantisError::Io)?
                .canonicalize()
                .map_err(super::AbundantisError::Io)?
        };

        if config.workspace.provider.is_none() {
            if let Some(detected) = super::workspace::provider::ProviderRegistry::detect(&root) {
                tracing::info!("Auto-detected workspace provider: {:?}", detected);
                config.workspace.provider = Some(detected);
            } else {
                tracing::info!("No workspace provider detected, defaulting to simple project");
                config.workspace.provider = Some(super::config::MonorepoProviderType::Custom);

                if config.workspace.roots.is_empty() {
                    config.workspace.roots.push(".".into());
                }
            }
        }

        let workspace =
            super::workspace::WorkspaceManager::with_root(root.clone(), &config.workspace)?;

        let registry = Arc::new(super::source::SourceRegistry::new());

        for source in &self.custom_sources {
            registry.register_sync(Arc::clone(source));
        }

        let event_bus = Arc::new(super::events::EventBus::new(
            self._event_buffer_size.unwrap_or(256),
        ));

        for subscriber in &self.subscribers {
            event_bus.subscribe(Arc::clone(subscriber));
        }

        #[cfg(all(feature = "watch", feature = "async"))]
        let watch_manager: Arc<Option<super::watch_manager::WatchManager>> = Arc::new(
            match super::watch_manager::WatchManager::new(Arc::clone(&event_bus)) {
                Ok(m) => Some(m),
                Err(e) => {
                    return Err(super::AbundantisError::Runtime(format!(
                        "Failed to initialize file watcher: {}",
                        e
                    )))
                }
            },
        );

        #[cfg(feature = "file")]
        if config.sources.defaults.file {
            let file_sources = self.discover_file_sources(&workspace, &config)?;
            for source in file_sources {
                #[cfg(all(feature = "watch", feature = "async"))]
                if let Some(ref manager) = &*watch_manager {
                    manager.watch_file(Arc::clone(&source));
                }
                registry.register_sync(source as Arc<dyn super::source::EnvSource>);
            }
        }

        #[cfg(feature = "shell")]
        if config.sources.defaults.shell {
            let shell_source =
                Arc::new(super::source::ShellSource::new()) as Arc<dyn super::source::EnvSource>;
            registry.register_sync(shell_source);
        }

        let resolution_engine = Arc::new(super::resolution::ResolutionEngine::new(
            &config.resolution,
            &config.interpolation,
            &config.cache,
        ));

        let cache = Arc::clone(resolution_engine.cache());

        let path_cache = super::path_cache::PathCache::new();

        let selector = Arc::new(super::selection::ActiveFileSelector::new(
            &root,
            Arc::new(path_cache.clone()),
        ));

        #[cfg(all(feature = "watch", feature = "async"))]
        if let Some(ref manager) = &*watch_manager {
            manager.start();
        }

        Ok(super::Abundantis {
            config,
            registry,
            resolution: resolution_engine,
            workspace: Arc::new(parking_lot::RwLock::new(workspace)),
            cache,
            selector,
            global_active_files: parking_lot::RwLock::new(self.active_files),
            directory_active_files: parking_lot::RwLock::new(self.active_files_for_directory),
            path_to_source_id: parking_lot::RwLock::new(HashMap::new()),
            path_cache,
            event_bus,
        })
    }

    #[cfg(feature = "async")]
    #[cfg(feature = "file")]
    fn discover_file_sources(
        &self,
        workspace: &super::workspace::WorkspaceManager,
        config: &super::AbundantisConfig,
    ) -> Result<Vec<Arc<super::source::FileSource>>, super::AbundantisError> {
        let mut sources = Vec::new();

        for package in workspace.packages() {
            for pattern in &config.workspace.env_files {
                let full_pattern = package.root.join(pattern.as_str());
                let pattern_str = full_pattern.to_string_lossy();

                match glob::glob(&pattern_str) {
                    Ok(paths) => {
                        for entry in paths {
                            match entry {
                                Ok(path) => {
                                    if path.is_file() {
                                        match super::source::FileSource::new(&path) {
                                            Ok(file_source) => {
                                                let arc_source = Arc::new(file_source);
                                                sources.push(arc_source);
                                            }
                                            Err(e) => {
                                                tracing::warn!(
                                                    "Failed to load env file {}: {}",
                                                    path.display(),
                                                    e
                                                );
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    tracing::warn!("Glob error for pattern {}: {}", pattern_str, e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to compile glob pattern {}: {}", pattern_str, e);
                    }
                }
            }
        }

        Ok(sources)
    }

    #[cfg(not(feature = "async"))]
    pub fn build(self) -> Result<super::Abundantis, super::AbundantisError> {
        let config = self.config.clone();

        if config.workspace.provider.is_none() {
            return Err(super::AbundantisError::MissingConfig {
                field: "workspace.provider",
                suggestion: "Set to one of: turbo, nx, lerna, pnpm, npm, cargo, custom".into(),
            });
        }

        let root = if let Some(ref r) = self.root {
            r.canonicalize().map_err(super::AbundantisError::Io)?
        } else {
            std::env::current_dir()
                .map_err(super::AbundantisError::Io)?
                .canonicalize()
                .map_err(super::AbundantisError::Io)?
        };

        let workspace =
            super::workspace::WorkspaceManager::with_root(root.clone(), &config.workspace)?;

        let registry = Arc::new(super::source::SourceRegistry::new());

        for source in &self.custom_sources {
            registry.register_sync(Arc::clone(source));
        }

        #[cfg(feature = "file")]
        if config.sources.defaults.file {
            let file_sources = self.discover_file_sources(&workspace, &config)?;
            for source in file_sources {
                registry.register_sync(source as Arc<dyn super::source::EnvSource>);
            }
        }

        #[cfg(feature = "shell")]
        if config.sources.defaults.shell {
            let shell_source =
                Arc::new(super::source::ShellSource::new()) as Arc<dyn super::source::EnvSource>;
            registry.register_sync(shell_source);
        }

        let resolution_engine = Arc::new(super::resolution::ResolutionEngine::new(
            &config.resolution,
            &config.interpolation,
            &config.cache,
        ));

        let cache = Arc::clone(resolution_engine.cache());

        let path_cache = super::path_cache::PathCache::new();

        let selector = Arc::new(super::selection::ActiveFileSelector::new(
            &root,
            Arc::new(path_cache.clone()),
        ));

        let event_bus = Arc::new(super::events::EventBus::new(
            self._event_buffer_size.unwrap_or(256),
        ));

        for subscriber in &self.subscribers {
            event_bus.subscribe(Arc::clone(subscriber));
        }

        Ok(super::Abundantis {
            config,
            registry,
            resolution: resolution_engine,
            workspace: Arc::new(parking_lot::RwLock::new(workspace)),
            cache,
            selector,
            global_active_files: parking_lot::RwLock::new(self.active_files),
            directory_active_files: parking_lot::RwLock::new(self.active_files_for_directory),
            path_to_source_id: parking_lot::RwLock::new(HashMap::new()),
            path_cache,
            event_bus,
        })
    }

    #[cfg(not(feature = "async"))]
    #[cfg(feature = "file")]
    fn discover_file_sources(
        &self,
        workspace: &super::workspace::WorkspaceManager,
        config: &super::AbundantisConfig,
    ) -> Result<Vec<Arc<super::source::FileSource>>, super::AbundantisError> {
        let mut sources = Vec::new();

        for package in workspace.packages() {
            for pattern in &config.workspace.env_files {
                let full_pattern = package.root.join(pattern.as_str());
                let pattern_str = full_pattern.to_string_lossy();

                match glob::glob(&pattern_str) {
                    Ok(paths) => {
                        for entry in paths {
                            match entry {
                                Ok(path) => {
                                    if path.is_file() {
                                        match super::source::FileSource::new(&path) {
                                            Ok(file_source) => {
                                                let arc_source = Arc::new(file_source);
                                                sources.push(arc_source);
                                            }
                                            Err(e) => {
                                                tracing::warn!(
                                                    "Failed to load env file {}: {}",
                                                    path.display(),
                                                    e
                                                );
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    tracing::warn!("Glob error for pattern {}: {}", pattern_str, e);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Failed to compile glob pattern {}: {}", pattern_str, e);
                    }
                }
            }
        }

        Ok(sources)
    }
}
