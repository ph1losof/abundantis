//! Nx monorepo provider.

use super::{MonorepoProvider, PackageInfo};
use crate::config::MonorepoProviderType;
use compact_str::CompactString;
use serde::Deserialize;
use std::path::Path;
use walkdir::WalkDir;

/// Nx monorepo provider.
///
/// Discovers projects by scanning for project.json files.
pub struct NxProvider;

impl NxProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for NxProvider {
    fn default() -> Self {
        Self::new()
    }
}

/// Minimal nx.json structure.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NxJson {
    #[serde(default)]
    #[allow(dead_code)]
    workspace_layout: Option<WorkspaceLayout>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct WorkspaceLayout {
    #[allow(dead_code)]
    apps_dir: Option<String>,
    #[allow(dead_code)]
    libs_dir: Option<String>,
}

/// project.json structure.
#[derive(Debug, Deserialize)]
struct ProjectJson {
    name: Option<String>,
}

impl MonorepoProvider for NxProvider {
    fn provider_type(&self) -> MonorepoProviderType {
        MonorepoProviderType::Nx
    }

    fn config_file(&self) -> &'static str {
        "nx.json"
    }

    fn discover_packages(&self, root: &Path) -> crate::Result<Vec<PackageInfo>> {
        let mut packages = Vec::new();

        // Walk the directory looking for project.json files
        for entry in WalkDir::new(root)
            .max_depth(4) // Limit depth for performance
            .into_iter()
            .filter_entry(|e| {
                // Skip common ignore patterns
                let name = e.file_name().to_str().unwrap_or("");
                !matches!(name, "node_modules" | ".git" | "dist" | "build" | "target")
            })
            .flatten()
        {
            if entry.file_name() == "project.json" {
                let project_dir = entry.path().parent().unwrap_or(root);

                // Parse project.json for name
                let name = std::fs::read_to_string(entry.path())
                    .ok()
                    .and_then(|content| serde_json::from_str::<ProjectJson>(&content).ok())
                    .and_then(|p| p.name)
                    .map(CompactString::new);

                let relative_path = project_dir
                    .strip_prefix(root)
                    .unwrap_or(project_dir)
                    .to_string_lossy();

                packages.push(PackageInfo {
                    root: project_dir.to_path_buf(),
                    name,
                    relative_path: CompactString::new(&relative_path),
                });
            }
        }

        Ok(packages)
    }
}
