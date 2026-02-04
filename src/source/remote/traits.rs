//! Core traits and types for remote secret sources.

use compact_str::CompactString;
use serde::{Deserialize, Serialize};
use std::fmt;

/// Information about a remote provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteProviderInfo {
    /// Unique identifier for this provider (e.g., "doppler", "aws").
    pub id: CompactString,
    /// Human-readable display name (e.g., "Doppler", "AWS Secrets Manager").
    pub display_name: CompactString,
    /// Short name for UI display (e.g., "DPL", "AWS").
    pub short_name: CompactString,
    /// Provider description.
    pub description: Option<CompactString>,
    /// Provider documentation URL.
    pub docs_url: Option<String>,
}

/// Authentication status for a remote source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthStatus {
    /// Not authenticated yet.
    NotAuthenticated,
    /// Authentication in progress.
    Authenticating,
    /// Successfully authenticated.
    Authenticated {
        /// Optional identity info (e.g., user email, service account).
        identity: Option<CompactString>,
        /// When the auth expires, if applicable.
        expires_at: Option<u64>,
    },
    /// Authentication failed.
    Failed {
        /// Error message describing why auth failed.
        reason: CompactString,
    },
    /// Authentication expired and needs refresh.
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

impl fmt::Display for AuthStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuthStatus::NotAuthenticated => write!(f, "not_authenticated"),
            AuthStatus::Authenticating => write!(f, "authenticating"),
            AuthStatus::Authenticated { identity, .. } => {
                if let Some(id) = identity {
                    write!(f, "authenticated ({})", id)
                } else {
                    write!(f, "authenticated")
                }
            }
            AuthStatus::Failed { reason } => write!(f, "failed: {}", reason),
            AuthStatus::Expired => write!(f, "expired"),
        }
    }
}

/// A field required for authentication.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthField {
    /// Field name/key (e.g., "token", "client_id").
    pub name: CompactString,
    /// Human-readable label (e.g., "API Token", "Client ID").
    pub label: CompactString,
    /// Description of what this field is for.
    pub description: Option<CompactString>,
    /// Whether this field is required.
    pub required: bool,
    /// Whether this field should be masked in UI (e.g., for tokens/passwords).
    pub secret: bool,
    /// Environment variable that can provide this value.
    pub env_var: Option<CompactString>,
    /// Default value, if any.
    pub default: Option<CompactString>,
}

/// Authentication configuration passed to the authenticate method.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AuthConfig {
    /// Key-value pairs of credentials.
    pub credentials: std::collections::HashMap<String, String>,
}

impl AuthConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_credential(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.credentials.insert(key.into(), value.into());
        self
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.credentials.get(key).map(|s| s.as_str())
    }
}

/// A level in the scope hierarchy (e.g., project, environment, folder).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopeLevel {
    /// Level name/key (e.g., "project", "environment").
    pub name: CompactString,
    /// Human-readable label (e.g., "Project", "Environment").
    pub display_name: CompactString,
    /// Whether this level is required to fetch secrets.
    pub required: bool,
    /// Whether multiple selections are allowed at this level.
    pub multi_select: bool,
    /// Optional description.
    pub description: Option<CompactString>,
}

/// An option available at a scope level.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopeOption {
    /// Unique identifier for this option.
    pub id: CompactString,
    /// Human-readable display name.
    pub display_name: CompactString,
    /// Optional description or metadata.
    pub description: Option<CompactString>,
    /// Optional icon or indicator.
    pub icon: Option<CompactString>,
}

/// Selected scope for fetching secrets.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScopeSelection {
    /// Map of level name to selected option IDs.
    pub selections: std::collections::HashMap<String, Vec<String>>,
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

/// Provider-specific configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// Whether this provider is enabled.
    #[serde(default)]
    pub enabled: bool,
    /// Provider-specific settings as key-value pairs.
    #[serde(flatten)]
    pub settings: std::collections::HashMap<String, serde_json::Value>,
}

impl ProviderConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn enabled(mut self) -> Self {
        self.enabled = true;
        self
    }

    pub fn with_setting(mut self, key: impl Into<String>, value: impl Into<serde_json::Value>) -> Self {
        self.settings.insert(key.into(), value.into());
        self
    }

    pub fn get_string(&self, key: &str) -> Option<&str> {
        self.settings.get(key).and_then(|v| v.as_str())
    }

    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.settings.get(key).and_then(|v| v.as_bool())
    }

    pub fn get_u64(&self, key: &str) -> Option<u64> {
        self.settings.get(key).and_then(|v| v.as_u64())
    }
}

/// Configuration for all remote sources.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RemoteSourcesConfig {
    /// Per-provider configurations.
    #[serde(flatten)]
    pub providers: std::collections::HashMap<String, ProviderConfig>,
}

impl RemoteSourcesConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_provider(mut self, name: impl Into<String>, config: ProviderConfig) -> Self {
        self.providers.insert(name.into(), config);
        self
    }

    pub fn get(&self, provider: &str) -> Option<&ProviderConfig> {
        self.providers.get(provider)
    }

    pub fn enabled_providers(&self) -> impl Iterator<Item = (&String, &ProviderConfig)> {
        self.providers.iter().filter(|(_, c)| c.enabled)
    }
}

/// Summary information about a remote source for UI display.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteSourceInfo {
    /// Provider ID.
    pub id: CompactString,
    /// Display name.
    pub display_name: CompactString,
    /// Short name for compact UI.
    pub short_name: CompactString,
    /// Current auth status.
    pub auth_status: AuthStatus,
    /// Current scope selection.
    pub scope: ScopeSelection,
    /// Number of secrets currently loaded.
    pub secret_count: usize,
    /// Last refresh timestamp (Unix millis).
    pub last_refreshed: Option<u64>,
    /// Whether secrets are currently being fetched.
    pub loading: bool,
    /// Last error, if any.
    pub last_error: Option<CompactString>,
}
