//! Workspace management and monorepo provider detection.

mod manager;
pub mod provider;

mod context;

pub use manager::WorkspaceManager;
pub use provider::{MonorepoProvider, ProviderRegistry};
pub use context::{WorkspaceContext, PackageInfo};
