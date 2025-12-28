//! Turborepo provider.

use super::{MonorepoProvider, PackageInfo};
use crate::config::MonorepoProviderType;
use serde::Deserialize;
use std::path::Path;

/// Turborepo provider.
///
/// Turbo delegates workspace management to the underlying package manager
/// (pnpm, npm, yarn), but we can read turbo.json for validation.
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

/// Minimal turbo.json structure.
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
        // Turbo delegates to the package manager
        // Check for pnpm first, then npm/yarn
        if root.join("pnpm-workspace.yaml").exists() {
            return super::PnpmProvider::new().discover_packages(root);
        }

        // Fall back to package.json workspaces
        super::NpmProvider::new().discover_packages(root)
    }
}
