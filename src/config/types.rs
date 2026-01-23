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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SourcesConfig {
    pub defaults: SourceDefaults,
}

impl Default for SourcesConfig {
    fn default() -> Self {
        Self {
            defaults: SourceDefaults::default(),
        }
    }
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
