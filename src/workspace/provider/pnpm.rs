use super::{
    traverse_glob_patterns, GlobTraversalConfig, MonorepoProvider, PackageInfo,
    STANDARD_EXCLUSIONS,
};
use crate::config::MonorepoProviderType;
use compact_str::CompactString;
use serde::Deserialize;
use std::path::Path;

pub struct PnpmProvider;

impl PnpmProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PnpmProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct PnpmWorkspace {
    #[serde(default)]
    packages: Vec<String>,
}

impl MonorepoProvider for PnpmProvider {
    fn provider_type(&self) -> MonorepoProviderType {
        MonorepoProviderType::Pnpm
    }

    fn config_file(&self) -> &'static str {
        "pnpm-workspace.yaml"
    }

    fn discover_packages(&self, root: &Path) -> crate::Result<Vec<PackageInfo>> {
        let config_path = root.join("pnpm-workspace.yaml");
        let content = std::fs::read_to_string(&config_path).unwrap_or_default();

        let config: PnpmWorkspace = serde_yaml_ng::from_str(&content).unwrap_or(PnpmWorkspace {
            packages: vec!["packages/*".to_string()],
        });

        expand_package_patterns(root, &config.packages)
    }
}

pub(super) fn expand_package_patterns(
    root: &Path,
    patterns: &[String],
) -> crate::Result<Vec<PackageInfo>> {
    let config = GlobTraversalConfig {
        max_depth: 4,
        excluded_dirs: STANDARD_EXCLUSIONS,
        marker_file: Some("package.json"),
    };

    traverse_glob_patterns(root, patterns, &config, extract_package_name)
}

fn extract_package_name(path: &Path) -> Option<CompactString> {
    let pkg_json = path.join("package.json");
    std::fs::read_to_string(&pkg_json)
        .ok()
        .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
        .and_then(|v| v.get("name")?.as_str().map(CompactString::new))
}
