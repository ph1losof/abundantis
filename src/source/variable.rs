use compact_str::CompactString;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct ParsedVariable {
    pub key: CompactString,
    pub raw_value: CompactString,
    pub source: VariableSource,
    pub description: Option<CompactString>,
    pub is_commented: bool,
}

impl ParsedVariable {
    pub fn simple(
        key: impl Into<CompactString>,
        value: impl Into<CompactString>,
        source: VariableSource,
    ) -> Self {
        Self {
            key: key.into(),
            raw_value: value.into(),
            source,
            description: None,
            is_commented: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VariableSource {
    File {
        path: PathBuf,
        offset: usize,
    },
    Shell,
    Memory,
    Remote {
        provider: CompactString,
        path: Option<String>,
    },
}

impl VariableSource {
    pub fn file_path(&self) -> Option<&PathBuf> {
        match self {
            VariableSource::File { path, .. } => Some(path),
            _ => None,
        }
    }
}
