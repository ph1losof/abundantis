//! JSON-RPC 2.0 protocol types for external provider communication.
//!
//! This module defines all message types used to communicate with out-of-process
//! provider binaries over stdio. The protocol follows JSON-RPC 2.0 specification
//! with provider-specific methods for authentication, scope navigation, and secret
//! management.
//!
//! ## Protocol Version
//!
//! The protocol version uses `YYYY-MM-DD` format (e.g., "2025-01-01").
//! Providers should support current + previous version for backward compatibility.
//!
//! ## Method Categories
//!
//! - **Lifecycle**: `initialize`, `initialized`, `shutdown`, `exit`, `ping`
//! - **Authentication**: `auth/status`, `auth/fields`, `auth/authenticate`, `auth/revoke`
//! - **Scope**: `scope/levels`, `scope/options`
//! - **Secrets**: `secrets/fetch`, `secrets/get`, `secrets/set`, `secrets/delete`
//!
//! ## Notifications
//!
//! - `secrets/changed`: Provider notifies when secrets change (push-based invalidation)
//! - `log`: Provider sends log messages to LSP for debugging

use compact_str::CompactString;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Current protocol version.
pub const PROTOCOL_VERSION: &str = "2025-01-01";

// =============================================================================
// JSON-RPC 2.0 Base Types
// =============================================================================

/// JSON-RPC 2.0 request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: JsonRpcId,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

impl JsonRpcRequest {
    pub fn new(id: impl Into<JsonRpcId>, method: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id: id.into(),
            method: method.into(),
            params: None,
        }
    }

    pub fn with_params<T: Serialize>(mut self, params: T) -> Self {
        self.params = Some(serde_json::to_value(params).expect("params must be serializable"));
        self
    }
}

/// JSON-RPC 2.0 response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: JsonRpcId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

impl JsonRpcResponse {
    pub fn success(id: JsonRpcId, result: impl Serialize) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: Some(serde_json::to_value(result).expect("result must be serializable")),
            error: None,
        }
    }

    pub fn error(id: JsonRpcId, error: JsonRpcError) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: None,
            error: Some(error),
        }
    }

    pub fn is_success(&self) -> bool {
        self.error.is_none()
    }

    /// Extract the result, returning an error if the response is an error.
    pub fn into_result<T: for<'de> Deserialize<'de>>(self) -> Result<T, JsonRpcError> {
        if let Some(error) = self.error {
            return Err(error);
        }
        let value = self.result.ok_or_else(|| JsonRpcError {
            code: ErrorCode::InternalError as i32,
            message: "No result in response".into(),
            data: None,
        })?;
        serde_json::from_value(value).map_err(|e| JsonRpcError {
            code: ErrorCode::ParseError as i32,
            message: format!("Failed to deserialize result: {}", e),
            data: None,
        })
    }
}

/// JSON-RPC 2.0 notification (no response expected).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

impl JsonRpcNotification {
    pub fn new(method: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            method: method.into(),
            params: None,
        }
    }

    pub fn with_params<T: Serialize>(mut self, params: T) -> Self {
        self.params = Some(serde_json::to_value(params).expect("params must be serializable"));
        self
    }
}

/// JSON-RPC ID (can be number, string, or null).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum JsonRpcId {
    Number(i64),
    String(String),
    Null,
}

impl From<i64> for JsonRpcId {
    fn from(n: i64) -> Self {
        JsonRpcId::Number(n)
    }
}

impl From<String> for JsonRpcId {
    fn from(s: String) -> Self {
        JsonRpcId::String(s)
    }
}

impl From<&str> for JsonRpcId {
    fn from(s: &str) -> Self {
        JsonRpcId::String(s.to_string())
    }
}

/// JSON-RPC 2.0 error object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl std::fmt::Display for JsonRpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code, self.message)
    }
}

impl std::error::Error for JsonRpcError {}

/// Standard and provider-specific error codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ErrorCode {
    // Standard JSON-RPC errors
    ParseError = -32700,
    InvalidRequest = -32600,
    MethodNotFound = -32601,
    InvalidParams = -32602,
    InternalError = -32603,

    // Provider-specific errors (-32000 to -32099)
    NotAuthenticated = -32000,
    AuthenticationFailed = -32001,
    InvalidScope = -32002,
    RateLimited = -32003,
    NetworkError = -32004,
    PermissionDenied = -32005,
    SecretNotFound = -32006,
    UnsupportedOperation = -32007,
}

impl ErrorCode {
    pub fn from_i32(code: i32) -> Option<ErrorCode> {
        match code {
            -32700 => Some(ErrorCode::ParseError),
            -32600 => Some(ErrorCode::InvalidRequest),
            -32601 => Some(ErrorCode::MethodNotFound),
            -32602 => Some(ErrorCode::InvalidParams),
            -32603 => Some(ErrorCode::InternalError),
            -32000 => Some(ErrorCode::NotAuthenticated),
            -32001 => Some(ErrorCode::AuthenticationFailed),
            -32002 => Some(ErrorCode::InvalidScope),
            -32003 => Some(ErrorCode::RateLimited),
            -32004 => Some(ErrorCode::NetworkError),
            -32005 => Some(ErrorCode::PermissionDenied),
            -32006 => Some(ErrorCode::SecretNotFound),
            -32007 => Some(ErrorCode::UnsupportedOperation),
            _ => None,
        }
    }
}

impl JsonRpcError {
    pub fn not_authenticated(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::NotAuthenticated as i32,
            message: message.into(),
            data: None,
        }
    }

    pub fn authentication_failed(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::AuthenticationFailed as i32,
            message: message.into(),
            data: None,
        }
    }

    pub fn invalid_scope(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::InvalidScope as i32,
            message: message.into(),
            data: None,
        }
    }

    pub fn rate_limited(retry_after_secs: u64) -> Self {
        Self {
            code: ErrorCode::RateLimited as i32,
            message: format!("Rate limited. Retry after {} seconds", retry_after_secs),
            data: Some(serde_json::json!({ "retry_after_secs": retry_after_secs })),
        }
    }

    pub fn network_error(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::NetworkError as i32,
            message: message.into(),
            data: None,
        }
    }

    pub fn permission_denied(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::PermissionDenied as i32,
            message: message.into(),
            data: None,
        }
    }

    pub fn method_not_found(method: &str) -> Self {
        Self {
            code: ErrorCode::MethodNotFound as i32,
            message: format!("Method not found: {}", method),
            data: None,
        }
    }

    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::InvalidParams as i32,
            message: message.into(),
            data: None,
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self {
            code: ErrorCode::InternalError as i32,
            message: message.into(),
            data: None,
        }
    }
}

// =============================================================================
// Message Wrapper (for parsing incoming messages)
// =============================================================================

/// Wrapper for incoming JSON-RPC messages.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum JsonRpcMessage {
    Request(JsonRpcRequest),
    Response(JsonRpcResponse),
    Notification(JsonRpcNotification),
}

impl JsonRpcMessage {
    /// Try to parse a JSON-RPC message from a string.
    pub fn parse(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }
}

// =============================================================================
// Lifecycle Methods
// =============================================================================

/// Client info sent during initialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientInfo {
    pub name: String,
    pub version: String,
}

/// Client capabilities sent during initialization.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientCapabilities {
    /// Whether client supports secrets/changed notifications.
    #[serde(default)]
    pub notifications: NotificationCapabilities,
    /// Whether client can store credentials securely.
    #[serde(default)]
    pub credential_storage: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationCapabilities {
    #[serde(default)]
    pub secrets_changed: bool,
}

/// Initialize request parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    /// Protocol version the client speaks.
    pub protocol_version: String,
    /// Information about the client (LSP).
    pub client_info: ClientInfo,
    /// Client capabilities.
    #[serde(default)]
    pub capabilities: ClientCapabilities,
    /// Provider-specific configuration from ecolog.toml.
    #[serde(default)]
    pub config: HashMap<String, serde_json::Value>,
}

/// Provider info returned during initialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderInfo {
    /// Unique provider identifier (e.g., "doppler", "aws").
    pub id: String,
    /// Human-readable name (e.g., "Doppler", "AWS Secrets Manager").
    pub name: String,
    /// Provider version.
    pub version: String,
    /// Short name for UI display (e.g., "DPL", "AWS").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub short_name: Option<String>,
    /// Provider description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Documentation URL.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub docs_url: Option<String>,
}

/// Provider capabilities returned during initialization.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCapabilities {
    /// Secret operation capabilities.
    pub secrets: SecretsCapability,
    /// Supported authentication methods.
    #[serde(default)]
    pub authentication: AuthenticationCapability,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretsCapability {
    /// Provider can read secrets.
    #[serde(default)]
    pub read: bool,
    /// Provider can write secrets.
    #[serde(default)]
    pub write: bool,
    /// Provider supports push notifications for changes.
    #[serde(default)]
    pub watch: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthenticationCapability {
    /// Supported auth methods (e.g., ["token", "oauth"]).
    #[serde(default)]
    pub methods: Vec<String>,
}

/// Initialize response result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeResult {
    /// Protocol version the provider will use.
    pub protocol_version: String,
    /// Provider information.
    pub provider_info: ProviderInfo,
    /// Provider capabilities.
    pub capabilities: ProviderCapabilities,
}

/// Ping request/response for health checks.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PingParams {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PingResult {
    /// Optional status message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
}

// =============================================================================
// Authentication Methods
// =============================================================================

/// A field required for authentication.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthField {
    /// Field name/key (e.g., "token", "client_id").
    pub name: String,
    /// Human-readable label (e.g., "API Token").
    pub label: String,
    /// Description of what this field is for.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Whether this field is required.
    #[serde(default)]
    pub required: bool,
    /// Whether this field should be masked (for tokens/passwords).
    #[serde(default)]
    pub secret: bool,
    /// Environment variable that can provide this value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub env_var: Option<String>,
    /// Default value, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default: Option<String>,
}

/// Result of auth/fields request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthFieldsResult {
    pub fields: Vec<AuthField>,
}

/// Authentication status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum AuthStatus {
    /// Not authenticated.
    NotAuthenticated,
    /// Authentication in progress.
    Authenticating,
    /// Successfully authenticated.
    Authenticated {
        /// Optional identity info (e.g., user email).
        #[serde(skip_serializing_if = "Option::is_none")]
        identity: Option<String>,
        /// When auth expires (Unix timestamp).
        #[serde(skip_serializing_if = "Option::is_none")]
        expires_at: Option<u64>,
    },
    /// Authentication failed.
    Failed {
        /// Error message.
        reason: String,
    },
    /// Authentication expired.
    Expired,
}

impl AuthStatus {
    pub fn is_authenticated(&self) -> bool {
        matches!(self, AuthStatus::Authenticated { .. })
    }

    pub fn is_failed(&self) -> bool {
        matches!(self, AuthStatus::Failed { .. })
    }
}

/// Result of auth/status request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthStatusResult {
    #[serde(flatten)]
    pub status: AuthStatus,
}

/// Parameters for auth/authenticate request.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthenticateParams {
    /// Credentials keyed by field name.
    pub credentials: HashMap<String, String>,
}

/// Result of auth/authenticate request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthenticateResult {
    #[serde(flatten)]
    pub status: AuthStatus,
}

/// Result of auth/revoke request.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AuthRevokeResult {}

// =============================================================================
// Scope Methods
// =============================================================================

/// A level in the scope hierarchy (e.g., project, environment).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopeLevel {
    /// Level name/key (e.g., "project", "environment").
    pub name: String,
    /// Human-readable label (e.g., "Project").
    pub display_name: String,
    /// Whether this level is required to fetch secrets.
    #[serde(default)]
    pub required: bool,
    /// Whether multiple selections are allowed.
    #[serde(default)]
    pub multi_select: bool,
    /// Optional description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Result of scope/levels request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopeLevelsResult {
    pub levels: Vec<ScopeLevel>,
}

/// An option available at a scope level.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopeOption {
    /// Unique identifier for this option.
    pub id: String,
    /// Human-readable display name.
    pub display_name: String,
    /// Optional description or metadata.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Optional icon or indicator.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Number of secrets at this scope (if known).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secret_count: Option<usize>,
}

/// Parameters for scope/options request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopeOptionsParams {
    /// The level to list options for.
    pub level: String,
    /// Parent scope selections (for hierarchical navigation).
    #[serde(default)]
    pub parent: HashMap<String, Vec<String>>,
}

/// Result of scope/options request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopeOptionsResult {
    pub options: Vec<ScopeOption>,
}

/// Current scope selection.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScopeSelection {
    /// Map of level name to selected option IDs.
    pub selections: HashMap<String, Vec<String>>,
}

impl ScopeSelection {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_selection(
        mut self,
        level: impl Into<String>,
        values: Vec<impl Into<String>>,
    ) -> Self {
        self.selections
            .insert(level.into(), values.into_iter().map(|v| v.into()).collect());
        self
    }

    pub fn set(&mut self, level: impl Into<String>, values: Vec<impl Into<String>>) {
        self.selections
            .insert(level.into(), values.into_iter().map(|v| v.into()).collect());
    }

    pub fn get(&self, level: &str) -> Option<&[String]> {
        self.selections.get(level).map(|v| v.as_slice())
    }

    pub fn get_single(&self, level: &str) -> Option<&str> {
        self.selections
            .get(level)
            .and_then(|v| v.first())
            .map(|s| s.as_str())
    }

    pub fn is_empty(&self) -> bool {
        self.selections.is_empty()
    }
}

// =============================================================================
// Secrets Methods
// =============================================================================

/// A secret/environment variable.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Secret {
    /// Variable key/name.
    pub key: String,
    /// Variable value.
    pub value: String,
    /// Optional description/comment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Whether the secret is sensitive (should be masked in UI).
    #[serde(default)]
    pub sensitive: bool,
}

/// Parameters for secrets/fetch request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretsFetchParams {
    /// Scope to fetch secrets from.
    pub scope: ScopeSelection,
}

/// Result of secrets/fetch request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretsFetchResult {
    /// The fetched secrets.
    pub secrets: Vec<Secret>,
    /// Version/ETag for cache invalidation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Scope path for display (e.g., "project/config").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope_path: Option<String>,
}

/// Parameters for secrets/get request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretsGetParams {
    /// Scope to get secret from.
    pub scope: ScopeSelection,
    /// Key to get.
    pub key: String,
}

/// Result of secrets/get request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretsGetResult {
    pub secret: Option<Secret>,
}

/// Parameters for secrets/set request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretsSetParams {
    /// Scope to set secret in.
    pub scope: ScopeSelection,
    /// Key to set.
    pub key: String,
    /// Value to set.
    pub value: String,
    /// Optional description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Result of secrets/set request.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SecretsSetResult {}

/// Parameters for secrets/delete request.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretsDeleteParams {
    /// Scope to delete secret from.
    pub scope: ScopeSelection,
    /// Key to delete.
    pub key: String,
}

/// Result of secrets/delete request.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SecretsDeleteResult {}

// =============================================================================
// Notifications
// =============================================================================

/// Parameters for secrets/changed notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretsChangedParams {
    /// Scope that changed.
    pub scope: ScopeSelection,
    /// Keys that changed (empty means all).
    #[serde(default)]
    pub keys: Vec<String>,
}

/// Log level for log notifications.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

/// Parameters for log notification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogParams {
    pub level: LogLevel,
    pub message: String,
}

// =============================================================================
// Method Constants
// =============================================================================

pub mod methods {
    // Lifecycle
    pub const INITIALIZE: &str = "initialize";
    pub const INITIALIZED: &str = "initialized";
    pub const SHUTDOWN: &str = "shutdown";
    pub const EXIT: &str = "exit";
    pub const PING: &str = "ping";

    // Authentication
    pub const AUTH_STATUS: &str = "auth/status";
    pub const AUTH_FIELDS: &str = "auth/fields";
    pub const AUTH_AUTHENTICATE: &str = "auth/authenticate";
    pub const AUTH_REVOKE: &str = "auth/revoke";

    // Scope
    pub const SCOPE_LEVELS: &str = "scope/levels";
    pub const SCOPE_OPTIONS: &str = "scope/options";

    // Secrets
    pub const SECRETS_FETCH: &str = "secrets/fetch";
    pub const SECRETS_GET: &str = "secrets/get";
    pub const SECRETS_SET: &str = "secrets/set";
    pub const SECRETS_DELETE: &str = "secrets/delete";

    // Notifications
    pub const SECRETS_CHANGED: &str = "secrets/changed";
    pub const LOG: &str = "log";
}

// =============================================================================
// Conversion utilities for existing types
// =============================================================================

impl From<AuthStatus> for super::traits::AuthStatus {
    fn from(status: AuthStatus) -> Self {
        match status {
            AuthStatus::NotAuthenticated => super::traits::AuthStatus::NotAuthenticated,
            AuthStatus::Authenticating => super::traits::AuthStatus::Authenticating,
            AuthStatus::Authenticated { identity, expires_at } => {
                super::traits::AuthStatus::Authenticated {
                    identity: identity.map(CompactString::from),
                    expires_at,
                }
            }
            AuthStatus::Failed { reason } => super::traits::AuthStatus::Failed {
                reason: CompactString::from(reason),
            },
            AuthStatus::Expired => super::traits::AuthStatus::Expired,
        }
    }
}

impl From<super::traits::AuthStatus> for AuthStatus {
    fn from(status: super::traits::AuthStatus) -> Self {
        match status {
            super::traits::AuthStatus::NotAuthenticated => AuthStatus::NotAuthenticated,
            super::traits::AuthStatus::Authenticating => AuthStatus::Authenticating,
            super::traits::AuthStatus::Authenticated { identity, expires_at } => {
                AuthStatus::Authenticated {
                    identity: identity.map(|s| s.to_string()),
                    expires_at,
                }
            }
            super::traits::AuthStatus::Failed { reason } => AuthStatus::Failed {
                reason: reason.to_string(),
            },
            super::traits::AuthStatus::Expired => AuthStatus::Expired,
        }
    }
}

impl From<AuthField> for super::traits::AuthField {
    fn from(field: AuthField) -> Self {
        super::traits::AuthField {
            name: CompactString::from(field.name),
            label: CompactString::from(field.label),
            description: field.description.map(CompactString::from),
            required: field.required,
            secret: field.secret,
            env_var: field.env_var.map(CompactString::from),
            default: field.default.map(CompactString::from),
        }
    }
}

impl From<super::traits::AuthField> for AuthField {
    fn from(field: super::traits::AuthField) -> Self {
        AuthField {
            name: field.name.to_string(),
            label: field.label.to_string(),
            description: field.description.map(|s| s.to_string()),
            required: field.required,
            secret: field.secret,
            env_var: field.env_var.map(|s| s.to_string()),
            default: field.default.map(|s| s.to_string()),
        }
    }
}

impl From<ScopeLevel> for super::traits::ScopeLevel {
    fn from(level: ScopeLevel) -> Self {
        super::traits::ScopeLevel {
            name: CompactString::from(level.name),
            display_name: CompactString::from(level.display_name),
            required: level.required,
            multi_select: level.multi_select,
            description: level.description.map(CompactString::from),
        }
    }
}

impl From<super::traits::ScopeLevel> for ScopeLevel {
    fn from(level: super::traits::ScopeLevel) -> Self {
        ScopeLevel {
            name: level.name.to_string(),
            display_name: level.display_name.to_string(),
            required: level.required,
            multi_select: level.multi_select,
            description: level.description.map(|s| s.to_string()),
        }
    }
}

impl From<ScopeOption> for super::traits::ScopeOption {
    fn from(option: ScopeOption) -> Self {
        super::traits::ScopeOption {
            id: CompactString::from(option.id),
            display_name: CompactString::from(option.display_name),
            description: option.description.map(CompactString::from),
            icon: option.icon.map(CompactString::from),
        }
    }
}

impl From<super::traits::ScopeOption> for ScopeOption {
    fn from(option: super::traits::ScopeOption) -> Self {
        ScopeOption {
            id: option.id.to_string(),
            display_name: option.display_name.to_string(),
            description: option.description.map(|s| s.to_string()),
            icon: option.icon.map(|s| s.to_string()),
            secret_count: None,
        }
    }
}

impl From<ScopeSelection> for super::traits::ScopeSelection {
    fn from(selection: ScopeSelection) -> Self {
        super::traits::ScopeSelection {
            selections: selection.selections,
        }
    }
}

impl From<super::traits::ScopeSelection> for ScopeSelection {
    fn from(selection: super::traits::ScopeSelection) -> Self {
        ScopeSelection {
            selections: selection.selections,
        }
    }
}

impl From<ProviderInfo> for super::traits::RemoteProviderInfo {
    fn from(info: ProviderInfo) -> Self {
        super::traits::RemoteProviderInfo {
            id: CompactString::from(info.id),
            display_name: CompactString::from(info.name),
            short_name: info
                .short_name
                .map(CompactString::from)
                .unwrap_or_else(|| CompactString::from("")),
            description: info.description.map(CompactString::from),
            docs_url: info.docs_url,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_rpc_request_serialization() {
        let request = JsonRpcRequest::new(1i64, "initialize")
            .with_params(InitializeParams {
                protocol_version: PROTOCOL_VERSION.to_string(),
                client_info: ClientInfo {
                    name: "ecolog-lsp".to_string(),
                    version: "1.0.0".to_string(),
                },
                capabilities: ClientCapabilities::default(),
                config: HashMap::new(),
            });

        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("initialize"));
        assert!(json.contains("2.0"));
    }

    #[test]
    fn test_json_rpc_response_success() {
        let response = JsonRpcResponse::success(
            JsonRpcId::Number(1),
            InitializeResult {
                protocol_version: PROTOCOL_VERSION.to_string(),
                provider_info: ProviderInfo {
                    id: "doppler".to_string(),
                    name: "Doppler".to_string(),
                    version: "1.0.0".to_string(),
                    short_name: Some("DPL".to_string()),
                    description: None,
                    docs_url: None,
                },
                capabilities: ProviderCapabilities::default(),
            },
        );

        assert!(response.is_success());
    }

    #[test]
    fn test_json_rpc_error_codes() {
        let error = JsonRpcError::not_authenticated("Token expired");
        assert_eq!(error.code, ErrorCode::NotAuthenticated as i32);

        let error = JsonRpcError::rate_limited(60);
        assert_eq!(error.code, ErrorCode::RateLimited as i32);
    }

    #[test]
    fn test_auth_status_tagged_serialization() {
        let authenticated = AuthStatus::Authenticated {
            identity: Some("user@example.com".to_string()),
            expires_at: None,
        };

        let json = serde_json::to_string(&authenticated).unwrap();
        assert!(json.contains("authenticated"));
        assert!(json.contains("user@example.com"));
    }

    #[test]
    fn test_scope_selection() {
        let selection = ScopeSelection::new()
            .with_selection("project", vec!["my-project"])
            .with_selection("config", vec!["dev"]);

        assert_eq!(selection.get_single("project"), Some("my-project"));
        assert_eq!(selection.get_single("config"), Some("dev"));
        assert!(!selection.is_empty());
    }
}
