use abundantis::error::{
    AbundantisError, Diagnostic, DiagnosticCode, DiagnosticSeverity, SourceError,
};
use abundantis::Result;
use std::path::PathBuf;

#[test]
fn test_error_display() {
    let err = AbundantisError::Config {
        message: "Test error".to_string(),
        path: None,
    };

    let display = format!("{}", err);
    assert!(display.contains("Test error"));
    assert!(display.contains("Configuration error"));
}

#[test]
fn test_source_error_display() {
    let err = SourceError::SourceRead {
        source_name: "test-source".to_string(),
        reason: "Failed to read".to_string(),
    };

    let display = format!("{}", err);
    assert!(display.contains("test-source"));
    assert!(display.contains("Failed to read"));
}

#[test]
fn test_error_source_conversion() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "File not found");

    let result: Result<()> = Err(io_err.into());
    assert!(result.is_err());

    match result {
        Err(AbundantisError::Io { .. }) => (),
        _ => panic!("Expected Io error"),
    }
}

#[test]
fn test_error_missing_config() {
    let err = AbundantisError::MissingConfig {
        field: "test.field",
        suggestion: "Use default value".to_string(),
    };

    let display = format!("{}", err);
    assert!(display.contains("test.field"));
    assert!(display.contains("Use default value"));
    assert!(display.contains("Missing required configuration"));
}

#[test]
fn test_error_unknown_provider() {
    let err = AbundantisError::UnknownProvider {
        provider: "invalid-provider".to_string(),
    };

    let display = format!("{}", err);
    assert!(display.contains("invalid-provider"));
    assert!(display.contains("Unknown provider"));
}

#[test]
fn test_error_invalid_glob() {
    let err = AbundantisError::InvalidGlob {
        pattern: "**/*.invalid".to_string(),
        reason: "Invalid pattern".to_string(),
    };

    let display = format!("{}", err);
    assert!(display.contains("**/*.invalid"));
    assert!(display.contains("Invalid pattern"));
}

#[test]
fn test_error_workspace_not_found() {
    let err = AbundantisError::WorkspaceNotFound {
        search_path: PathBuf::from("/nonexistent"),
    };

    let display = format!("{}", err);
    assert!(display.contains("/nonexistent"));
    assert!(display.contains("Workspace root not found"));
}

#[test]
fn test_error_provider_config_not_found() {
    let err = AbundantisError::ProviderConfigNotFound {
        expected_file: "turbo.json",
        search_path: PathBuf::from("/workspace"),
    };

    let display = format!("{}", err);
    assert!(display.contains("turbo.json"));
    assert!(display.contains("/workspace"));
    assert!(display.contains("Provider config file not found"));
}

#[test]
fn test_error_provider_config_parse() {
    let err = AbundantisError::ProviderConfigParse {
        path: PathBuf::from("/workspace/turbo.json"),
        reason: "Invalid JSON".to_string(),
    };

    let display = format!("{}", err);
    assert!(display.contains("/workspace/turbo.json"));
    assert!(display.contains("Invalid JSON"));
}

#[test]
fn test_error_circular_dependency() {
    let err = AbundantisError::CircularDependency {
        chain: "A -> B -> A".to_string(),
    };

    let display = format!("{}", err);
    assert!(display.contains("A -> B -> A"));
    assert!(display.contains("Circular dependency"));
}

#[test]
fn test_error_max_depth_exceeded() {
    let err = AbundantisError::MaxDepthExceeded {
        key: "RECURSIVE_VAR".to_string(),
        depth: 100,
    };

    let display = format!("{}", err);
    assert!(display.contains("RECURSIVE_VAR"));
    assert!(display.contains("100"));
    assert!(display.contains("Max interpolation depth"));
}

#[test]
fn test_error_undefined_variable() {
    let err = AbundantisError::UndefinedVariable {
        key: "UNDEFINED_VAR".to_string(),
    };

    let display = format!("{}", err);
    assert!(display.contains("UNDEFINED_VAR"));
    assert!(display.contains("Undefined variable"));
}

#[test]
fn test_error_runtime() {
    let err = AbundantisError::Runtime("Tokio error".to_string());

    let display = format!("{}", err);
    assert!(display.contains("Tokio error"));
    assert!(display.contains("Tokio runtime error"));
}

#[test]
fn test_error_cache() {
    let err = AbundantisError::Cache("Cache invalidation failed".to_string());

    let display = format!("{}", err);
    assert!(display.contains("Cache invalidation failed"));
    assert!(display.contains("Cache error"));
}

#[test]
fn test_source_error_parse_error() {
    let err = SourceError::ParseError {
        path: PathBuf::from("/.env"),
        line: 42,
        message: "Invalid syntax".to_string(),
    };

    let display = format!("{}", err);
    assert!(display.contains("/.env"));
    assert!(display.contains("42"));
    assert!(display.contains("Invalid syntax"));
}

#[test]
fn test_source_error_remote() {
    let err = SourceError::Remote {
        provider: "vault".to_string(),
        reason: "Connection timeout".to_string(),
    };

    let display = format!("{}", err);
    assert!(display.contains("vault"));
    assert!(display.contains("Connection timeout"));
    assert!(display.contains("Remote source error"));
}

#[test]
fn test_source_error_timeout() {
    let err = SourceError::Timeout {
        source_name: "slow-source".to_string(),
    };

    let display = format!("{}", err);
    assert!(display.contains("slow-source"));
    assert!(display.contains("Timeout"));
}

#[test]
fn test_source_error_authentication() {
    let err = SourceError::Authentication {
        source_name: "secured-source".to_string(),
    };

    let display = format!("{}", err);
    assert!(display.contains("secured-source"));
    assert!(display.contains("Authentication failed"));
}

#[test]
fn test_source_error_permission() {
    let err = SourceError::Permission {
        source_name: "protected-source".to_string(),
    };

    let display = format!("{}", err);
    assert!(display.contains("protected-source"));
    assert!(display.contains("Permission denied"));
}

#[test]
fn test_diagnostic_creation() {
    let diagnostic = Diagnostic {
        severity: DiagnosticSeverity::Error,
        code: DiagnosticCode::RES001,
        message: "Undefined variable".to_string(),
        path: PathBuf::from("/.env"),
        line: 10,
        column: 5,
    };

    assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
    assert_eq!(diagnostic.code, DiagnosticCode::RES001);
    assert_eq!(diagnostic.message, "Undefined variable");
    assert_eq!(diagnostic.path, PathBuf::from("/.env"));
    assert_eq!(diagnostic.line, 10);
    assert_eq!(diagnostic.column, 5);
}

#[test]
fn test_diagnostic_all_severities() {
    let severities = [
        DiagnosticSeverity::Error,
        DiagnosticSeverity::Warning,
        DiagnosticSeverity::Info,
        DiagnosticSeverity::Hint,
    ];

    assert_eq!(severities.len(), 4);
}

#[test]
fn test_diagnostic_all_codes() {
    let codes = [
        DiagnosticCode::EDF001,
        DiagnosticCode::EDF002,
        DiagnosticCode::EDF003,
        DiagnosticCode::EDF004,
        DiagnosticCode::RES001,
        DiagnosticCode::RES002,
        DiagnosticCode::RES003,
        DiagnosticCode::WS001,
        DiagnosticCode::WS002,
    ];

    assert_eq!(codes.len(), 9);
}

#[test]
fn test_diagnostic_code_display() {
    let codes = [
        DiagnosticCode::EDF001,
        DiagnosticCode::RES001,
        DiagnosticCode::WS001,
    ];

    for code in codes {
        let display = format!("{}", code);
        assert!(!display.is_empty());
    }
}

#[test]
fn test_diagnostic_clone() {
    let diag1 = Diagnostic {
        severity: DiagnosticSeverity::Warning,
        code: DiagnosticCode::RES002,
        message: "Test".to_string(),
        path: PathBuf::from("/.env"),
        line: 5,
        column: 2,
    };

    let diag2 = diag1.clone();

    assert_eq!(diag1.severity, diag2.severity);
    assert_eq!(diag1.code, diag2.code);
    assert_eq!(diag1.message, diag2.message);
    assert_eq!(diag1.path, diag2.path);
    assert_eq!(diag1.line, diag2.line);
    assert_eq!(diag1.column, diag2.column);
}

#[test]
fn test_diagnostic_equality() {
    let diag1 = Diagnostic {
        severity: DiagnosticSeverity::Error,
        code: DiagnosticCode::RES001,
        message: "Test".to_string(),
        path: PathBuf::from("/.env"),
        line: 10,
        column: 5,
    };

    let diag2 = Diagnostic {
        severity: DiagnosticSeverity::Error,
        code: DiagnosticCode::RES001,
        message: "Test".to_string(),
        path: PathBuf::from("/.env"),
        line: 10,
        column: 5,
    };

    assert_eq!(diag1, diag2);
}

#[test]
fn test_diagnostic_inequality() {
    let diag1 = Diagnostic {
        severity: DiagnosticSeverity::Error,
        code: DiagnosticCode::RES001,
        message: "Test".to_string(),
        path: PathBuf::from("/.env"),
        line: 10,
        column: 5,
    };

    let diag2 = Diagnostic {
        severity: DiagnosticSeverity::Warning,
        code: DiagnosticCode::RES001,
        message: "Test".to_string(),
        path: PathBuf::from("/.env"),
        line: 10,
        column: 5,
    };

    assert_ne!(diag1, diag2);
}

#[test]
fn test_error_chain() {
    let source_err = SourceError::SourceRead {
        source_name: "file.env".to_string(),
        reason: "IO error".to_string(),
    };

    let abundantis_err = AbundantisError::Source(source_err);

    let display = format!("{}", abundantis_err);
    assert!(display.contains("file.env"));
    assert!(display.contains("IO error"));
}

#[test]
fn test_result_type_alias() {
    let success = "Success".to_string();

    let result: Result<String> = if true {
        Ok(success.clone())
    } else {
        Err(AbundantisError::Runtime("error".to_string()))
    };

    assert!(result.is_ok());
    if let Ok(val) = result {
        assert_eq!(val, success);
    }
}

#[test]
fn test_result_type_alias_error() {
    let result: Result<String> = if false {
        Ok("success".to_string())
    } else {
        Err(AbundantisError::Runtime("Test error".to_string()))
    };

    assert!(result.is_err());

    if let Err(err) = result {
        assert!(matches!(err, AbundantisError::Runtime { .. }));
    }
}

#[test]
fn test_diagnostic_severity_ordering() {
    let severities = vec![
        DiagnosticSeverity::Hint,
        DiagnosticSeverity::Info,
        DiagnosticSeverity::Warning,
        DiagnosticSeverity::Error,
    ];

    let unique: std::collections::HashSet<_> = severities.into_iter().collect();
    assert_eq!(unique.len(), 4);
}

#[test]
fn test_diagnostic_code_ordering() {
    let codes = vec![
        DiagnosticCode::EDF001,
        DiagnosticCode::EDF002,
        DiagnosticCode::RES001,
        DiagnosticCode::RES002,
    ];

    let unique: std::collections::HashSet<_> = codes.into_iter().collect();
    assert_eq!(unique.len(), 4);
}

#[test]
fn test_source_error_debug() {
    let err = SourceError::ParseError {
        path: PathBuf::from("/.env"),
        line: 10,
        message: "Test".to_string(),
    };

    let debug_str = format!("{:?}", err);
    assert!(debug_str.contains("ParseError"));
    assert!(debug_str.contains("/.env"));
    assert!(debug_str.contains("10"));
}

#[test]
fn test_abundantis_error_debug() {
    let err = AbundantisError::Config {
        message: "Test".to_string(),
        path: Some(PathBuf::from("/config.toml")),
    };

    let debug_str = format!("{:?}", err);
    assert!(debug_str.contains("Config"));
    assert!(debug_str.contains("Test"));
    assert!(debug_str.contains("/config.toml"));
}

#[test]
fn test_diagnostic_with_empty_path() {
    let diagnostic = Diagnostic {
        severity: DiagnosticSeverity::Error,
        code: DiagnosticCode::RES001,
        message: "Test".to_string(),
        path: PathBuf::new(),
        line: 0,
        column: 0,
    };

    assert!(diagnostic.path.as_os_str().is_empty());
}

#[test]
fn test_diagnostic_with_large_line_column() {
    let diagnostic = Diagnostic {
        severity: DiagnosticSeverity::Warning,
        code: DiagnosticCode::WS001,
        message: "Test".to_string(),
        path: PathBuf::from("/.env"),
        line: 999999,
        column: 999999,
    };

    assert_eq!(diagnostic.line, 999999);
    assert_eq!(diagnostic.column, 999999);
}

#[test]
fn test_source_error_with_special_chars() {
    let err = SourceError::SourceRead {
        source_name: "test-source-with-special.chars?".to_string(),
        reason: "Special chars: !@#$%^&*()".to_string(),
    };

    let display = format!("{}", err);
    assert!(display.contains("test-source-with-special.chars?"));
    assert!(display.contains("!@#$%^&*()"));
}

#[test]
fn test_abundantis_error_all_variants() {
    let errors = vec![
        AbundantisError::Config {
            message: "test".to_string(),
            path: None,
        },
        AbundantisError::MissingConfig {
            field: "test",
            suggestion: "test".to_string(),
        },
        AbundantisError::UnknownProvider {
            provider: "test".to_string(),
        },
        AbundantisError::InvalidGlob {
            pattern: "test".to_string(),
            reason: "test".to_string(),
        },
        AbundantisError::WorkspaceNotFound {
            search_path: PathBuf::from("/"),
        },
        AbundantisError::ProviderConfigNotFound {
            expected_file: "test",
            search_path: PathBuf::from("/"),
        },
        AbundantisError::ProviderConfigParse {
            path: PathBuf::from("/"),
            reason: "test".to_string(),
        },
        AbundantisError::CircularDependency {
            chain: "test".to_string(),
        },
        AbundantisError::MaxDepthExceeded {
            key: "test".to_string(),
            depth: 0,
        },
        AbundantisError::UndefinedVariable {
            key: "test".to_string(),
        },
        AbundantisError::Runtime("test".to_string()),
        AbundantisError::Cache("test".to_string()),
        AbundantisError::Io(std::io::Error::other("test")),
    ];

    assert_eq!(errors.len(), 13);
}

#[test]
fn test_source_error_all_variants() {
    let errors = vec![
        SourceError::SourceRead {
            source_name: "test".to_string(),
            reason: "test".to_string(),
        },
        SourceError::ParseError {
            path: PathBuf::from("/"),
            line: 0,
            message: "test".to_string(),
        },
        SourceError::Remote {
            provider: "test".to_string(),
            reason: "test".to_string(),
        },
        SourceError::Timeout {
            source_name: "test".to_string(),
        },
        SourceError::Authentication {
            source_name: "test".to_string(),
        },
        SourceError::Permission {
            source_name: "test".to_string(),
        },
    ];

    assert_eq!(errors.len(), 6);
}
