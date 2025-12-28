//! # Abundantis
//!
//! High-performance unified environment variable management from multiple sources.
//!
//! ## Quick Start
//!
//! ```no_run
//! use abundantis::{Abundantis, config::MonorepoProviderType};
//!
//! # #[tokio::main]
//! # async fn example() -> abundantis::Result<()> {
//! let _abundantis = Abundantis::builder()
//!     .root(".")
//!     .provider(MonorepoProviderType::Custom)
//!     .with_shell()
//!     .env_files(vec![".env", ".env.local"])
//!     .build()
//!     .await?;
//!
//! # Ok(())
//! # }
//! ```
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

pub use config::{AbundantisConfig, MonorepoProviderType, CacheConfig, InterpolationConfig, ResolutionConfig};
pub use error::{AbundantisError, Diagnostic, DiagnosticCode, DiagnosticSeverity, Result};
pub use resolution::{DependencyGraph, ResolutionEngine, ResolutionCache, ResolvedVariable, CacheKey};
pub use source::{EnvSource, MemorySource, SourceId, SourceType, SourceCapabilities, Priority, VariableSource, ParsedVariable};
#[cfg(feature = "file")]
pub use source::FileSource;
#[cfg(feature = "shell")]
pub use source::ShellSource;
#[cfg(feature = "async")]
pub use source::AsyncEnvSource;
pub use workspace::{PackageInfo, WorkspaceContext, WorkspaceManager};
pub use path_cache::PathCache;
#[cfg(feature = "async")]
pub use events::{AbundantisEvent, EventBus, EventSubscriber};
#[cfg(all(feature = "watch", feature = "async"))]
pub use watch_manager::WatchManager;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Main entry point for Abundantis.
///
/// Async-first design with sync wrappers for convenience.
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

    #[cfg(feature = "async")]
    pub async fn get_for_file(
        &self,
        key: &str,
        file_path: &std::path::Path,
    ) -> crate::Result<Option<Arc<ResolvedVariable>>> {
        let context = {
            let workspace = self.workspace.read();
            workspace.context_for_file(file_path)
                .ok_or_else(|| AbundantisError::Config {
                    message: format!("No workspace context found for file: {}", file_path.display()),
                    path: Some(file_path.to_path_buf()),
                })?
        };

        let active_files = self.active_env_files(file_path);
        self.get_in_context_with_filter(key, &context, &active_files).await
    }

    #[cfg(not(feature = "async"))]
    pub fn get_for_file(
        &self,
        key: &str,
        file_path: &std::path::Path,
    ) -> crate::Result<Option<Arc<ResolvedVariable>>> {
        let context = {
            let workspace = self.workspace.read();
            workspace.context_for_file(file_path)
                .ok_or_else(|| AbundantisError::Config {
                    message: format!("No workspace context found for file: {}", file_path.display()),
                    path: Some(file_path.to_path_buf()),
                })?
        };

        let active_files = self.active_env_files(file_path);
        self.get_in_context_with_filter(key, &context, &active_files)
    }

    #[cfg(feature = "async")]
    pub async fn get_in_context(
        &self,
        key: &str,
        context: &workspace::WorkspaceContext,
    ) -> crate::Result<Option<Arc<ResolvedVariable>>> {
        self.resolution.resolve(key, context, &self.registry).await
    }

    #[cfg(not(feature = "async"))]
    pub fn get_in_context(
        &self,
        key: &str,
        context: &workspace::WorkspaceContext,
    ) -> crate::Result<Option<Arc<ResolvedVariable>>> {
        self.resolution.resolve(key, context, &self.registry)
    }

    #[cfg(feature = "async")]
    pub async fn all_for_file(&self, file_path: &std::path::Path) -> crate::Result<Vec<Arc<ResolvedVariable>>> {
        let context = {
            let workspace = self.workspace.read();
            workspace.context_for_file(file_path)
                .ok_or_else(|| AbundantisError::Config {
                    message: format!("No workspace context found for file: {}", file_path.display()),
                    path: Some(file_path.to_path_buf()),
                })?
        };

        let active_files = self.active_env_files(file_path);
        self.all_in_context_with_filter(&context, &active_files).await
    }

    #[cfg(not(feature = "async"))]
    pub fn all_for_file(&self, file_path: &std::path::Path) -> crate::Result<Vec<Arc<ResolvedVariable>>> {
        let context = {
            let workspace = self.workspace.read();
            workspace.context_for_file(file_path)
                .ok_or_else(|| AbundantisError::Config {
                    message: format!("No workspace context found for file: {}", file_path.display()),
                    path: Some(file_path.to_path_buf()),
                })?
        };

        let active_files = self.active_env_files(file_path);
        self.all_in_context_with_filter(&context, &active_files)
    }

    #[cfg(feature = "async")]
    pub async fn all_in_context(
        &self,
        context: &workspace::WorkspaceContext,
    ) -> crate::Result<Vec<Arc<ResolvedVariable>>> {
        self.resolution.all_variables(context, &self.registry).await
    }

    #[cfg(not(feature = "async"))]
    pub fn all_in_context(
        &self,
        context: &workspace::WorkspaceContext,
    ) -> crate::Result<Vec<Arc<ResolvedVariable>>> {
        self.resolution.all_variables(context, &self.registry)
    }

    #[cfg(feature = "async")]
    async fn get_in_context_with_filter(
        &self,
        key: &str,
        context: &workspace::WorkspaceContext,
        active_files: &[PathBuf],
    ) -> crate::Result<Option<Arc<ResolvedVariable>>> {
        let file_source_ids: std::collections::HashSet<source::SourceId> = self.get_source_ids_for_paths(active_files);

        self.resolution
            .resolve_with_filter(key, context, &self.registry, Some(&file_source_ids))
            .await
    }

    #[cfg(not(feature = "async"))]
    fn get_in_context_with_filter(
        &self,
        key: &str,
        context: &workspace::WorkspaceContext,
        active_files: &[PathBuf],
    ) -> crate::Result<Option<Arc<ResolvedVariable>>> {
        let file_source_ids = self.get_source_ids_for_paths(active_files);

        self.resolution
            .resolve_with_filter(key, context, &self.registry, Some(&file_source_ids))
    }

    #[cfg(feature = "async")]
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

    #[cfg(not(feature = "async"))]
    fn all_in_context_with_filter(
        &self,
        context: &workspace::WorkspaceContext,
        active_files: &[PathBuf],
    ) -> crate::Result<Vec<Arc<ResolvedVariable>>> {
        let file_source_ids = self.get_source_ids_for_paths(active_files);

        self.resolution
            .all_variables_with_filter(context, &self.registry, Some(&file_source_ids))
    }

    #[cfg(feature = "async")]
    pub async fn refresh(&self) -> Result<()> {
        for source in self.registry.sync_sources_by_priority() {
            source.invalidate();
        }

        {
            let workspace = self.workspace.write();
            workspace.refresh()?;
        }

        self.cache.clear();

        self.event_bus.publish_async(events::AbundantisEvent::CacheInvalidated {
            scope: None,
        }).await;

        Ok(())
    }

    #[cfg(not(feature = "async"))]
    pub fn refresh(&self) -> Result<()> {
        for source in self.registry.sync_sources_by_priority() {
            source.invalidate();
        }

        {
            let workspace = self.workspace.write();
            workspace.refresh()?;
        }

        self.cache.clear();

        Ok(())
    }

    #[cfg(feature = "async")]
    pub fn event_bus(&self) -> &events::EventBus {
        &self.event_bus
    }

    #[cfg(not(feature = "async"))]
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

    pub fn set_active_files_for_directory(&self, directory: impl AsRef<Path>, patterns: &[impl AsRef<str>]) {
        let dir_path = directory.as_ref().to_path_buf();
        let canonical_dir = self.path_cache.canonicalize(&dir_path);
        let patterns_vec: Vec<String> = patterns.iter().map(|p| p.as_ref().to_string()).collect();
        self.directory_active_files.write().insert(canonical_dir, patterns_vec);
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

    fn get_source_ids_for_paths(&self, paths: &[PathBuf]) -> std::collections::HashSet<source::SourceId> {
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
}

#[derive(Debug, Clone)]
pub struct AbundantisStats {
    pub cached_variables: usize,
    pub source_count: usize,
}

mod core;
