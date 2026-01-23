use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Default)]
pub struct FileSourceConfig {
    pub active_files: Option<Vec<String>>,

    pub directory_overrides: HashMap<PathBuf, Vec<String>>,
}

#[derive(Debug, Clone)]
pub struct ShellSourceConfig {
    pub enabled: bool,

    pub include_patterns: Option<Vec<String>>,
}

impl Default for ShellSourceConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            include_patterns: None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct RemoteSourceConfig {
    pub endpoint: Option<String>,
    pub auth_token: Option<String>,
    pub timeout_ms: Option<u64>,
    pub retry_count: Option<u32>,
}

#[derive(Debug, Clone, Default)]
pub struct MemorySourceConfig {}

#[derive(Debug, Clone, Default)]
pub struct SourceRefreshOptions {
    pub preserve_config: bool,
}

impl SourceRefreshOptions {
    pub fn preserve() -> Self {
        Self {
            preserve_config: true,
        }
    }

    pub fn reset() -> Self {
        Self {
            preserve_config: false,
        }
    }
}
