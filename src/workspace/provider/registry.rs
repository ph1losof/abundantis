use super::{
    CargoProvider, CustomProvider, LernaProvider, MonorepoProvider, NpmProvider, NxProvider,
    PnpmProvider, TurboProvider,
};
use crate::config::{MonorepoProviderType, WorkspaceConfig};
use std::sync::Arc;

pub struct ProviderRegistry;

impl ProviderRegistry {
    pub fn create(config: &WorkspaceConfig) -> Option<Arc<dyn MonorepoProvider>> {
        let provider_type = config.provider?;

        let provider: Arc<dyn MonorepoProvider> = match provider_type {
            MonorepoProviderType::Turbo => Arc::new(TurboProvider::new()),
            MonorepoProviderType::Nx => Arc::new(NxProvider::new()),
            MonorepoProviderType::Lerna => Arc::new(LernaProvider::new()),
            MonorepoProviderType::Pnpm => Arc::new(PnpmProvider::new()),
            MonorepoProviderType::Npm | MonorepoProviderType::Yarn => Arc::new(NpmProvider::new()),
            MonorepoProviderType::Cargo => Arc::new(CargoProvider::new()),
            MonorepoProviderType::Custom => Arc::new(CustomProvider::new(config.roots.clone())),
        };

        Some(provider)
    }

    pub fn detect(root: &std::path::Path) -> Option<MonorepoProviderType> {
        if root.join("turbo.json").exists() {
            return Some(MonorepoProviderType::Turbo);
        }
        if root.join("nx.json").exists() {
            return Some(MonorepoProviderType::Nx);
        }
        if root.join("lerna.json").exists() {
            return Some(MonorepoProviderType::Lerna);
        }
        if root.join("pnpm-workspace.yaml").exists() {
            return Some(MonorepoProviderType::Pnpm);
        }

        let cargo_toml = root.join("Cargo.toml");
        if cargo_toml.exists() {
            if let Ok(content) = std::fs::read_to_string(&cargo_toml) {
                if content.contains("[workspace]") {
                    return Some(MonorepoProviderType::Cargo);
                }
            }
        }

        let package_json = root.join("package.json");
        if package_json.exists() {
            if let Ok(content) = std::fs::read_to_string(&package_json) {
                if content.contains("\"workspaces\"") {
                    return Some(MonorepoProviderType::Npm);
                }
            }
        }

        None
    }
}
