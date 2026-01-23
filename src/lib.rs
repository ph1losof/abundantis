//! # Abundantis
//!
//! High-performance unified environment variable management from multiple sources.
//!
//! ## Quick Start
//!
#![cfg_attr(
    feature = "async",
    doc = r#"
```no_run
use abundantis::{Abundantis, config::MonorepoProviderType};

# #[tokio::main]
# async fn example() -> abundantis::Result<()> {
let _abundantis = Abundantis::builder()
    .root(".")
    .provider(MonorepoProviderType::Custom)
    .with_shell()
    .env_files(vec![".env", ".env.local"])
    .build()
    .await?;

# Ok(())
# }
```
"#
)]
#![cfg_attr(
    not(feature = "async"),
    doc = r#"
```no_run
use abundantis::{Abundantis, config::MonorepoProviderType};

# fn example() -> abundantis::Result<()> {
let _abundantis = Abundantis::builder()
    .root(".")
    .provider(MonorepoProviderType::Custom)
    .with_shell()
    .env_files(vec![".env", ".env.local"])
    .build()?;

# Ok(())
# }
```
"#
)]
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────┐
//! │                    Abundantis                 │
//! │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  │
//! │  │  Source     │  │ Resolution  │  │  Workspace  │  │
//! │  │  Registry   │──│    Engine   │──│   Manager   │  │
//! │  └─────────────┘  └─────────────┘  └─────────────┘  │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Features
//!
//! - **Plugin Architecture**: Add custom sources via `EnvSource` trait
//! - **Multiple Sources**: File, shell, memory, remote (prepared for future)
//! - **Dependency Resolution**: Full interpolation with cycle detection
//! - **Workspace Support**: Monorepo providers (Turbo, Nx, Lerna, etc.)
//! - **Event System**: Async event bus for reactive updates
//! - **Multi-level Cache**: Hot LRU cache + warm TTL cache
//!
//! ## Performance
//!
//! - Zero-copy parsing via `korni`
//! - SIMD interpolation via `germi`
//! - Lock-free concurrent access with `dashmap`
//! - Cache-friendly data structures with `hashbrown`
//! - Small string optimization with `compact_str`

pub mod config;
pub mod error;
pub mod events;
pub mod path_cache;
pub mod resolution;
pub mod selection;
pub mod source;
pub mod workspace;

pub mod watch;
pub mod watch_manager;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[cfg(feature = "async")]
use maybe_async::must_be_async;
#[cfg(not(feature = "async"))]
use maybe_async::must_be_sync;

pub use config::{
    AbundantisConfig, CacheConfig, InterpolationConfig, MonorepoProviderType, ResolutionConfig,
    SourceDefaults, SourcesConfig,
};
pub use error::{AbundantisError, Diagnostic, DiagnosticCode, DiagnosticSeverity, Result};
#[cfg(feature = "async")]
pub use events::{AbundantisEvent, EventBus, EventSubscriber};
pub use path_cache::PathCache;
pub use resolution::{
    CacheKey, DependencyGraph, ResolutionCache, ResolutionEngine, ResolvedVariable,
};
#[cfg(feature = "async")]
pub use source::AsyncEnvSource;
#[cfg(feature = "file")]
pub use source::FileSource;
#[cfg(feature = "file")]
pub use source::FileSourceManager;
#[cfg(feature = "shell")]
pub use source::ShellSource;
pub use source::{
    EnvSource, MemorySource, ParsedVariable, Priority, SourceCapabilities, SourceId,
    SourceRefreshOptions, SourceType, VariableSource,
};
#[cfg(all(feature = "watch", feature = "async"))]
pub use watch_manager::WatchManager;
pub use workspace::{PackageInfo, WorkspaceContext, WorkspaceManager};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Options for Abundantis refresh operations.

#[derive(Debug, Clone, Default)]
pub struct RefreshOptions {
    pub preserve_file_config: bool,
    /// Preserve shell source configuration
    pub preserve_shell_config: bool,

    pub preserve_remote_config: bool,

    pub preserve_precedence: bool,
}

impl RefreshOptions {
    pub fn preserve_all() -> Self {
        Self {
            preserve_file_config: true,
            preserve_shell_config: true,
            preserve_remote_config: true,
            preserve_precedence: true,
        }
    }

    pub fn reset_all() -> Self {
        Self::default()
    }
}

pub struct Abundantis {
    pub config: AbundantisConfig,
    pub registry: Arc<source::SourceRegistry>,
    pub resolution: Arc<resolution::ResolutionEngine>,
    pub workspace: Arc<parking_lot::RwLock<workspace::WorkspaceManager>>,
    cache: Arc<resolution::ResolutionCache>,
    selector: Arc<selection::ActiveFileSelector>,
    global_active_files: parking_lot::RwLock<Option<Vec<String>>>,
    directory_active_files: parking_lot::RwLock<HashMap<PathBuf, Vec<String>>>,
    path_to_source_id: parking_lot::RwLock<HashMap<PathBuf, source::SourceId>>,
    path_cache: path_cache::PathCache,
    #[cfg(feature = "async")]
    event_bus: Arc<events::EventBus>,
    #[cfg(not(feature = "async"))]
    event_bus: Arc<events::EventBus>,
}

impl Abundantis {
    pub fn builder() -> core::AbundantisBuilder {
        core::AbundantisBuilder::default()
    }

    #[cfg_attr(feature = "async", must_be_async)]
    #[cfg_attr(not(feature = "async"), must_be_sync)]
    pub async fn get_for_file(
        &self,
        key: &str,
        file_path: &std::path::Path,
    ) -> crate::Result<Option<Arc<ResolvedVariable>>> {
        let context = {
            let workspace = self.workspace.read();
            workspace
                .context_for_file(file_path)
                .ok_or_else(|| AbundantisError::Config {
                    message: format!(
                        "No workspace context found for file: {}",
                        file_path.display()
                    ),
                    path: Some(file_path.to_path_buf()),
                })?
        };

        let active_files = self.active_env_files(file_path);
        self.get_in_context_with_filter(key, &context, &active_files)
            .await
    }

    #[cfg_attr(feature = "async", must_be_async)]
    #[cfg_attr(not(feature = "async"), must_be_sync)]
    pub async fn get_in_context(
        &self,
        key: &str,
        context: &workspace::WorkspaceContext,
    ) -> crate::Result<Option<Arc<ResolvedVariable>>> {
        self.resolution.resolve(key, context, &self.registry).await
    }

    #[cfg_attr(feature = "async", must_be_async)]
    #[cfg_attr(not(feature = "async"), must_be_sync)]
    pub async fn all_for_file(
        &self,
        file_path: &std::path::Path,
    ) -> crate::Result<Vec<Arc<ResolvedVariable>>> {
        let context = {
            let workspace = self.workspace.read();
            workspace
                .context_for_file(file_path)
                .ok_or_else(|| AbundantisError::Config {
                    message: format!(
                        "No workspace context found for file: {}",
                        file_path.display()
                    ),
                    path: Some(file_path.to_path_buf()),
                })?
        };

        let active_files = self.active_env_files(file_path);
        self.all_in_context_with_filter(&context, &active_files)
            .await
    }

    #[cfg_attr(feature = "async", must_be_async)]
    #[cfg_attr(not(feature = "async"), must_be_sync)]
    pub async fn all_in_context(
        &self,
        context: &workspace::WorkspaceContext,
    ) -> crate::Result<Vec<Arc<ResolvedVariable>>> {
        self.resolution.all_variables(context, &self.registry).await
    }

    #[cfg_attr(feature = "async", must_be_async)]
    #[cfg_attr(not(feature = "async"), must_be_sync)]
    async fn get_in_context_with_filter(
        &self,
        key: &str,
        context: &workspace::WorkspaceContext,
        active_files: &[PathBuf],
    ) -> crate::Result<Option<Arc<ResolvedVariable>>> {
        let file_source_ids = self.get_source_ids_for_paths(active_files);
        self.resolution
            .resolve_with_filter(key, context, &self.registry, Some(&file_source_ids))
            .await
    }

    #[cfg_attr(feature = "async", must_be_async)]
    #[cfg_attr(not(feature = "async"), must_be_sync)]
    async fn all_in_context_with_filter(
        &self,
        context: &workspace::WorkspaceContext,
        active_files: &[PathBuf],
    ) -> crate::Result<Vec<Arc<ResolvedVariable>>> {
        let file_source_ids = self.get_source_ids_for_paths(active_files);

        self.resolution
            .all_variables_with_filter(context, &self.registry, Some(&file_source_ids))
            .await
    }

    #[cfg(feature = "async")]
    pub async fn refresh(&self, options: RefreshOptions) -> Result<()> {
        self.refresh_inner(&options)?;
        self.event_bus
            .publish_async(events::AbundantisEvent::CacheInvalidated { scope: None })
            .await;
        Ok(())
    }

    #[cfg(not(feature = "async"))]
    pub fn refresh(&self, options: RefreshOptions) -> Result<()> {
        self.refresh_inner(&options)
    }

    fn refresh_inner(&self, options: &RefreshOptions) -> Result<()> {
        let file_config_backup = if options.preserve_file_config {
            let current_global = self.global_active_files.read().clone();

            if current_global.is_none() {
                let workspace = self.workspace.read();
                let root = workspace.root().to_path_buf();
                drop(workspace);

                let auto_files = self.active_env_files(&root);
                let patterns: Vec<String> = auto_files
                    .iter()
                    .map(|p| p.to_string_lossy().to_string())
                    .collect();

                Some((
                    if patterns.is_empty() {
                        None
                    } else {
                        Some(patterns)
                    },
                    self.directory_active_files.read().clone(),
                ))
            } else {
                Some((current_global, self.directory_active_files.read().clone()))
            }
        } else {
            None
        };

        let source_options = source::SourceRefreshOptions {
            preserve_config: options.preserve_file_config,
        };

        for source in self.registry.sync_sources_by_priority() {
            source.refresh(&source_options);
        }

        {
            let workspace = self.workspace.write();
            workspace.refresh()?;
        }

        self.rediscover_file_sources()?;

        if let Some((global, directory)) = file_config_backup {
            *self.global_active_files.write() = global;
            *self.directory_active_files.write() = directory;
        }

        self.cache.clear();
        self.path_to_source_id.write().clear();

        Ok(())
    }

    pub fn event_bus(&self) -> &events::EventBus {
        &self.event_bus
    }

    pub fn config(&self) -> &AbundantisConfig {
        &self.config
    }

    pub fn stats(&self) -> AbundantisStats {
        AbundantisStats {
            cached_variables: self.cache.len(),
            source_count: self.registry.source_count(),
        }
    }

    pub fn set_active_files(&self, patterns: &[impl AsRef<str>]) {
        let patterns_vec: Vec<String> = patterns.iter().map(|p| p.as_ref().to_string()).collect();
        *self.global_active_files.write() = Some(patterns_vec);
        self.path_to_source_id.write().clear();
        self.cache.clear();
    }

    pub fn set_active_files_for_directory(
        &self,
        directory: impl AsRef<Path>,
        patterns: &[impl AsRef<str>],
    ) {
        let dir_path = directory.as_ref().to_path_buf();
        let canonical_dir = self.path_cache.canonicalize(&dir_path);
        let patterns_vec: Vec<String> = patterns.iter().map(|p| p.as_ref().to_string()).collect();
        self.directory_active_files
            .write()
            .insert(canonical_dir, patterns_vec);
        self.path_to_source_id.write().clear();
        self.cache.clear();
    }

    pub fn active_env_files(&self, file_path: impl AsRef<Path>) -> Vec<PathBuf> {
        let workspace = self.workspace.read();
        let global = self.global_active_files.read();
        let directory_scoped = self.directory_active_files.read();

        self.selector.compute_active_files(
            file_path.as_ref(),
            global.as_deref(),
            &directory_scoped,
            &workspace,
        )
    }

    pub fn clear_active_files(&self) {
        *self.global_active_files.write() = None;
        self.path_to_source_id.write().clear();
        self.cache.clear();
    }

    pub fn clear_active_files_for_directory(&self, directory: impl AsRef<Path>) {
        let dir_path = directory.as_ref().to_path_buf();
        let canonical_dir = self.path_cache.canonicalize(&dir_path);
        self.directory_active_files.write().remove(&canonical_dir);
        self.path_to_source_id.write().clear();
        self.cache.clear();
    }

    pub fn clear_all_active_files(&self) {
        self.clear_active_files();
        self.directory_active_files.write().clear();
    }

    #[cfg(feature = "async")]
    pub async fn set_root(&self, new_root: impl AsRef<Path>) -> Result<()> {
        self.set_root_inner(new_root.as_ref())?;
        self.event_bus
            .publish_async(events::AbundantisEvent::CacheInvalidated { scope: None })
            .await;
        Ok(())
    }

    #[cfg(not(feature = "async"))]
    pub fn set_root(&self, new_root: impl AsRef<Path>) -> Result<()> {
        self.set_root_inner(new_root.as_ref())
    }

    fn set_root_inner(&self, new_root: &Path) -> Result<()> {
        let new_root = new_root.canonicalize().map_err(AbundantisError::Io)?;

        tracing::info!("Changing workspace root to: {:?}", new_root);

        let mut workspace_config = self.config.workspace.clone();

        if workspace_config.provider.is_none() {
            if let Some(detected) = workspace::provider::ProviderRegistry::detect(&new_root) {
                tracing::info!("Auto-detected workspace provider: {:?}", detected);
                workspace_config.provider = Some(detected);
            } else {
                tracing::info!("No workspace provider detected, defaulting to custom");
                workspace_config.provider = Some(config::MonorepoProviderType::Custom);
                if workspace_config.roots.is_empty() {
                    workspace_config.roots.push(".".into());
                }
            }
        }

        let new_workspace = workspace::WorkspaceManager::with_root(new_root, &workspace_config)?;

        {
            let mut workspace = self.workspace.write();
            *workspace = new_workspace;
        }

        self.rediscover_file_sources()?;

        self.cache.clear();
        self.path_to_source_id.write().clear();

        Ok(())
    }

    fn get_source_ids_for_paths(
        &self,
        paths: &[PathBuf],
    ) -> std::collections::HashSet<source::SourceId> {
        let mut cache = self.path_to_source_id.write();
        let mut result = std::collections::HashSet::new();

        for path in paths {
            let canonical = self.path_cache.canonicalize(path);

            if let Some(source_id) = cache.get(&canonical) {
                result.insert(source_id.clone());
                continue;
            }

            let source_id = source::SourceId::from(format!("file:{}", canonical.display()));
            cache.insert(canonical, source_id.clone());
            result.insert(source_id);
        }

        result
    }

    #[cfg(feature = "file")]
    fn rediscover_file_sources(&self) -> Result<()> {
        use std::collections::HashSet;

        let workspace = self.workspace.read();
        let mut discovered_paths: HashSet<PathBuf> = HashSet::new();

        for package in workspace.packages() {
            for pattern in &self.config.workspace.env_files {
                let full_pattern = package.root.join(pattern.as_str());
                let pattern_str = full_pattern.to_string_lossy();

                if let Ok(paths) = glob::glob(&pattern_str) {
                    for entry in paths.flatten() {
                        if entry.is_file() {
                            if let Ok(canonical) = entry.canonicalize() {
                                discovered_paths.insert(canonical);
                            } else {
                                discovered_paths.insert(entry);
                            }
                        }
                    }
                }
            }
        }

        for path in &discovered_paths {
            let source_id = source::SourceId::from(format!("file:{}", path.display()));
            if !self.registry.is_registered(&source_id) {
                if let Ok(file_source) = source::FileSource::new(path) {
                    tracing::info!("Discovered new env file: {}", path.display());
                    self.registry
                        .register_sync(Arc::new(file_source) as Arc<dyn source::EnvSource>);
                }
            }
        }

        let registered_paths = self.registry.registered_file_paths();
        for registered_path in registered_paths {
            if !discovered_paths.contains(&registered_path) && !registered_path.exists() {
                let source_id =
                    source::SourceId::from(format!("file:{}", registered_path.display()));
                tracing::info!("Removing deleted env file: {}", registered_path.display());
                self.registry.unregister_sync(&source_id);
            }
        }

        Ok(())
    }

    #[cfg(not(feature = "file"))]
    fn rediscover_file_sources(&self) -> Result<()> {
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct AbundantisStats {
    pub cached_variables: usize,
    pub source_count: usize,
}

mod core;
