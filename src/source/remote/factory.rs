//! Factory trait for creating remote sources from configuration.

use super::traits::{ProviderConfig, RemoteSource};
use crate::error::SourceError;
use async_trait::async_trait;
use std::sync::Arc;

/// Factory trait for creating remote sources.
///
/// Implementations of this trait create instances of specific remote source
/// providers (e.g., Doppler, AWS, Vault) from configuration.
#[async_trait]
pub trait RemoteSourceFactory: Send + Sync {
    /// Returns the provider ID this factory creates (e.g., "doppler", "aws").
    fn provider_id(&self) -> &str;

    /// Returns a human-readable name for the provider.
    fn provider_name(&self) -> &str;

    /// Creates a new remote source instance from configuration.
    ///
    /// The factory should:
    /// 1. Validate the configuration
    /// 2. Set up any required clients or connections
    /// 3. Return an unauthenticated source (auth happens separately)
    async fn create(&self, config: &ProviderConfig) -> Result<Arc<dyn RemoteSource>, SourceError>;

    /// Returns whether this provider is available in the current environment.
    ///
    /// Some providers may require specific dependencies or credentials to be
    /// available. This method allows factories to indicate unavailability.
    fn is_available(&self) -> bool {
        true
    }
}

/// A simple function-based factory wrapper.
pub struct RemoteSourceFactoryFn<F>
where
    F: Fn(&ProviderConfig) -> Result<Arc<dyn RemoteSource>, SourceError> + Send + Sync,
{
    provider_id: &'static str,
    provider_name: &'static str,
    create_fn: F,
}

impl<F> RemoteSourceFactoryFn<F>
where
    F: Fn(&ProviderConfig) -> Result<Arc<dyn RemoteSource>, SourceError> + Send + Sync,
{
    pub fn new(provider_id: &'static str, provider_name: &'static str, create_fn: F) -> Self {
        Self {
            provider_id,
            provider_name,
            create_fn,
        }
    }
}

#[async_trait]
impl<F> RemoteSourceFactory for RemoteSourceFactoryFn<F>
where
    F: Fn(&ProviderConfig) -> Result<Arc<dyn RemoteSource>, SourceError> + Send + Sync,
{
    fn provider_id(&self) -> &str {
        self.provider_id
    }

    fn provider_name(&self) -> &str {
        self.provider_name
    }

    async fn create(&self, config: &ProviderConfig) -> Result<Arc<dyn RemoteSource>, SourceError> {
        (self.create_fn)(config)
    }
}

#[cfg(test)]
mod tests {
    // Tests would go here
}
