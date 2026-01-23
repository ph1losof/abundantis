use super::{MonorepoProvider, PackageInfo};
use crate::config::MonorepoProviderType;
use serde::Deserialize;
use std::path::Path;

pub struct NpmProvider;

impl NpmProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for NpmProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum Workspaces {
    Array(Vec<String>),
    Object { packages: Vec<String> },
}

#[derive(Debug, Deserialize)]
struct PackageJson {
    workspaces: Option<Workspaces>,
}

impl MonorepoProvider for NpmProvider {
    fn provider_type(&self) -> MonorepoProviderType {
        MonorepoProviderType::Npm
    }

    fn config_file(&self) -> &'static str {
        "package.json"
    }

    fn detect(&self, root: &Path) -> bool {
        let pkg_path = root.join("package.json");
        if !pkg_path.exists() {
            return false;
        }

        std::fs::read_to_string(&pkg_path)
            .ok()
            .and_then(|c| serde_json::from_str::<PackageJson>(&c).ok())
            .map(|p| p.workspaces.is_some())
            .unwrap_or(false)
    }

    fn discover_packages(&self, root: &Path) -> crate::Result<Vec<PackageInfo>> {
        let pkg_path = root.join("package.json");
        let content = std::fs::read_to_string(&pkg_path).unwrap_or_default();
        let pkg: PackageJson =
            serde_json::from_str(&content).unwrap_or(PackageJson { workspaces: None });

        let patterns = match pkg.workspaces {
            Some(Workspaces::Array(arr)) => arr,
            Some(Workspaces::Object { packages }) => packages,
            None => return Ok(Vec::new()),
        };

        super::pnpm::expand_package_patterns(root, &patterns)
    }
}
