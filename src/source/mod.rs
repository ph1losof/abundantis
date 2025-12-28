//! Environment variable sources.
//!
//! Implements plugin architecture for creating and managing sources.

mod traits;
mod variable;
mod registry;

#[cfg(feature = "file")]
mod file;

#[cfg(feature = "shell")]
mod shell;

mod memory;

pub use traits::*;
pub use variable::*;
pub use registry::*;

#[cfg(feature = "file")]
pub use file::FileSource;

#[cfg(feature = "shell")]
pub use shell::ShellSource;

pub use memory::MemorySource;

pub use traits::SourceSnapshot;
