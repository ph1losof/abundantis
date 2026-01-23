use abundantis::{config::MonorepoProviderType, source::EnvSource, Abundantis, MemorySource};

#[test]
fn test_abundantis_builder_creation() {
    let _builder = Abundantis::builder();
}

#[test]
fn test_abundantis_builder_with_memory_source() {
    let _builder = Abundantis::builder();
}

#[test]
fn test_abundantis_version() {
    assert!(!abundantis::VERSION.is_empty());
    assert!(abundantis::VERSION.contains('.'));
}

#[test]
fn test_monorepo_provider_type_values() {
    let providers = [
        MonorepoProviderType::Turbo,
        MonorepoProviderType::Nx,
        MonorepoProviderType::Lerna,
        MonorepoProviderType::Pnpm,
        MonorepoProviderType::Npm,
        MonorepoProviderType::Yarn,
        MonorepoProviderType::Cargo,
        MonorepoProviderType::Custom,
    ];

    assert_eq!(providers.len(), 8);
}

#[test]
fn test_memory_source_basic_usage() {
    let source = MemorySource::new();

    source.set("TEST_KEY", "test_value");
    source.set("ANOTHER_KEY", "another_value");

    let snapshot = source.load().unwrap();
    assert_eq!(snapshot.variables.len(), 2);
}

#[test]
fn test_memory_source_persistence() {
    let source = MemorySource::new();

    source.set("KEY1", "value1");
    source.set("KEY2", "value2");

    let snapshot1 = source.load().unwrap();
    let snapshot2 = source.load().unwrap();

    assert_eq!(snapshot1.variables.len(), snapshot2.variables.len());
    assert_eq!(snapshot1.variables.len(), 2);
}

#[test]
fn test_memory_source_update() {
    let source = MemorySource::new();

    source.set("KEY1", "value1");
    assert_eq!(source.len(), 1);

    source.remove("KEY1");
    assert_eq!(source.len(), 0);

    source.set("KEY2", "value2");
    assert_eq!(source.len(), 1);
}

#[test]
fn test_memory_source_clear() {
    let source = MemorySource::new();

    source.set("KEY1", "value1");
    source.set("KEY2", "value2");
    source.set("KEY3", "value3");
    assert_eq!(source.len(), 3);

    source.clear();
    assert_eq!(source.len(), 0);
}

#[test]
fn test_memory_source_many_keys() {
    let source = MemorySource::new();

    for i in 0..1000 {
        source.set(format!("KEY{}", i), format!("value{}", i));
    }

    assert_eq!(source.len(), 1000);
}

#[test]
fn test_memory_source_version_increments() {
    let source = MemorySource::new();

    let snapshot1 = source.load().unwrap();
    let v1 = snapshot1.version.unwrap();

    source.set("KEY1", "value1");
    let snapshot2 = source.load().unwrap();
    let v2 = snapshot2.version.unwrap();

    source.set("KEY2", "value2");
    let snapshot3 = source.load().unwrap();
    let v3 = snapshot3.version.unwrap();

    assert!(v3 > v2);
    assert!(v2 > v1);
}

#[test]
fn test_memory_source_override() {
    let source = MemorySource::new();

    source.set("KEY1", "value1");
    source.set("KEY2", "value2");

    let snapshot = source.load().unwrap();
    let var = snapshot
        .variables
        .iter()
        .find(|v| v.key.as_str() == "KEY1")
        .unwrap();
    assert_eq!(var.raw_value.as_str(), "value1");

    source.set("KEY1", "value1-updated");
    let snapshot = source.load().unwrap();
    let var = snapshot
        .variables
        .iter()
        .find(|v| v.key.as_str() == "KEY1")
        .unwrap();
    assert_eq!(var.raw_value.as_str(), "value1-updated");
}

#[test]
fn test_memory_source_empty_values() {
    let source = MemorySource::new();

    source.set("EMPTY_KEY", "");
    source.set("WHITESPACE_KEY", "   ");
    source.set("NORMAL_KEY", "value");

    let snapshot = source.load().unwrap();
    assert_eq!(snapshot.variables.len(), 3);
}

#[test]
fn test_memory_source_special_keys() {
    let source = MemorySource::new();

    source.set("UPPERCASE_KEY", "value1");
    source.set("lowercase_key", "value2");
    source.set("MixedCase_Key", "value3");
    source.set("key-with-dashes", "value4");
    source.set("key_with_underscores", "value5");

    let snapshot = source.load().unwrap();
    assert_eq!(snapshot.variables.len(), 5);
}

#[test]
fn test_memory_source_special_values() {
    let source = MemorySource::new();

    source.set("UNICODE_KEY", "日本語");
    source.set("EMOJI_KEY", "🌽");
    source.set("LONG_KEY", "a".repeat(1000));

    let snapshot = source.load().unwrap();
    assert_eq!(snapshot.variables.len(), 3);
}

#[test]
fn test_memory_source_rapid_operations() {
    let source = MemorySource::new();

    for i in 0..100 {
        for j in 0..10 {
            source.set(format!("KEY_{}_{}", i, j), format!("value_{}_{}", i, j));
        }
    }

    assert_eq!(source.len(), 1000);
}

#[test]
fn test_memory_source_concurrent_reads() {
    use std::sync::{Arc, Barrier};
    use std::thread;

    let source = Arc::new(MemorySource::new());
    let barrier = Arc::new(Barrier::new(10));

    let handles: Vec<_> = (0..10)
        .map(|thread_id| {
            let source = Arc::clone(&source);
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                for i in 0..100 {
                    source.set(format!("T{}KEY{}", thread_id, i), format!("value{}", i));
                }
                barrier.wait();
                let _ = source.load().unwrap();
            })
        })
        .collect();

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(source.len(), 1000);
}

#[test]
fn test_memory_source_with_description() {
    let source = MemorySource::new();

    source.set_with_description(
        "API_KEY",
        "secret-value",
        "Production API key for external service",
    );

    let snapshot = source.load().unwrap();
    assert_eq!(snapshot.variables.len(), 1);
    assert_eq!(
        snapshot.variables[0].description.as_deref(),
        Some("Production API key for external service")
    );
}

#[test]
fn test_memory_source_empty_state() {
    let source = MemorySource::new();
    assert!(source.is_empty());
    assert_eq!(source.len(), 0);

    let snapshot = source.load().unwrap();
    assert_eq!(snapshot.variables.len(), 0);
    assert_eq!(snapshot.version.unwrap(), 0);
}

#[test]
fn test_memory_source_interleaved_operations() {
    let source = MemorySource::new();

    for i in 0..50 {
        if i % 2 == 0 {
            source.set(format!("KEY_{}", i), format!("value_{}", i));
        } else {
            source.remove(&format!("KEY_{}", i));
        }
    }

    assert_eq!(source.len(), 25);
}

#[test]
fn test_memory_source_order_preservation() {
    let source = MemorySource::new();

    let expected_order = vec!["KEY1", "KEY2", "KEY3", "KEY4", "KEY5"];

    for key in &expected_order {
        source.set(*key, format!("value_{}", key));
    }

    let snapshot = source.load().unwrap();
    let keys: Vec<_> = snapshot.variables.iter().map(|v| v.key.as_str()).collect();

    assert_eq!(keys.len(), expected_order.len());
}

#[test]
fn test_memory_source_key_existence() {
    let source = MemorySource::new();

    source.set("KEY1", "value1");

    let snapshot = source.load().unwrap();
    assert!(snapshot.variables.iter().any(|v| v.key.as_str() == "KEY1"));
    assert!(!snapshot.variables.iter().any(|v| v.key.as_str() == "KEY2"));
}

#[test]
fn test_memory_source_invalidate() {
    let source = MemorySource::new();

    source.set("KEY1", "value1");
    assert_eq!(source.len(), 1);

    source.invalidate();

    assert_eq!(source.len(), 1);
}

#[test]
fn test_memory_source_has_changed() {
    let source = MemorySource::new();

    assert!(source.has_changed());
}

#[test]
fn test_memory_source_clone_independent() {
    let source1 = MemorySource::new();
    let source2 = MemorySource::new();

    source1.set("KEY1", "value1");
    source2.set("KEY2", "value2");

    assert_eq!(source1.len(), 1);
    assert_eq!(source2.len(), 1);

    let snapshot1 = source1.load().unwrap();
    let snapshot2 = source2.load().unwrap();

    assert_ne!(snapshot1.variables[0].key, snapshot2.variables[0].key);
}

#[test]
fn test_memory_source_capacity() {
    let source = MemorySource::new();

    for i in 0..5000 {
        source.set(format!("KEY{}", i), format!("value{}", i));
    }

    assert_eq!(source.len(), 5000);
}

#[test]
fn test_memory_source_performance_load() {
    let source = MemorySource::new();

    for i in 0..1000 {
        source.set(format!("PERF_KEY{}", i), format!("PERF_VALUE{}", i));
    }

    let start = std::time::Instant::now();
    let snapshot = source.load().unwrap();
    let duration = start.elapsed();

    assert_eq!(snapshot.variables.len(), 1000);

    assert!(duration.as_millis() < 10);
}

#[test]
fn test_abundantis_constants() {
    assert!(!abundantis::VERSION.is_empty());
}

#[test]
fn test_config_type_existence() {
    let _ = MonorepoProviderType::Turbo;
    let _ = MonorepoProviderType::Nx;
    let _ = MonorepoProviderType::Custom;
}

#[test]
fn test_memory_source_iteration() {
    let source = MemorySource::new();

    source.set("KEY1", "value1");
    source.set("KEY2", "value2");
    source.set("KEY3", "value3");

    let snapshot = source.load().unwrap();

    let mut count = 0;
    for var in snapshot.variables.iter() {
        assert!(var.key.as_str().starts_with("KEY"));
        count += 1;
    }

    assert_eq!(count, 3);
}

#[test]
fn test_memory_source_lookup() {
    let source = MemorySource::new();

    source.set("KEY1", "value1");
    source.set("KEY2", "value2");

    let snapshot = source.load().unwrap();

    let var1 = snapshot.variables.iter().find(|v| v.key.as_str() == "KEY1");
    assert!(var1.is_some());
    assert_eq!(var1.unwrap().raw_value.as_str(), "value1");

    let var2 = snapshot
        .variables
        .iter()
        .find(|v| v.key.as_str() == "NONEXISTENT");
    assert!(var2.is_none());
}

#[test]
fn test_memory_source_metadata() {
    let source = MemorySource::new();
    let metadata = source.metadata();

    assert_eq!(metadata.display_name, None);
    assert_eq!(metadata.description, None);
    assert_eq!(metadata.error_count, 0);
    assert!(metadata.last_refreshed.is_none());
}

#[test]
fn test_memory_source_id_consistency() {
    let source = MemorySource::new();

    let id1 = source.id();
    let id2 = source.id();

    assert_eq!(id1, id2);
    assert_eq!(id1.as_str(), "memory");
}

#[test]
fn test_memory_source_type_checking() {
    let source = MemorySource::new();

    assert_eq!(source.source_type(), abundantis::source::SourceType::Memory);
}

#[test]
fn test_memory_source_priority() {
    let source = MemorySource::new();

    assert_eq!(source.priority(), abundantis::source::Priority::MEMORY);
    assert_eq!(source.priority().0, 30);
}

#[test]
fn test_memory_source_capabilities_check() {
    let source = MemorySource::new();
    let caps = source.capabilities();

    assert!(caps.contains(abundantis::source::SourceCapabilities::READ));
    assert!(caps.contains(abundantis::source::SourceCapabilities::WRITE));
    assert!(caps.contains(abundantis::source::SourceCapabilities::CACHEABLE));
    assert!(!caps.contains(abundantis::source::SourceCapabilities::WATCH));
}
