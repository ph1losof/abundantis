use abundantis::{
    config::{AbundantisConfig, MonorepoProviderType},
    events::{AbundantisEvent, EventSubscriber},
    source::{EnvSource, MemorySource, SourceId},
    CacheKey, ResolutionCache,
};
use compact_str::CompactString;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

#[test]
fn test_basic_memory_source() {
    let source = MemorySource::new();
    source.set("TEST_VAR", "test_value");

    let snapshot = source.load().unwrap();
    assert_eq!(snapshot.variables.len(), 1);
    assert_eq!(snapshot.variables[0].key.as_str(), "TEST_VAR");
    assert_eq!(snapshot.variables[0].raw_value.as_str(), "test_value");
}

#[test]
fn test_memory_source_with_description() {
    let source = MemorySource::new();
    source.set_with_description("API_KEY", "secret123", "Production API key");

    let snapshot = source.load().unwrap();
    assert_eq!(
        snapshot.variables[0].description.as_deref(),
        Some("Production API key")
    );
}

#[test]
fn test_memory_source_crud() {
    let source = MemorySource::new();

    source.set("KEY1", "value1");
    source.set("KEY2", "value2");
    assert_eq!(source.len(), 2);

    let removed = source.remove("KEY1");
    assert!(removed.is_some());
    assert_eq!(source.len(), 1);

    source.clear();
    assert_eq!(source.len(), 0);
    assert!(source.is_empty());
}

#[test]
fn test_resolution_cache() {
    let config = abundantis::config::CacheConfig {
        enabled: true,
        hot_cache_size: 10,
        ttl: std::time::Duration::from_secs(60),
    };

    let cache = ResolutionCache::new(&config);
    assert!(cache.is_empty());

    let key = CacheKey::new("TEST", 123);

    let var = Arc::new(abundantis::ResolvedVariable {
        key: CompactString::new("TEST"),
        raw_value: CompactString::new("value"),
        resolved_value: CompactString::new("value"),
        source: abundantis::source::VariableSource::Memory,
        description: None,
        has_warnings: false,
        interpolation_depth: 0,
    });

    cache.insert(key.clone(), var.clone());
    assert_eq!(cache.len(), 2);

    let retrieved = cache.get(&key).unwrap();
    assert_eq!(retrieved.key.as_str(), "TEST");

    cache.invalidate(&key);
    assert!(cache.get(&key).is_none());
}

struct TestEventCounter {
    count: Arc<AtomicU32>,
}

impl TestEventCounter {
    fn new() -> Self {
        Self {
            count: Arc::new(AtomicU32::new(0)),
        }
    }

    #[allow(dead_code)]
    fn get_count(&self) -> u32 {
        self.count.load(Ordering::SeqCst)
    }
}

impl EventSubscriber for TestEventCounter {
    fn on_event(&self, _event: &AbundantisEvent) {
        self.count.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn test_event_bus() {
    use abundantis::events::EventBus;

    let bus = EventBus::new(100);
    let counter = TestEventCounter::new();

    bus.subscribe(Arc::new(counter));

    bus.publish(AbundantisEvent::CacheInvalidated { scope: None });
}

#[test]
fn test_config_defaults() {
    let config = AbundantisConfig::default();

    assert!(config.workspace.provider.is_none());
    assert!(!config.workspace.cascading);
    assert!(config.resolution.type_check);
    assert!(config.interpolation.enabled);
    assert_eq!(config.interpolation.max_depth, 64);
    assert!(config.cache.enabled);
}

#[test]
fn test_source_id() {
    let id = SourceId::new("test-source");
    assert_eq!(id.as_str(), "test-source");

    let id2: SourceId = "another-source".into();
    assert_eq!(id2.as_str(), "another-source");

    let id3: SourceId = String::from("string-source").into();
    assert_eq!(id3.as_str(), "string-source");
}

#[test]
fn test_priority_constants() {
    use abundantis::source::Priority;

    assert!(Priority::SHELL > Priority::FILE);
    assert!(Priority::FILE > Priority::MEMORY);
    assert_eq!(Priority::REMOTE.0, 75);
}

#[test]
fn test_source_capabilities() {
    use abundantis::source::SourceCapabilities;

    let capabilities =
        SourceCapabilities::READ | SourceCapabilities::WRITE | SourceCapabilities::CACHEABLE;

    assert!(capabilities.contains(SourceCapabilities::READ));
    assert!(capabilities.contains(SourceCapabilities::WRITE));
    assert!(capabilities.contains(SourceCapabilities::CACHEABLE));
    assert!(!capabilities.contains(SourceCapabilities::WATCH));
}

#[test]
fn test_variable_source() {
    use abundantis::source::VariableSource;

    let file_source = VariableSource::File {
        path: PathBuf::from("/path/to/.env"),
        offset: 42,
    };

    assert_eq!(
        file_source.file_path(),
        Some(&PathBuf::from("/path/to/.env"))
    );
}

#[test]
fn test_workspace_context() {
    let context = abundantis::workspace::WorkspaceContext {
        workspace_root: PathBuf::from("/workspace"),
        package_root: PathBuf::from("/workspace/package"),
        package_name: Some(CompactString::new("my-package")),
        env_files: vec![PathBuf::from("/workspace/package/.env")],
    };

    assert_eq!(context.workspace_root, PathBuf::from("/workspace"));
    assert_eq!(context.package_name.as_deref(), Some("my-package"));
}

#[test]
fn test_package_info() {
    let info = abundantis::workspace::PackageInfo {
        root: PathBuf::from("/workspace/package"),
        name: Some(CompactString::new("my-package")),
        relative_path: CompactString::new("package"),
    };

    assert_eq!(info.name.as_deref(), Some("my-package"));
    assert_eq!(info.relative_path.as_str(), "package");
}

#[test]
fn test_error_types() {
    use abundantis::error::{AbundantisError, SourceError};

    let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "test");
    let abundantis_error: AbundantisError = io_error.into();
    assert!(matches!(abundantis_error, AbundantisError::Io(_)));

    let source_error = SourceError::SourceRead {
        source_name: "test".into(),
        reason: "failed".into(),
    };
    let abundantis_error: AbundantisError = source_error.into();
    assert!(matches!(abundantis_error, AbundantisError::Source(_)));
}

#[test]
fn test_diagnostic() {
    use abundantis::error::{Diagnostic, DiagnosticCode, DiagnosticSeverity};

    let diagnostic = Diagnostic {
        severity: DiagnosticSeverity::Error,
        code: DiagnosticCode::EDF001,
        message: "Test error".to_string(),
        path: PathBuf::from("/test.env"),
        line: 10,
        column: 5,
    };

    assert_eq!(diagnostic.severity, DiagnosticSeverity::Error);
    assert_eq!(diagnostic.line, 10);
}

#[test]
fn test_resolution_engine_basic() {
    let resolution_config = abundantis::config::ResolutionConfig::default();
    let interpolation_config = abundantis::config::InterpolationConfig::default();
    let cache_config = abundantis::config::CacheConfig::default();

    let engine =
        abundantis::ResolutionEngine::new(&resolution_config, &interpolation_config, &cache_config);

    assert!(engine.cache().is_empty());
}

#[test]
fn test_interpolation_config() {
    let config = abundantis::config::InterpolationConfig::default();

    assert!(config.enabled);
    assert_eq!(config.max_depth, 64);
    assert!(config.features.defaults);
    assert!(config.features.alternates);
    assert!(config.features.recursion);
    assert!(!config.features.commands);
}

#[test]
fn test_cache_config() {
    let config = abundantis::config::CacheConfig::default();

    assert!(config.enabled);
    assert_eq!(config.hot_cache_size, 1000);
    assert_eq!(config.ttl, std::time::Duration::from_secs(300));
}

#[test]
fn test_file_resolution_config() {
    let config = abundantis::config::FileResolutionConfig::default();

    assert_eq!(config.mode, abundantis::config::FileMergeMode::Merge);
    assert_eq!(config.order.len(), 2);
    assert_eq!(config.order[0].as_str(), ".env");
    assert_eq!(config.order[1].as_str(), ".env.local");
}

#[test]
fn test_abundantis_stats() {
    let stats = abundantis::AbundantisStats {
        cached_variables: 100,
        source_count: 5,
    };

    assert_eq!(stats.cached_variables, 100);
    assert_eq!(stats.source_count, 5);
}

#[test]
fn test_compact_string_optimization() {
    let short = CompactString::new("short");
    let long =
        CompactString::new("this is a much longer string that won't fit in the inline buffer");

    assert_eq!(short.as_str(), "short");
    assert_eq!(
        long.as_str(),
        "this is a much longer string that won't fit in the inline buffer"
    );
}
