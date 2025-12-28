//! Error types for abundantis.
//!
//! Uses `thiserror` for ergonomic error handling with zero-cost abstractions.

use std::path::PathBuf;
use thiserror::Error;

/// Main error type for abundantis operations.
#[derive(Error, Debug)]
pub enum AbundantisError {
    // ─────────────────────────────────────────────────────────────
    // Configuration Errors
    // ─────────────────────────────────────────────────────────────
    #[error("Configuration error: {message}")]
    Config { message: String, path: Option<PathBuf> },

    #[error("Missing required configuration: `{field}`. {suggestion}")]
    MissingConfig { field: &'static str, suggestion: String },

    #[error("Unknown provider `{provider}`. Valid options: turbo, nx, lerna, pnpm, npm, yarn, cargo, custom")]
    UnknownProvider { provider: String },

    #[error("Invalid glob pattern `{pattern}`: {reason}")]
    InvalidGlob { pattern: String, reason: String },

    // ─────────────────────────────────────────────────────────────
    // Workspace Errors
    // ─────────────────────────────────────────────────────────────
    #[error("Workspace root not found. Searched from: {search_path:?}")]
    WorkspaceNotFound { search_path: PathBuf },

    #[error("Provider config file not found: {expected_file} in {search_path:?}")]
    ProviderConfigNotFound {
        expected_file: &'static str,
        search_path: PathBuf,
    },

    #[error("Failed to parse provider config `{path:?}`: {reason}")]
    ProviderConfigParse { path: PathBuf, reason: String },

    // ─────────────────────────────────────────────────────────────
    // Source Errors
    // ─────────────────────────────────────────────────────────────
    #[error("Source error: {0}")]
    Source(#[from] SourceError),

    // ─────────────────────────────────────────────────────────────
    // Resolution Errors
    // ─────────────────────────────────────────────────────────────
    #[error("Circular dependency detected: {chain}")]
    CircularDependency { chain: String },

    #[error("Max interpolation depth ({depth}) exceeded for `{key}`")]
    MaxDepthExceeded { key: String, depth: u32 },

    #[error("Undefined variable `{key}` referenced in interpolation")]
    UndefinedVariable { key: String },

    // ─────────────────────────────────────────────────────────────
    // IO Errors
    // ─────────────────────────────────────────────────────────────
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    // ─────────────────────────────────────────────────────────────
    // Runtime Errors
    // ─────────────────────────────────────────────────────────────
    #[error("Tokio runtime error: {0}")]
    Runtime(String),

    #[error("Cache error: {0}")]
    Cache(String),
}

/// Source-specific errors.
#[derive(Error, Debug, Clone)]
pub enum SourceError {
    #[error("Failed to read source `{source_name}`: {reason}")]
    SourceRead { source_name: String, reason: String },

    #[error("Parse error in `{path:?}` at line {line}: {message}")]
    ParseError {
        path: PathBuf,
        line: u32,
        message: String,
    },

    #[error("Remote source error from `{provider}`: {reason}")]
    Remote { provider: String, reason: String },

    #[error("Timeout while loading source `{source_name}`")]
    Timeout { source_name: String },

    #[error("Authentication failed for source `{source_name}`")]
    Authentication { source_name: String },

    #[error("Permission denied for source `{source_name}`")]
    Permission { source_name: String },

    #[error("Unsupported operation `{operation}` for source: {reason}")]
    UnsupportedOperation {
        operation: String,
        source_type: String,
        reason: String,
    },
}

/// Convenience result type for abundantis.
pub type Result<T> = std::result::Result<T, AbundantisError>;

// ─────────────────────────────────────────────────────────────────────────────
// Diagnostic (non-fatal issues for LSP)
// ─────────────────────────────────────────────────────────────────────────────

/// Severity level for diagnostics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Info,
    Hint,
}

/// A diagnostic message for LSP reporting.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: DiagnosticSeverity,
    pub code: DiagnosticCode,
    pub message: String,
    pub path: PathBuf,
    pub line: u32,
    pub column: u32,
}

/// Diagnostic codes for categorization.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DiagnosticCode {
    // EDF Syntax
    EDF001,
    EDF002,
    EDF003,
    EDF004,

    // Resolution
    RES001,
    RES002,
    RES003,

    // Workspace
    WS001,
    WS002,
}

impl std::fmt::Display for DiagnosticCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}
