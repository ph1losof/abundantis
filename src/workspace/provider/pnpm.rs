use super::{MonorepoProvider, PackageInfo};
use crate::config::MonorepoProviderType;
use compact_str::CompactString;
use globset::Glob;
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
    let mut packages = Vec::new();
    let mut exclusion_matchers = Vec::new();

    let mut inclusion_patterns = Vec::new();
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
        let full_pattern = root.join(pattern);
        let pattern_str = full_pattern.to_string_lossy();

        if let Ok(glob) = Glob::new(&pattern_str) {
            let matcher = glob.compile_matcher();

            for entry in walkdir::WalkDir::new(root)
                .max_depth(3)
                .into_iter()
                .filter_entry(|e| {
                    let name = e.file_name().to_str().unwrap_or("");
                    !matches!(name, "node_modules" | ".git" | "dist")
                })
                .flatten()
            {
                if entry.file_type().is_dir() && matcher.is_match(entry.path()) {
                    let excluded = exclusion_matchers
                        .iter()
                        .any(|excl| excl.is_match(entry.path()));

                    if !excluded {
                        let pkg_json = entry.path().join("package.json");
                        if pkg_json.exists() {
                            let name = std::fs::read_to_string(&pkg_json)
                                .ok()
                                .and_then(|c| serde_json::from_str::<serde_json::Value>(&c).ok())
                                .and_then(|v| v.get("name")?.as_str().map(CompactString::new));

                            let relative_path = entry
                                .path()
                                .strip_prefix(root)
                                .unwrap_or(entry.path())
                                .to_string_lossy();

                            packages.push(PackageInfo {
                                root: entry.path().to_path_buf(),
                                name,
                                relative_path: CompactString::new(&relative_path),
                            });
                        }
                    }
                }
            }
        }
    }

    Ok(packages)
}
