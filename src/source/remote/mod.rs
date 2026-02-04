//! Remote source module for fetching secrets from external secret managers.
//!
//! This module provides the infrastructure for integrating with remote secret
//! management services via external providers.
//!
//! ## External Providers
//!
//! Out-of-process binaries communicating via JSON-RPC 2.0 over stdio. This provides:
//! - Full process isolation
//! - Independent versioning and releases
//! - Users install only providers they need
//! - Community can create providers without touching core
//! - Security boundaries for credential handling
//!
//! External providers are managed via the `ExternalProviderAdapter` and `ProviderManager`.
//!
//! ## Available Providers
//!
//! - `ecolog-provider-doppler` - Doppler secrets manager
//!
//! Install providers to the configured `providers_path` directory.

mod traits;

// External provider infrastructure
pub mod discovery;
pub mod external;
pub mod manager;
pub mod protocol;
pub mod transport;

pub use traits::*;

// Re-export external provider types
pub use discovery::{DiscoveryConfig, ProviderBinaryInfo, ProviderDiscovery};
pub use protocol::{
    AuthField as ProtocolAuthField, AuthStatus as ProtocolAuthStatus,
    AuthenticateParams, AuthenticateResult, AuthFieldsResult, AuthStatusResult,
    ClientCapabilities, ClientInfo, ErrorCode, InitializeParams, InitializeResult,
    JsonRpcError, JsonRpcId, JsonRpcMessage, JsonRpcNotification, JsonRpcRequest,
    JsonRpcResponse, PingParams, PingResult, ProviderCapabilities, ProviderInfo,
    ScopeLevel as ProtocolScopeLevel, ScopeLevelsResult, ScopeOption as ProtocolScopeOption,
    ScopeOptionsParams, ScopeOptionsResult, ScopeSelection as ProtocolScopeSelection,
    Secret, SecretsFetchParams, SecretsFetchResult, SecretsGetParams, SecretsGetResult,
    SecretsSetParams, SecretsSetResult, SecretsDeleteParams, SecretsDeleteResult,
    SecretsChangedParams, LogLevel, LogParams, PROTOCOL_VERSION,
};
pub use transport::{StdioTransport, SyncStdioTransport, TransportError, spawn_provider};
pub use external::{ExternalProviderAdapter, ProviderState};
pub use manager::ProviderManager;
