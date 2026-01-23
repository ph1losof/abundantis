pub mod config;
mod registry;
mod traits;
mod variable;

#[cfg(feature = "file")]
mod file;
#[cfg(feature = "file")]
mod file_manager;

#[cfg(feature = "shell")]
mod shell;

mod memory;

pub use config::{
    FileSourceConfig, MemorySourceConfig, RemoteSourceConfig, ShellSourceConfig,
    SourceRefreshOptions,
};
pub use registry::*;
pub use traits::*;
pub use variable::*;

#[cfg(feature = "file")]
pub use file::FileSource;
#[cfg(feature = "file")]
pub use file_manager::FileSourceManager;

#[cfg(feature = "shell")]
pub use shell::ShellSource;

pub use memory::MemorySource;

pub use traits::SourceSnapshot;
