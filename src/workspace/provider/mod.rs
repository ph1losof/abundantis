//! Monorepo provider trait and implementations.

mod registry;
mod turbo;
mod nx;
mod lerna;
mod pnpm;
mod npm;
mod cargo;
mod custom;

pub use registry::ProviderRegistry;

use crate::config::MonorepoProviderType;
use super::context::PackageInfo;
use std::path::Path;

pub trait MonorepoProvider: Send + Sync {
    fn provider_type(&self) -> MonorepoProviderType;
    fn config_file(&self) -> &'static str;
    fn detect(&self, root: &Path) -> bool {
        root.join(self.config_file()).exists()
    }
    fn discover_packages(&self, root: &Path) -> crate::Result<Vec<PackageInfo>>;
}

// Re-export implementations
pub use turbo::TurboProvider;
pub use nx::NxProvider;
pub use lerna::LernaProvider;
pub use pnpm::PnpmProvider;
pub use npm::NpmProvider;
pub use cargo::CargoProvider;
pub use custom::CustomProvider;
