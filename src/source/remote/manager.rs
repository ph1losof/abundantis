//! Provider manager for coordinating multiple external providers.
//!
//! The `ProviderManager` is the central coordinator for all external providers.
//! It handles:
//!
//! - Provider discovery and registration
//! - Lazy/eager spawning based on configuration
//! - Health check scheduling
//! - Crash recovery with exponential backoff
//! - Graceful shutdown of all providers
//!
//! ## Usage
//!
//! ```ignore
//! let manager = ProviderManager::new(config, discovery);
//!
//! // Register providers from configuration
//! manager.register_from_config().await?;
//!
//! // Get a specific provider
//! if let Some(doppler) = manager.get("doppler") {
//!     doppler.authenticate(credentials).await?;
//!     let secrets = doppler.fetch_secrets().await?;
//! }
//!
//! // Shutdown all providers
//! manager.shutdown_all().await;
//! ```

use super::discovery::{DiscoveryConfig, ProviderDiscovery};
use super::external::{ExternalProviderAdapter, ProviderState};
use super::traits::RemoteSourceInfo;
use crate::config::{ExternalProviderConfig, ProvidersConfig, SpawnStrategy};
use crate::error::SourceError;
use crate::source::traits::{AsyncEnvSource, SourceId};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

/// Health check interval.
const HEALTH_CHECK_INTERVAL: Duration = Duration::from_secs(30);

/// Provider manager state.
pub struct ProviderManager {
    /// Provider configuration.
    config: ProvidersConfig,
    /// Provider discovery.
    discovery: ProviderDiscovery,
    /// Registered providers by ID.
    providers: RwLock<HashMap<String, Arc<ExternalProviderAdapter>>>,
    /// Whether the manager is running.
    running: RwLock<bool>,
    /// Health check task handle.
    health_check_handle: RwLock<Option<tokio::task::JoinHandle<()>>>,
}

impl ProviderManager {
    /// Creates a new provider manager.
    pub fn new(config: ProvidersConfig) -> Self {
        let discovery_config = DiscoveryConfig {
            providers_path: config.path.clone(),
            binary_overrides: config
                .providers
                .iter()
                .filter_map(|(id, c)| c.binary.as_ref().map(|b| (id.clone(), b.clone())))
                .collect(),
        };

        Self {
            config,
            discovery: ProviderDiscovery::new(discovery_config),
            providers: RwLock::new(HashMap::new()),
            running: RwLock::new(false),
            health_check_handle: RwLock::new(None),
        }
    }

    /// Creates a provider manager with custom discovery.
    pub fn with_discovery(config: ProvidersConfig, discovery: ProviderDiscovery) -> Self {
        Self {
            config,
            discovery,
            providers: RwLock::new(HashMap::new()),
            running: RwLock::new(false),
            health_check_handle: RwLock::new(None),
        }
    }

    /// Starts the provider manager.
    ///
    /// This registers all enabled providers and spawns eager providers.
    pub async fn start(&self) -> Result<(), SourceError> {
        *self.running.write() = true;

        // Register all enabled providers
        self.register_from_config().await?;

        // Start health check task
        self.start_health_checks();

        Ok(())
    }

    /// Registers providers based on configuration.
    pub async fn register_from_config(&self) -> Result<(), SourceError> {
        for (provider_id, provider_config) in &self.config.providers {
            if !provider_config.enabled {
                continue;
            }

            match self.register(provider_id, provider_config.clone()).await {
                Ok(_) => {
                    tracing::info!("Registered provider: {}", provider_id);
                }
                Err(e) => {
                    tracing::warn!("Failed to register provider {}: {}", provider_id, e);
                    // Continue with other providers
                }
            }
        }

        Ok(())
    }

    /// Registers a single provider.
    pub async fn register(
        &self,
        provider_id: &str,
        config: ExternalProviderConfig,
    ) -> Result<Arc<ExternalProviderAdapter>, SourceError> {
        // Create adapter
        let adapter =
            ExternalProviderAdapter::discover(provider_id, config.clone(), &self.discovery)?;
        let adapter = Arc::new(adapter);

        // Spawn if eager
        if config.spawn == SpawnStrategy::Eager {
            adapter.spawn().await?;
        }

        // Store
        self.providers
            .write()
            .insert(provider_id.to_string(), Arc::clone(&adapter));

        Ok(adapter)
    }

    /// Gets a provider by ID.
    pub fn get(&self, provider_id: &str) -> Option<Arc<ExternalProviderAdapter>> {
        self.providers.read().get(provider_id).cloned()
    }

    /// Gets a provider by ID, spawning if not yet started.
    pub async fn get_or_spawn(
        &self,
        provider_id: &str,
    ) -> Result<Arc<ExternalProviderAdapter>, SourceError> {
        let adapter = self.providers.read().get(provider_id).cloned();

        match adapter {
            Some(adapter) => {
                // Spawn if not started
                if adapter.state() == ProviderState::NotStarted {
                    adapter.spawn().await?;
                }
                Ok(adapter)
            }
            None => {
                // Check if configured
                if let Some(config) = self.config.providers.get(provider_id) {
                    if config.enabled {
                        // Register and spawn
                        let adapter = self.register(provider_id, config.clone()).await?;
                        adapter.spawn().await?;
                        return Ok(adapter);
                    }
                }
                Err(SourceError::UnknownProvider {
                    provider: provider_id.into(),
                })
            }
        }
    }

    /// Lists all registered providers.
    pub fn list(&self) -> Vec<Arc<ExternalProviderAdapter>> {
        self.providers.read().values().cloned().collect()
    }

    /// Lists all provider IDs.
    pub fn provider_ids(&self) -> Vec<String> {
        self.providers.read().keys().cloned().collect()
    }

    /// Lists providers with their info for UI display.
    pub fn list_with_info(&self) -> Vec<RemoteSourceInfo> {
        self.providers
            .read()
            .values()
            .map(|p| p.info())
            .collect()
    }

    /// Returns provider info by ID.
    pub fn info(&self, provider_id: &str) -> Option<RemoteSourceInfo> {
        self.providers.read().get(provider_id).map(|p| p.info())
    }

    /// Checks if a provider is registered.
    pub fn has_provider(&self, provider_id: &str) -> bool {
        self.providers.read().contains_key(provider_id)
    }

    /// Checks if a provider is running.
    pub fn is_running(&self, provider_id: &str) -> bool {
        self.providers
            .read()
            .get(provider_id)
            .map(|p| p.state() == ProviderState::Running)
            .unwrap_or(false)
    }

    /// Checks if a provider is authenticated.
    pub fn is_authenticated(&self, provider_id: &str) -> bool {
        self.providers
            .read()
            .get(provider_id)
            .map(|p| p.auth_status().is_authenticated())
            .unwrap_or(false)
    }

    /// Unregisters a provider.
    pub async fn unregister(&self, provider_id: &str) -> Result<(), SourceError> {
        // Extract the adapter while holding the lock, then drop the lock before awaiting
        let adapter = self.providers.write().remove(provider_id);
        if let Some(adapter) = adapter {
            adapter.shutdown().await?;
        }
        Ok(())
    }

    /// Refreshes all running providers.
    pub async fn refresh_all(&self) -> Result<(), SourceError> {
        let providers: Vec<_> = self.providers.read().values().cloned().collect();

        for provider in providers {
            if provider.state() == ProviderState::Running {
                if let Err(e) = provider.refresh().await {
                    tracing::warn!(
                        "Failed to refresh provider {}: {}",
                        provider.provider_id(),
                        e
                    );
                }
            }
        }

        Ok(())
    }

    /// Shuts down all providers.
    pub async fn shutdown_all(&self) {
        *self.running.write() = false;

        // Stop health check task
        if let Some(handle) = self.health_check_handle.write().take() {
            handle.abort();
        }

        // Shutdown all providers
        let providers: Vec<_> = self.providers.write().drain().collect();

        for (id, provider) in providers {
            tracing::info!("Shutting down provider: {}", id);
            if let Err(e) = provider.shutdown().await {
                tracing::warn!("Failed to shutdown provider {}: {}", id, e);
            }
        }
    }

    /// Returns the providers path.
    pub fn providers_path(&self) -> &std::path::Path {
        &self.config.path
    }

    /// Lists installed provider binaries.
    pub fn list_installed(&self) -> Vec<super::discovery::ProviderBinaryInfo> {
        self.discovery.list_installed()
    }

    /// Checks if a provider binary is installed.
    pub fn is_installed(&self, provider_id: &str) -> bool {
        self.discovery.is_installed(provider_id)
    }

    /// Returns the enabled provider IDs from configuration.
    pub fn enabled_provider_ids(&self) -> Vec<String> {
        self.config
            .providers
            .iter()
            .filter(|(_, c)| c.enabled)
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Starts the health check background task.
    fn start_health_checks(&self) {
        let providers = Arc::new(self.providers.read().clone());
        let running = Arc::new(RwLock::new(true));

        let running_clone = Arc::clone(&running);
        let handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(HEALTH_CHECK_INTERVAL);

            loop {
                interval.tick().await;

                if !*running_clone.read() {
                    break;
                }

                for (_id, provider) in providers.iter() {
                    if provider.state() == ProviderState::Running {
                        match provider.health_check().await {
                            Ok(healthy) => {
                                if !healthy {
                                    tracing::warn!(
                                        "Provider {} health check failed, attempting restart",
                                        provider.provider_id()
                                    );
                                    if let Err(e) = provider.restart_if_needed().await {
                                        tracing::error!(
                                            "Failed to restart provider {}: {}",
                                            provider.provider_id(),
                                            e
                                        );
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::warn!(
                                    "Provider {} health check error: {}",
                                    provider.provider_id(),
                                    e
                                );
                            }
                        }
                    }
                }
            }
        });

        *self.health_check_handle.write() = Some(handle);
    }

    /// Gets all providers as async env sources for registration.
    pub fn as_async_sources(&self) -> Vec<Arc<dyn AsyncEnvSource>> {
        self.providers
            .read()
            .values()
            .map(|p| Arc::clone(p) as Arc<dyn AsyncEnvSource>)
            .collect()
    }

    /// Gets provider source IDs for filtering.
    pub fn source_ids(&self) -> Vec<SourceId> {
        self.providers
            .read()
            .values()
            .map(|p| p.id().clone())
            .collect()
    }
}

impl Drop for ProviderManager {
    fn drop(&mut self) {
        // Mark as not running to stop health checks
        *self.running.write() = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_provider_manager_creation() {
        let config = ProvidersConfig {
            path: PathBuf::from("/tmp/providers"),
            providers: HashMap::new(),
        };

        let manager = ProviderManager::new(config);
        assert!(manager.list().is_empty());
    }

    #[test]
    fn test_enabled_provider_ids() {
        let mut providers = HashMap::new();
        providers.insert(
            "doppler".to_string(),
            ExternalProviderConfig {
                enabled: true,
                ..Default::default()
            },
        );
        providers.insert(
            "aws".to_string(),
            ExternalProviderConfig {
                enabled: false,
                ..Default::default()
            },
        );
        providers.insert(
            "vault".to_string(),
            ExternalProviderConfig {
                enabled: true,
                ..Default::default()
            },
        );

        let config = ProvidersConfig {
            path: PathBuf::from("/tmp/providers"),
            providers,
        };

        let manager = ProviderManager::new(config);
        let enabled = manager.enabled_provider_ids();

        assert_eq!(enabled.len(), 2);
        assert!(enabled.contains(&"doppler".to_string()));
        assert!(enabled.contains(&"vault".to_string()));
        assert!(!enabled.contains(&"aws".to_string()));
    }
}
