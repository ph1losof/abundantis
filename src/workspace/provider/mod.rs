mod cargo;
mod custom;
mod lerna;
mod npm;
mod nx;
mod pnpm;
mod registry;
mod turbo;

pub use registry::ProviderRegistry;

use super::context::PackageInfo;
use crate::config::MonorepoProviderType;
use compact_str::CompactString;
use globset::Glob;
use std::path::Path;
use walkdir::WalkDir;

pub trait MonorepoProvider: Send + Sync {
    fn provider_type(&self) -> MonorepoProviderType;
    fn config_file(&self) -> &'static str;
    fn detect(&self, root: &Path) -> bool {
        root.join(self.config_file()).exists()
    }
    fn discover_packages(&self, root: &Path) -> crate::Result<Vec<PackageInfo>>;
}

/// Configuration for glob pattern traversal.
pub struct GlobTraversalConfig {
    pub max_depth: usize,
    pub excluded_dirs: &'static [&'static str],
    pub marker_file: Option<&'static str>,
}

/// Standard directory exclusions for most providers.
pub const STANDARD_EXCLUSIONS: &[&str] = &["node_modules", ".git", "target", "dist", "build"];

/// Traverses the root directory matching glob patterns and returns discovered packages.
///
/// Handles both inclusion patterns (e.g., "packages/*") and exclusion patterns (e.g., "!packages/internal").
/// The `name_extractor` callback is called for each matched directory to extract the package name.
pub fn traverse_glob_patterns<F>(
    root: &Path,
    patterns: &[String],
    config: &GlobTraversalConfig,
    name_extractor: F,
) -> crate::Result<Vec<PackageInfo>>
where
    F: Fn(&Path) -> Option<CompactString>,
{
    let mut packages = Vec::new();
    let mut exclusion_matchers = Vec::new();
    let mut inclusion_patterns = Vec::new();

    // Separate inclusion and exclusion patterns
    for pattern in patterns {
        if let Some(excl_pattern) = pattern.strip_prefix('!') {
            let full_pattern = root.join(excl_pattern);
            let pattern_str = full_pattern.to_string_lossy();
            if let Ok(glob) = Glob::new(&pattern_str) {
                exclusion_matchers.push(glob.compile_matcher());
            }
        } else {
            inclusion_patterns.push(pattern.clone());
        }
    }

    for pattern in inclusion_patterns {
        let full_pattern = root.join(&pattern);
        let pattern_str = full_pattern.to_string_lossy();

        if let Ok(glob) = Glob::new(&pattern_str) {
            let matcher = glob.compile_matcher();

            for entry in WalkDir::new(root)
                .max_depth(config.max_depth)
                .into_iter()
                .filter_entry(|e| {
                    let name = e.file_name().to_str().unwrap_or("");
                    !config.excluded_dirs.contains(&name)
                })
                .flatten()
            {
                if entry.file_type().is_dir() && matcher.is_match(entry.path()) {
                    // Check exclusion patterns
                    let excluded = exclusion_matchers
                        .iter()
                        .any(|excl| excl.is_match(entry.path()));

                    if excluded {
                        continue;
                    }

                    // Check for marker file if specified
                    if let Some(marker) = config.marker_file {
                        if !entry.path().join(marker).exists() {
                            continue;
                        }
                    }

                    let relative_path = entry
                        .path()
                        .strip_prefix(root)
                        .unwrap_or(entry.path())
                        .to_string_lossy();

                    packages.push(PackageInfo {
                        root: entry.path().to_path_buf(),
                        name: name_extractor(entry.path()),
                        relative_path: CompactString::new(&relative_path),
                    });
                }
            }
        }
    }

    Ok(packages)
}

/// Traverses directories looking for a marker file (e.g., project.json) without glob matching.
pub fn traverse_for_marker_file<F>(
    root: &Path,
    config: &GlobTraversalConfig,
    name_extractor: F,
) -> crate::Result<Vec<PackageInfo>>
where
    F: Fn(&Path) -> Option<CompactString>,
{
    let mut packages = Vec::new();
    let marker = config.marker_file.expect("marker_file required for traverse_for_marker_file");

    for entry in WalkDir::new(root)
        .max_depth(config.max_depth)
        .into_iter()
        .filter_entry(|e| {
            let name = e.file_name().to_str().unwrap_or("");
            !config.excluded_dirs.contains(&name)
        })
        .flatten()
    {
        if entry.file_name().to_str() == Some(marker) {
            let project_dir = entry.path().parent().unwrap_or(root);
            let relative_path = project_dir
                .strip_prefix(root)
                .unwrap_or(project_dir)
                .to_string_lossy();

            packages.push(PackageInfo {
                root: project_dir.to_path_buf(),
                name: name_extractor(entry.path()),
                relative_path: CompactString::new(&relative_path),
            });
        }
    }

    Ok(packages)
}

pub use cargo::CargoProvider;
pub use custom::CustomProvider;
pub use lerna::LernaProvider;
pub use npm::NpmProvider;
pub use nx::NxProvider;
pub use pnpm::PnpmProvider;
pub use turbo::TurboProvider;
