use super::{MonorepoProvider, PackageInfo};
use crate::config::MonorepoProviderType;
use serde::Deserialize;
use std::path::Path;

pub struct TurboProvider;

impl TurboProvider {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TurboProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct TurboJson {
    #[serde(default)]
    #[allow(dead_code)]
    extends: Vec<String>,
}

impl MonorepoProvider for TurboProvider {
    fn provider_type(&self) -> MonorepoProviderType {
        MonorepoProviderType::Turbo
    }

    fn config_file(&self) -> &'static str {
        "turbo.json"
    }

    fn discover_packages(&self, root: &Path) -> crate::Result<Vec<PackageInfo>> {
        if root.join("pnpm-workspace.yaml").exists() {
            return super::PnpmProvider::new().discover_packages(root);
        }

        super::NpmProvider::new().discover_packages(root)
    }
}
