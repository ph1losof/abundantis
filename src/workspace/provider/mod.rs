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
use std::path::Path;

pub trait MonorepoProvider: Send + Sync {
    fn provider_type(&self) -> MonorepoProviderType;
    fn config_file(&self) -> &'static str;
    fn detect(&self, root: &Path) -> bool {
        root.join(self.config_file()).exists()
    }
    fn discover_packages(&self, root: &Path) -> crate::Result<Vec<PackageInfo>>;
}

pub use cargo::CargoProvider;
pub use custom::CustomProvider;
pub use lerna::LernaProvider;
pub use npm::NpmProvider;
pub use nx::NxProvider;
pub use pnpm::PnpmProvider;
pub use turbo::TurboProvider;
