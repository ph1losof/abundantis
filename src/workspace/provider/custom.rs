use super::{
    traverse_glob_patterns, GlobTraversalConfig, MonorepoProvider, PackageInfo,
    STANDARD_EXCLUSIONS,
};
use crate::config::MonorepoProviderType;
use compact_str::CompactString;
use std::path::Path;

pub struct CustomProvider {
    patterns: Vec<CompactString>,
}

impl CustomProvider {
    pub fn new(patterns: Vec<CompactString>) -> Self {
        Self { patterns }
    }
}

impl MonorepoProvider for CustomProvider {
    fn provider_type(&self) -> MonorepoProviderType {
        MonorepoProviderType::Custom
    }

    fn config_file(&self) -> &'static str {
        ".ecolog.toml"
    }

    fn detect(&self, _root: &Path) -> bool {
        true
    }

    fn discover_packages(&self, root: &Path) -> crate::Result<Vec<PackageInfo>> {
        let mut packages = Vec::new();

        // Handle root directory pattern specially
        let (root_patterns, glob_patterns): (Vec<_>, Vec<_>) = self
            .patterns
            .iter()
            .partition(|p| p.as_str() == ".");

        for _ in root_patterns {
            packages.push(PackageInfo {
                root: root.to_path_buf(),
                name: None,
                relative_path: CompactString::new("."),
            });
        }

        if !glob_patterns.is_empty() {
            let patterns: Vec<String> = glob_patterns.iter().map(|p| p.to_string()).collect();
            let config = GlobTraversalConfig {
                max_depth: 4,
                excluded_dirs: STANDARD_EXCLUSIONS,
                marker_file: None, // Custom provider doesn't require marker files
            };
            packages.extend(traverse_glob_patterns(root, &patterns, &config, |_| None)?);
        }

        Ok(packages)
    }
}
