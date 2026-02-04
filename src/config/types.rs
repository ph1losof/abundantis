use compact_str::CompactString;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AbundantisConfig {
    pub workspace: WorkspaceConfig,
    pub resolution: ResolutionConfig,
    pub interpolation: InterpolationConfig,
    pub cache: CacheConfig,
    pub sources: SourcesConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct WorkspaceConfig {
    pub root: Option<PathBuf>,
    pub provider: Option<MonorepoProviderType>,
    #[serde(default)]
    pub roots: Vec<CompactString>,
    #[serde(default)]
    pub cascading: bool,
    #[serde(default = "default_env_files")]
    pub env_files: Vec<CompactString>,
    #[serde(default = "default_ignores")]
    pub ignores: Vec<CompactString>,
}

impl Default for WorkspaceConfig {
    fn default() -> Self {
        Self {
            root: None,
            provider: None,
            roots: Vec::new(),
            cascading: false,
            env_files: default_env_files(),
            ignores: default_ignores(),
        }
    }
}

fn default_env_files() -> Vec<CompactString> {
    vec![
        ".env".into(),
        ".env.local".into(),
        ".env.development".into(),
        ".env.production".into(),
    ]
}

fn default_ignores() -> Vec<CompactString> {
    vec![
        "**/node_modules/**".into(),
        "**/.git/**".into(),
        "**/target/**".into(),
        "**/dist/**".into(),
        "**/build/**".into(),
    ]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MonorepoProviderType {
    Turbo,
    Nx,
    Lerna,
    Pnpm,
    Npm,
    Yarn,
    Cargo,
    Custom,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ResolutionConfig {
    #[serde(default = "default_precedence")]
    pub precedence: Vec<SourcePrecedence>,
    #[serde(default)]
    pub files: FileResolutionConfig,
    #[serde(default = "default_true")]
    pub type_check: bool,
}

impl Default for ResolutionConfig {
    fn default() -> Self {
        Self {
            precedence: default_precedence(),
            files: FileResolutionConfig::default(),
            type_check: true,
        }
    }
}

impl ResolutionConfig {
    pub fn precedence_from_defaults(defaults: &SourceDefaults) -> Vec<SourcePrecedence> {
        let mut precedence = Vec::new();
        if defaults.shell {
            precedence.push(SourcePrecedence::Shell);
        }
        if defaults.file {
            precedence.push(SourcePrecedence::File);
        }
        if defaults.remote {
            precedence.push(SourcePrecedence::Remote);
        }
        precedence
    }
}

fn default_precedence() -> Vec<SourcePrecedence> {
    vec![SourcePrecedence::Shell, SourcePrecedence::File]
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SourcePrecedence {
    Shell,
    File,
    Remote,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct FileResolutionConfig {
    #[serde(default)]
    pub mode: FileMergeMode,
    #[serde(default = "default_file_order")]
    pub order: Vec<CompactString>,
}

impl Default for FileResolutionConfig {
    fn default() -> Self {
        Self {
            mode: FileMergeMode::default(),
            order: default_file_order(),
        }
    }
}

fn default_file_order() -> Vec<CompactString> {
    vec![".env".into(), ".env.local".into()]
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileMergeMode {
    #[default]
    Merge,
    Override,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct InterpolationConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_max_depth")]
    pub max_depth: u32,
    #[serde(default)]
    pub features: InterpolationFeatures,
}

impl Default for InterpolationConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_depth: default_max_depth(),
            features: InterpolationFeatures::default(),
        }
    }
}

fn default_max_depth() -> u32 {
    64
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct InterpolationFeatures {
    #[serde(default = "default_true")]
    pub defaults: bool,
    #[serde(default = "default_true")]
    pub alternates: bool,
    #[serde(default = "default_true")]
    pub recursion: bool,
    #[serde(default)]
    pub commands: bool,
}

impl Default for InterpolationFeatures {
    fn default() -> Self {
        Self {
            defaults: true,
            alternates: true,
            recursion: true,
            commands: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CacheConfig {
    pub enabled: bool,
    pub hot_cache_size: usize,
    #[serde(with = "humantime_serde")]
    pub ttl: std::time::Duration,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            hot_cache_size: 1000,
            ttl: std::time::Duration::from_secs(300),
        }
    }
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct SourcesConfig {
    pub defaults: SourceDefaults,
    /// Remote source provider configurations (legacy).
    #[cfg(feature = "remote")]
    #[serde(default)]
    pub remote: crate::source::remote::RemoteSourcesConfig,
    /// External provider configuration.
    #[cfg(feature = "remote")]
    #[serde(default)]
    pub providers: ProvidersConfig,
}

/// Configuration for external out-of-process providers.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ProvidersConfig {
    /// Path to the directory containing provider binaries.
    /// Default: ~/.local/share/ecolog/providers
    #[serde(default = "default_providers_path")]
    pub path: PathBuf,
    /// Individual provider configurations.
    #[serde(flatten)]
    pub providers: std::collections::HashMap<String, ExternalProviderConfig>,
}

impl Default for ProvidersConfig {
    fn default() -> Self {
        Self {
            path: default_providers_path(),
            providers: std::collections::HashMap::new(),
        }
    }
}

fn default_providers_path() -> PathBuf {
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(".local/share/ecolog/providers");
        }
    }
    #[cfg(target_os = "windows")]
    {
        if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
            return PathBuf::from(local_app_data).join("ecolog\\providers");
        }
    }
    PathBuf::from(".ecolog/providers")
}

/// Configuration for an individual external provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ExternalProviderConfig {
    /// Whether this provider is enabled.
    #[serde(default)]
    pub enabled: bool,
    /// Override the binary path (instead of using providers_path).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binary: Option<PathBuf>,
    /// When to spawn the provider process.
    #[serde(default)]
    pub spawn: SpawnStrategy,
    /// Provider-specific settings.
    #[serde(flatten)]
    pub settings: std::collections::HashMap<String, serde_json::Value>,
}

impl Default for ExternalProviderConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            binary: None,
            spawn: SpawnStrategy::default(),
            settings: std::collections::HashMap::new(),
        }
    }
}

impl ExternalProviderConfig {
    /// Creates an enabled provider config.
    pub fn enabled() -> Self {
        Self {
            enabled: true,
            ..Default::default()
        }
    }

    /// Sets a custom binary path.
    pub fn with_binary(mut self, path: impl Into<PathBuf>) -> Self {
        self.binary = Some(path.into());
        self
    }

    /// Sets the spawn strategy.
    pub fn with_spawn(mut self, spawn: SpawnStrategy) -> Self {
        self.spawn = spawn;
        self
    }

    /// Adds a setting.
    pub fn with_setting(
        mut self,
        key: impl Into<String>,
        value: impl Into<serde_json::Value>,
    ) -> Self {
        self.settings.insert(key.into(), value.into());
        self
    }

    /// Gets a string setting.
    pub fn get_string(&self, key: &str) -> Option<&str> {
        self.settings.get(key).and_then(|v| v.as_str())
    }

    /// Gets a bool setting.
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.settings.get(key).and_then(|v| v.as_bool())
    }
}

/// When to spawn the provider process.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SpawnStrategy {
    /// Spawn on first access (default).
    #[default]
    Lazy,
    /// Spawn at LSP startup.
    Eager,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SourceDefaults {
    #[serde(default = "default_true")]
    pub shell: bool,
    #[serde(default = "default_true")]
    pub file: bool,
    #[serde(default)]
    pub remote: bool,
}

impl Default for SourceDefaults {
    fn default() -> Self {
        Self {
            shell: true,
            file: true,
            remote: false,
        }
    }
}
