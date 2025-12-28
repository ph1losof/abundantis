//! Workspace manager for context resolution.

use super::{PackageInfo, ProviderRegistry, WorkspaceContext};
use crate::config::WorkspaceConfig;
use crate::error::{AbundantisError as AbundantisError, Result};
use hashbrown::HashMap;
use parking_lot::RwLock;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Manages workspace state and context resolution.
///
/// This is the "router" that maps file paths to their workspace context,
/// determining which .env files apply.
pub struct WorkspaceManager {
    /// Workspace root directory.
    root: PathBuf,

    /// Configuration.
    config: WorkspaceConfig,

    /// Discovered packages (package_root -> info).
    packages: RwLock<HashMap<PathBuf, PackageInfo>>,

    /// Cache for context lookups (file_path -> context).
    context_cache: RwLock<HashMap<PathBuf, Arc<WorkspaceContext>>>,

    /// Whether cascading is enabled.
    cascading: bool,
}

impl WorkspaceManager {
    /// Create a new workspace manager.
    pub fn new(config: &WorkspaceConfig) -> Result<Self> {
        let root = std::env::current_dir()
            .map_err(AbundantisError::Io)?
            .canonicalize()
            .map_err(AbundantisError::Io)?;

        let manager = Self {
            root: root.clone(),
            config: config.clone(),
            packages: RwLock::new(HashMap::new()),
            context_cache: RwLock::new(HashMap::new()),
            cascading: config.cascading,
        };

        // Initial discovery
        manager.discover_packages()?;

        Ok(manager)
    }

    /// Create with explicit root.
    pub fn with_root(root: PathBuf, config: &WorkspaceConfig) -> Result<Self> {
        let root = root.canonicalize().map_err(AbundantisError::Io)?;

        let manager = Self {
            root,
            config: config.clone(),
            packages: RwLock::new(HashMap::new()),
            context_cache: RwLock::new(HashMap::new()),
            cascading: config.cascading,
        };

        // Initial discovery
        manager.discover_packages()?;

        Ok(manager)
    }

    /// Discover all packages using the configured provider.
    fn discover_packages(&self) -> Result<()> {
        let provider = ProviderRegistry::create(&self.config).ok_or_else(|| {
            AbundantisError::MissingConfig {
                field: "workspace.provider",
                suggestion: "Set to one of: turbo, nx, lerna, pnpm, npm, cargo, custom".into(),
            }
        })?;

        if !provider.detect(&self.root) {
            return Err(AbundantisError::ProviderConfigNotFound {
                expected_file: provider.config_file(),
                search_path: self.root.clone(),
            });
        }

        let packages = provider.discover_packages(&self.root)?;
        
        // Debug logging
        tracing::info!("Discovered {} packages at root {:?}", packages.len(), self.root);
        for pkg in &packages {
            tracing::info!("  Package: {:?} (relative: {})", pkg.root, pkg.relative_path);
        }

        {
            let mut pkg_map = self.packages.write();
            pkg_map.clear();
            for pkg in packages {
                pkg_map.insert(pkg.root.clone(), pkg);
            }
        }

        // Clear context cache
        self.context_cache.write().clear();

        Ok(())
    }

    /// Get the workspace context for a file.
    ///
    /// Returns `None` if the file is outside any known package.
    pub fn context_for_file(&self, file_path: &Path) -> Option<WorkspaceContext> {
        // Try cache first
        {
            let cache = self.context_cache.read();
            if let Some(ctx) = cache.get(file_path) {
                return Some((**ctx).clone());
            }
        }

        // Find the package this file belongs to
        let canonical = file_path.canonicalize().ok()?;
        let packages = self.packages.read();

        tracing::info!("Looking for context for {:?} (canonical: {:?})", file_path, canonical);
        tracing::info!("Available packages: {}", packages.len());

        // Find the closest package root
        let mut best_match: Option<&PackageInfo> = None;
        let mut best_depth = 0;

        for pkg in packages.values() {
            let matches = canonical.starts_with(&pkg.root);
            tracing::info!("  Checking pkg {:?}: starts_with={}", pkg.root, matches);
            if matches {
                let depth = pkg.root.components().count();
                if best_match.is_none() || depth > best_depth {
                    best_match = Some(pkg);
                    best_depth = depth;
                }
            }
        }

        // Build context
        let package = best_match?;
        let context = self.build_context(package);

        // Cache it
        {
            let mut cache = self.context_cache.write();
            cache.insert(file_path.to_path_buf(), Arc::new(context.clone()));
        }

        Some(context)
    }

    /// Build a workspace context for a package.
    fn build_context(&self, package: &PackageInfo) -> WorkspaceContext {
        let mut env_files = Vec::new();

        // If cascading, add root .env files first
        if self.cascading && package.root != self.root {
            for pattern in &self.config.env_files {
                let path = self.root.join(pattern.as_str());
                if path.exists() {
                    env_files.push(path);
                }
            }
        }

        // Add package-level .env files
        for pattern in &self.config.env_files {
            let path = package.root.join(pattern.as_str());
            if path.exists() {
                env_files.push(path);
            }
        }

        WorkspaceContext {
            workspace_root: self.root.clone(),
            package_root: package.root.clone(),
            package_name: package.name.clone(),
            env_files,
        }
    }

    /// Refresh package discovery.
    pub fn refresh(&self) -> Result<()> {
        self.discover_packages()
    }

    /// Get all discovered packages.
    pub fn packages(&self) -> Vec<PackageInfo> {
        self.packages.read().values().cloned().collect()
    }

    /// Get workspace root.
    pub fn root(&self) -> &Path {
        &self.root
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::MonorepoProviderType;

    #[test]
    fn test_custom_provider() {
        // This test would need a temp directory with test structure
        // For now, just verify the manager can be created
        let config = WorkspaceConfig {
            provider: Some(MonorepoProviderType::Custom),
            roots: vec!["*".into()],
            ..Default::default()
        };

        // Would fail without proper directory structure
        // Just testing the API compiles
        assert!(config.provider.is_some());
    }
}
