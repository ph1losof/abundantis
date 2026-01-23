use compact_str::CompactString;
use std::path::PathBuf;

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct WorkspaceContext {
    pub workspace_root: PathBuf,
    pub package_root: PathBuf,
    pub package_name: Option<CompactString>,
    pub env_files: Vec<PathBuf>,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct PackageInfo {
    pub root: PathBuf,
    pub name: Option<CompactString>,
    pub relative_path: CompactString,
}
