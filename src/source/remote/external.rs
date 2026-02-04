//! External provider adapter.
//!
//! This module implements `ExternalProviderAdapter`, which spawns and manages
//! out-of-process provider binaries. It handles:
//!
//! - Process lifecycle (spawn, health check, restart, shutdown)
//! - JSON-RPC protocol communication
//! - AsyncEnvSource implementation
//! - Authentication state management
//! - Scope selection and secret caching
//!
//! ## Crash Recovery
//!
//! If the provider process crashes, the adapter uses exponential backoff:
//! | Attempt | Delay |
//! |---------|-------|
//! | 1 | 0s |
//! | 2 | 1s |
//! | 3 | 2s |
//! | 4 | 4s |
//! | 5 | 8s (then stop) |

use super::discovery::ProviderDiscovery;
use super::protocol::{
    self, methods, AuthField, AuthStatus, AuthenticateParams, AuthenticateResult,
    ClientCapabilities, ClientInfo, InitializeParams, InitializeResult, PingParams,
    ProviderCapabilities, ProviderInfo, ScopeLevel, ScopeLevelsResult, ScopeOption,
    ScopeOptionsParams, ScopeOptionsResult, ScopeSelection, Secret, SecretsFetchParams,
    SecretsFetchResult, PROTOCOL_VERSION,
};
use super::traits::RemoteSourceInfo;
use super::transport::{spawn_provider, StdioTransport};
use crate::config::ExternalProviderConfig;
use crate::error::SourceError;
use crate::source::traits::{
    AsyncEnvSource, Priority, SourceCapabilities, SourceId, SourceMetadata,
    SourceSnapshot, SourceType,
};
use crate::source::variable::{ParsedVariable, VariableSource};
use async_trait::async_trait;
use compact_str::CompactString;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Child;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Maximum number of restart attempts before giving up.
const MAX_RESTART_ATTEMPTS: u32 = 5;

/// Health check interval (used by ProviderManager for scheduling).
#[allow(dead_code)]
const HEALTH_CHECK_INTERVAL: Duration = Duration::from_secs(30);

/// Health check timeout.
const HEALTH_CHECK_TIMEOUT: Duration = Duration::from_secs(5);

/// Request timeout.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// External provider state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderState {
    /// Not spawned yet.
    NotStarted,
    /// Starting up.
    Starting,
    /// Running and healthy.
    Running,
    /// Not authenticated.
    NeedsAuth,
    /// Process crashed, will attempt restart.
    Crashed,
    /// Stopped (manually or after max restarts).
    Stopped,
}

/// External provider adapter.
///
/// Manages an out-of-process provider binary, handling lifecycle and
/// implementing `AsyncEnvSource` by delegating to the external process.
pub struct ExternalProviderAdapter {
    /// Provider ID (e.g., "doppler").
    provider_id: String,
    /// Path to the provider binary.
    binary_path: PathBuf,
    /// Provider configuration.
    config: ExternalProviderConfig,
    /// Source ID for this adapter.
    source_id: SourceId,
    /// Child process handle.
    process: RwLock<Option<Child>>,
    /// Transport for communication (wrapped in Arc for Send across await).
    transport: RwLock<Option<Arc<StdioTransport>>>,
    /// Provider state.
    state: RwLock<ProviderState>,
    /// Provider info from initialize response.
    provider_info: RwLock<Option<ProviderInfo>>,
    /// Provider capabilities.
    capabilities: RwLock<ProviderCapabilities>,
    /// Current auth status.
    auth_status: RwLock<AuthStatus>,
    /// Current scope selection.
    scope: RwLock<ScopeSelection>,
    /// Cached secrets.
    cached_secrets: RwLock<Option<SecretsFetchResult>>,
    /// Last successful refresh time.
    last_refreshed: RwLock<Option<Instant>>,
    /// Restart attempt count.
    restart_count: RwLock<u32>,
    /// Last error message.
    last_error: RwLock<Option<String>>,
    /// Last health check time.
    last_health_check: RwLock<Option<Instant>>,
}

impl ExternalProviderAdapter {
    /// Creates a new external provider adapter.
    pub fn new(
        provider_id: impl Into<String>,
        binary_path: impl Into<PathBuf>,
        config: ExternalProviderConfig,
    ) -> Self {
        let provider_id = provider_id.into();
        let source_id = SourceId::new(format!("external:{}", provider_id));

        Self {
            provider_id,
            binary_path: binary_path.into(),
            config,
            source_id,
            process: RwLock::new(None),
            transport: RwLock::new(None),
            state: RwLock::new(ProviderState::NotStarted),
            provider_info: RwLock::new(None),
            capabilities: RwLock::new(ProviderCapabilities::default()),
            auth_status: RwLock::new(AuthStatus::NotAuthenticated),
            scope: RwLock::new(ScopeSelection::default()),
            cached_secrets: RwLock::new(None),
            last_refreshed: RwLock::new(None),
            restart_count: RwLock::new(0),
            last_error: RwLock::new(None),
            last_health_check: RwLock::new(None),
        }
    }

    /// Creates an adapter by discovering the provider binary.
    pub fn discover(
        provider_id: &str,
        config: ExternalProviderConfig,
        discovery: &ProviderDiscovery,
    ) -> Result<Self, SourceError> {
        // Check for binary override first
        let binary_path = if let Some(ref path) = config.binary {
            path.clone()
        } else {
            discovery.find_provider(provider_id)?
        };

        Ok(Self::new(provider_id, binary_path, config))
    }

    /// Returns the provider ID.
    pub fn provider_id(&self) -> &str {
        &self.provider_id
    }

    /// Returns the current state.
    pub fn state(&self) -> ProviderState {
        *self.state.read()
    }

    /// Returns the provider info if available.
    pub fn provider_info(&self) -> Option<ProviderInfo> {
        self.provider_info.read().clone()
    }

    /// Returns the current auth status.
    pub fn auth_status(&self) -> AuthStatus {
        self.auth_status.read().clone()
    }

    /// Returns the current scope selection.
    pub fn scope(&self) -> ScopeSelection {
        self.scope.read().clone()
    }

    /// Returns the last error message.
    pub fn last_error(&self) -> Option<String> {
        self.last_error.read().clone()
    }

    /// Gets the transport Arc for use in async methods.
    /// This clones the Arc so the lock can be released before await.
    fn get_transport(&self) -> Result<Arc<StdioTransport>, SourceError> {
        self.transport
            .read()
            .as_ref()
            .cloned()
            .ok_or_else(|| SourceError::Connection {
                provider: self.provider_id.clone(),
                reason: "Not connected".into(),
            })
    }

    /// Spawns the provider process and initializes it.
    pub async fn spawn(&self) -> Result<(), SourceError> {
        // Check if already running
        if *self.state.read() == ProviderState::Running {
            return Ok(());
        }

        *self.state.write() = ProviderState::Starting;

        // Spawn the process
        let mut child = spawn_provider(&self.binary_path).map_err(|e| SourceError::Remote {
            provider: self.provider_id.clone(),
            reason: e.to_string(),
        })?;

        // Create transport
        let transport = StdioTransport::new(&mut child).map_err(|e| SourceError::Remote {
            provider: self.provider_id.clone(),
            reason: e.to_string(),
        })?;

        *self.process.write() = Some(child);
        *self.transport.write() = Some(Arc::new(transport));

        // Initialize the provider
        self.initialize().await?;

        // Check auth status
        self.refresh_auth_status().await?;

        if self.auth_status.read().is_authenticated() {
            *self.state.write() = ProviderState::Running;
        } else {
            *self.state.write() = ProviderState::NeedsAuth;
        }

        *self.restart_count.write() = 0;
        *self.last_error.write() = None;

        Ok(())
    }

    /// Initializes the provider with capability negotiation.
    async fn initialize(&self) -> Result<InitializeResult, SourceError> {
        let transport = self.get_transport()?;

        let params = InitializeParams {
            protocol_version: PROTOCOL_VERSION.to_string(),
            client_info: ClientInfo {
                name: "ecolog-lsp".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            capabilities: ClientCapabilities {
                notifications: protocol::NotificationCapabilities {
                    secrets_changed: true,
                },
                credential_storage: true,
            },
            config: self.config.settings.clone(),
        };

        let response = transport
            .request_with_timeout(methods::INITIALIZE, params, REQUEST_TIMEOUT)
            .await
            .map_err(|e| SourceError::Connection {
                provider: self.provider_id.clone(),
                reason: e.to_string(),
            })?;

        let result: InitializeResult = response.into_result().map_err(|e| SourceError::Remote {
            provider: self.provider_id.clone(),
            reason: e.to_string(),
        })?;

        // Store provider info and capabilities
        *self.provider_info.write() = Some(result.provider_info.clone());
        *self.capabilities.write() = result.capabilities.clone();

        // Send initialized notification
        transport
            .notify(methods::INITIALIZED, serde_json::json!({}))
            .map_err(|e| SourceError::Connection {
                provider: self.provider_id.clone(),
                reason: e.to_string(),
            })?;

        Ok(result)
    }

    /// Refreshes the auth status from the provider.
    async fn refresh_auth_status(&self) -> Result<AuthStatus, SourceError> {
        let transport = self.get_transport()?;

        let response = transport
            .request_with_timeout(methods::AUTH_STATUS, serde_json::json!({}), REQUEST_TIMEOUT)
            .await
            .map_err(|e| SourceError::Connection {
                provider: self.provider_id.clone(),
                reason: e.to_string(),
            })?;

        let result: protocol::AuthStatusResult =
            response.into_result().map_err(|e| SourceError::Remote {
                provider: self.provider_id.clone(),
                reason: e.to_string(),
            })?;

        *self.auth_status.write() = result.status.clone();
        Ok(result.status)
    }

    /// Returns the authentication fields required by this provider.
    pub async fn auth_fields(&self) -> Result<Vec<AuthField>, SourceError> {
        self.ensure_running().await?;

        let transport = self.get_transport()?;

        let response = transport
            .request_with_timeout(methods::AUTH_FIELDS, serde_json::json!({}), REQUEST_TIMEOUT)
            .await
            .map_err(|e| SourceError::Connection {
                provider: self.provider_id.clone(),
                reason: e.to_string(),
            })?;

        let result: protocol::AuthFieldsResult =
            response.into_result().map_err(|e| SourceError::Remote {
                provider: self.provider_id.clone(),
                reason: e.to_string(),
            })?;

        Ok(result.fields)
    }

    /// Authenticates with the provider.
    pub async fn authenticate(
        &self,
        credentials: HashMap<String, String>,
    ) -> Result<AuthStatus, SourceError> {
        self.ensure_running().await?;

        let transport = self.get_transport()?;

        let params = AuthenticateParams { credentials };

        let response = transport
            .request_with_timeout(methods::AUTH_AUTHENTICATE, params, REQUEST_TIMEOUT)
            .await
            .map_err(|e| SourceError::Connection {
                provider: self.provider_id.clone(),
                reason: e.to_string(),
            })?;

        let result: AuthenticateResult =
            response.into_result().map_err(|_e| SourceError::Authentication {
                source_name: self.provider_id.clone(),
            })?;

        *self.auth_status.write() = result.status.clone();

        if result.status.is_authenticated() {
            *self.state.write() = ProviderState::Running;
        }

        Ok(result.status)
    }

    /// Returns the scope levels for this provider.
    pub async fn scope_levels(&self) -> Result<Vec<ScopeLevel>, SourceError> {
        self.ensure_running().await?;

        let transport = self.get_transport()?;

        let response = transport
            .request_with_timeout(methods::SCOPE_LEVELS, serde_json::json!({}), REQUEST_TIMEOUT)
            .await
            .map_err(|e| SourceError::Connection {
                provider: self.provider_id.clone(),
                reason: e.to_string(),
            })?;

        let result: ScopeLevelsResult =
            response.into_result().map_err(|e| SourceError::Remote {
                provider: self.provider_id.clone(),
                reason: e.to_string(),
            })?;

        Ok(result.levels)
    }

    /// Lists available options at the given scope level.
    pub async fn list_scope_options(
        &self,
        level: &str,
        parent: &ScopeSelection,
    ) -> Result<Vec<ScopeOption>, SourceError> {
        self.ensure_running().await?;

        let transport = self.get_transport()?;

        let params = ScopeOptionsParams {
            level: level.to_string(),
            parent: parent.selections.clone(),
        };

        let response = transport
            .request_with_timeout(methods::SCOPE_OPTIONS, params, REQUEST_TIMEOUT)
            .await
            .map_err(|e| SourceError::Connection {
                provider: self.provider_id.clone(),
                reason: e.to_string(),
            })?;

        let result: ScopeOptionsResult =
            response.into_result().map_err(|e| SourceError::Remote {
                provider: self.provider_id.clone(),
                reason: e.to_string(),
            })?;

        Ok(result.options)
    }

    /// Sets the scope selection.
    pub fn set_scope(&self, scope: ScopeSelection) {
        *self.scope.write() = scope;
        // Clear cache when scope changes
        *self.cached_secrets.write() = None;
    }

    /// Fetches secrets for the current scope.
    pub async fn fetch_secrets(&self) -> Result<Vec<Secret>, SourceError> {
        self.ensure_running().await?;
        self.ensure_authenticated()?;

        let scope = self.scope.read().clone();
        let transport = self.get_transport()?;

        let params = SecretsFetchParams { scope };

        let response = transport
            .request_with_timeout(methods::SECRETS_FETCH, params, REQUEST_TIMEOUT)
            .await
            .map_err(|e| SourceError::Connection {
                provider: self.provider_id.clone(),
                reason: e.to_string(),
            })?;

        let result: SecretsFetchResult =
            response.into_result().map_err(|e| SourceError::Remote {
                provider: self.provider_id.clone(),
                reason: e.to_string(),
            })?;

        // Cache the result
        *self.cached_secrets.write() = Some(result.clone());
        *self.last_refreshed.write() = Some(Instant::now());

        Ok(result.secrets)
    }

    /// Performs a health check on the provider.
    pub async fn health_check(&self) -> Result<bool, SourceError> {
        if *self.state.read() != ProviderState::Running {
            return Ok(false);
        }

        let transport = match self.get_transport() {
            Ok(t) => t,
            Err(_) => return Ok(false),
        };

        let result = transport
            .request_with_timeout(methods::PING, PingParams {}, HEALTH_CHECK_TIMEOUT)
            .await;

        *self.last_health_check.write() = Some(Instant::now());

        match result {
            Ok(_) => Ok(true),
            Err(_) => {
                *self.state.write() = ProviderState::Crashed;
                Ok(false)
            }
        }
    }

    /// Attempts to restart the provider if it crashed.
    pub async fn restart_if_needed(&self) -> Result<bool, SourceError> {
        let state = *self.state.read();
        if state != ProviderState::Crashed && state != ProviderState::Stopped {
            return Ok(false);
        }

        let restart_count = *self.restart_count.read();
        if restart_count >= MAX_RESTART_ATTEMPTS {
            tracing::warn!(
                "Provider {} exceeded max restart attempts",
                self.provider_id
            );
            return Ok(false);
        }

        // Exponential backoff
        let delay = Duration::from_secs(1 << restart_count.min(3));
        tokio::time::sleep(delay).await;

        *self.restart_count.write() = restart_count + 1;

        tracing::info!(
            "Restarting provider {} (attempt {})",
            self.provider_id,
            restart_count + 1
        );

        // Clean up old process
        self.cleanup().await;

        // Spawn new process
        match self.spawn().await {
            Ok(()) => Ok(true),
            Err(e) => {
                *self.last_error.write() = Some(e.to_string());
                *self.state.write() = ProviderState::Crashed;
                Err(e)
            }
        }
    }

    /// Gracefully shuts down the provider.
    pub async fn shutdown(&self) -> Result<(), SourceError> {
        let state = *self.state.read();
        if state == ProviderState::NotStarted || state == ProviderState::Stopped {
            return Ok(());
        }

        *self.state.write() = ProviderState::Stopped;

        // Send shutdown request - get transport first, then await
        if let Ok(transport) = self.get_transport() {
            let _ = transport
                .request_with_timeout(
                    methods::SHUTDOWN,
                    serde_json::json!({}),
                    Duration::from_secs(5),
                )
                .await;

            // Send exit notification
            let _ = transport.notify(methods::EXIT, serde_json::json!({}));
        }

        // Wait briefly then cleanup
        tokio::time::sleep(Duration::from_millis(100)).await;
        self.cleanup().await;

        Ok(())
    }

    /// Cleans up the process and transport.
    ///
    /// Note: We don't call transport.shutdown() here because:
    /// 1. We've already sent SHUTDOWN/EXIT messages in shutdown()
    /// 2. StdioTransport::shutdown requires &mut self which isn't compatible with Arc
    /// 3. Dropping the Arc will cause the transport to clean up its resources
    async fn cleanup(&self) {
        // Drop the transport - this releases our reference and lets it clean up
        let _ = self.transport.write().take();

        if let Some(mut process) = self.process.write().take() {
            // Try graceful termination first
            let _ = process.kill();
            let _ = process.wait();
        }
    }

    /// Ensures the provider is running, spawning if necessary.
    async fn ensure_running(&self) -> Result<(), SourceError> {
        let state = *self.state.read();
        match state {
            ProviderState::Running | ProviderState::NeedsAuth => Ok(()),
            ProviderState::NotStarted | ProviderState::Starting => self.spawn().await,
            ProviderState::Crashed => {
                self.restart_if_needed().await?;
                Ok(())
            }
            ProviderState::Stopped => Err(SourceError::Remote {
                provider: self.provider_id.clone(),
                reason: "Provider is stopped".into(),
            }),
        }
    }

    /// Ensures the provider is authenticated.
    fn ensure_authenticated(&self) -> Result<(), SourceError> {
        if !self.auth_status.read().is_authenticated() {
            return Err(SourceError::Authentication {
                source_name: self.provider_id.clone(),
            });
        }
        Ok(())
    }

    /// Returns summary info for UI display.
    pub fn info(&self) -> RemoteSourceInfo {
        let provider_info = self.provider_info.read();
        let (id, display_name, short_name) = if let Some(ref info) = *provider_info {
            (
                CompactString::from(&info.id),
                CompactString::from(&info.name),
                info.short_name
                    .as_ref()
                    .map(|s| CompactString::from(s.as_str()))
                    .unwrap_or_else(|| CompactString::from(&info.id)),
            )
        } else {
            (
                CompactString::from(&self.provider_id),
                CompactString::from(&self.provider_id),
                CompactString::from(&self.provider_id),
            )
        };

        let auth_status = self.auth_status.read().clone().into();
        let scope = self.scope.read().clone().into();
        let secret_count = self
            .cached_secrets
            .read()
            .as_ref()
            .map(|s| s.secrets.len())
            .unwrap_or(0);
        let last_refreshed = self.last_refreshed.read().map(|i| {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64
                - i.elapsed().as_millis() as u64
        });
        let loading = *self.state.read() == ProviderState::Starting;
        let last_error = self.last_error.read().clone().map(CompactString::from);

        RemoteSourceInfo {
            id,
            display_name,
            short_name,
            auth_status,
            scope,
            secret_count,
            last_refreshed,
            loading,
            last_error,
        }
    }

    /// Invalidates the cached secrets.
    pub fn invalidate_cache(&self) {
        *self.cached_secrets.write() = None;
    }
}

#[async_trait]
impl AsyncEnvSource for ExternalProviderAdapter {
    fn id(&self) -> &SourceId {
        &self.source_id
    }

    fn source_type(&self) -> SourceType {
        SourceType::Remote
    }

    fn priority(&self) -> Priority {
        Priority::REMOTE
    }

    fn capabilities(&self) -> SourceCapabilities {
        let caps = self.capabilities.read();
        let mut result = SourceCapabilities::ASYNC_ONLY | SourceCapabilities::SECRETS;

        if caps.secrets.read {
            result |= SourceCapabilities::READ;
        }
        if caps.secrets.write {
            result |= SourceCapabilities::WRITE;
        }
        if caps.secrets.watch {
            result |= SourceCapabilities::WATCH;
        }

        result
    }

    async fn load(&self) -> Result<SourceSnapshot, SourceError> {
        // Clone cached data immediately to release the RwLock guard before any await.
        // This is necessary because parking_lot guards contain non-Send raw pointers.
        let cached_data = self.cached_secrets.read().clone();

        if let Some(cached) = cached_data {
            let variables: Vec<ParsedVariable> = cached
                .secrets
                .iter()
                .map(|s| ParsedVariable {
                    key: CompactString::from(&s.key),
                    raw_value: CompactString::from(&s.value),
                    source: VariableSource::Remote {
                        provider: CompactString::from(&self.provider_id),
                        path: cached.scope_path.clone(),
                    },
                    description: s.description.as_ref().map(|d| CompactString::from(d.as_str())),
                    is_commented: false,
                })
                .collect();

            return Ok(SourceSnapshot {
                source_id: self.source_id.clone(),
                variables: Arc::from(variables),
                timestamp: Instant::now(),
                version: None,
            });
        }

        // Fetch secrets
        let secrets = self.fetch_secrets().await?;

        // Get scope_path from the freshly cached data
        let scope_path = self.cached_secrets.read().as_ref().and_then(|c| c.scope_path.clone());

        let variables: Vec<ParsedVariable> = secrets
            .iter()
            .map(|s| ParsedVariable {
                key: CompactString::from(&s.key),
                raw_value: CompactString::from(&s.value),
                source: VariableSource::Remote {
                    provider: CompactString::from(&self.provider_id),
                    path: scope_path.clone(),
                },
                description: s.description.as_ref().map(|d| CompactString::from(d.as_str())),
                is_commented: false,
            })
            .collect();

        Ok(SourceSnapshot {
            source_id: self.source_id.clone(),
            variables: Arc::from(variables),
            timestamp: Instant::now(),
            version: None,
        })
    }

    async fn refresh(&self) -> Result<bool, SourceError> {
        self.invalidate_cache();
        self.load().await?;
        Ok(true)
    }

    fn metadata(&self) -> SourceMetadata {
        let info = self.provider_info.read();
        SourceMetadata {
            display_name: info
                .as_ref()
                .map(|i| CompactString::from(&i.name)),
            description: info
                .as_ref()
                .and_then(|i| i.description.as_ref().map(|d| CompactString::from(d.as_str()))),
            last_refreshed: *self.last_refreshed.read(),
            error_count: 0,
        }
    }
}

impl std::fmt::Debug for ExternalProviderAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExternalProviderAdapter")
            .field("provider_id", &self.provider_id)
            .field("state", &*self.state.read())
            .field("auth_status", &*self.auth_status.read())
            .field("scope", &*self.scope.read())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_state_default() {
        let config = ExternalProviderConfig::default();
        let adapter = ExternalProviderAdapter::new("test", "/path/to/provider", config);

        assert_eq!(adapter.state(), ProviderState::NotStarted);
        assert_eq!(adapter.provider_id(), "test");
    }

    #[test]
    fn test_source_id_format() {
        let config = ExternalProviderConfig::default();
        let adapter = ExternalProviderAdapter::new("doppler", "/path/to/provider", config);

        assert_eq!(adapter.id().as_str(), "external:doppler");
    }
}
