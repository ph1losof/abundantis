use super::config::SourceRefreshOptions;
use crate::error::SourceError;
use crate::source::variable::ParsedVariable;
use compact_str::CompactString;
use std::sync::Arc;

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct SourceId(CompactString);

impl SourceId {
    pub fn new(id: impl Into<CompactString>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for SourceId {
    fn from(s: &str) -> Self {
        Self(CompactString::new(s))
    }
}

impl From<String> for SourceId {
    fn from(s: String) -> Self {
        Self(CompactString::new(s))
    }
}

impl std::fmt::Display for SourceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Priority(pub u32);

impl Priority {
    pub const SHELL: Priority = Priority(100);
    pub const FILE: Priority = Priority(50);
    pub const MEMORY: Priority = Priority(30);
    pub const REMOTE: Priority = Priority(75);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SourceType {
    File,
    Shell,
    Memory,
    Remote,
}

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct SourceCapabilities: u32 {
        const READ       = 0b00000001;
        const WRITE      = 0b00000010;
        const WATCH      = 0b00000100;
        const SECRETS    = 0b00001000;
        const VERSIONED  = 0b00010000;
        const CACHEABLE  = 0b00100000;
        const ASYNC_ONLY = 0b01000000;
    }
}

impl Default for SourceCapabilities {
    fn default() -> Self {
        Self::READ | Self::CACHEABLE
    }
}

#[derive(Debug, Clone)]
pub struct SourceSnapshot {
    pub source_id: SourceId,
    pub variables: Arc<[ParsedVariable]>,
    pub timestamp: std::time::Instant,
    pub version: Option<u64>,
}

#[derive(Debug, Clone, Default)]
pub struct SourceMetadata {
    pub display_name: Option<CompactString>,
    pub description: Option<CompactString>,
    pub last_refreshed: Option<std::time::Instant>,
    pub error_count: u32,
}

pub trait EnvSource: Send + Sync {
    fn id(&self) -> &SourceId;
    fn source_type(&self) -> SourceType;
    fn priority(&self) -> Priority;
    fn capabilities(&self) -> SourceCapabilities;
    fn load(&self) -> Result<SourceSnapshot, SourceError>;
    fn has_changed(&self) -> bool;
    fn invalidate(&self);
    fn metadata(&self) -> SourceMetadata {
        SourceMetadata::default()
    }

    fn refresh(&self, _options: &SourceRefreshOptions) {
        self.invalidate();
    }
}

#[cfg(feature = "async")]
#[async_trait::async_trait]
pub trait AsyncEnvSource: Send + Sync {
    fn id(&self) -> &SourceId;
    fn source_type(&self) -> SourceType;
    fn priority(&self) -> Priority;
    fn capabilities(&self) -> SourceCapabilities;

    async fn load(&self) -> Result<SourceSnapshot, SourceError>;
    async fn refresh(&self) -> Result<bool, SourceError>;

    fn metadata(&self) -> SourceMetadata {
        SourceMetadata::default()
    }
}
