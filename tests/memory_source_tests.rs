use abundantis::source::{EnvSource, MemorySource, Priority, SourceCapabilities, SourceType};

#[test]
fn test_memory_source_creation() {
    let source = MemorySource::new();
    assert_eq!(source.id().as_str(), "memory");
    assert_eq!(source.source_type(), SourceType::Memory);
    assert_eq!(source.priority(), Priority::MEMORY);
}

#[test]
fn test_memory_source_default() {
    let source = MemorySource::default();
    assert_eq!(source.id().as_str(), "memory");
}

#[test]
fn test_memory_source_capabilities() {
    let source = MemorySource::new();
    let caps = source.capabilities();

    assert!(caps.contains(SourceCapabilities::READ));
    assert!(caps.contains(SourceCapabilities::WRITE));
    assert!(caps.contains(SourceCapabilities::CACHEABLE));
    assert!(!caps.contains(SourceCapabilities::WATCH));
    assert!(!caps.contains(SourceCapabilities::ASYNC_ONLY));
}

#[test]
fn test_memory_source_set() {
    let source = MemorySource::new();

    source.set("KEY1", "value1");
    source.set("KEY2", "value2");
    source.set("KEY3", "value3");

    let snapshot = source.load().unwrap();
    assert_eq!(snapshot.variables.len(), 3);

    let keys: Vec<_> = snapshot.variables.iter().map(|v| v.key.as_str()).collect();
    assert!(keys.contains(&"KEY1"));
    assert!(keys.contains(&"KEY2"));
    assert!(keys.contains(&"KEY3"));
}

#[test]
fn test_memory_source_set_with_description() {
    let source = MemorySource::new();

    source.set_with_description("API_KEY", "secret-value", "External API key for production");

    let snapshot = source.load().unwrap();
    assert_eq!(snapshot.variables.len(), 1);
    assert_eq!(snapshot.variables[0].key.as_str(), "API_KEY");
    assert_eq!(snapshot.variables[0].raw_value.as_str(), "secret-value");
    assert_eq!(
        snapshot.variables[0].description.as_deref(),
        Some("External API key for production")
    );
}

#[test]
fn test_memory_source_remove() {
    let source = MemorySource::new();

    source.set("KEY1", "value1");
    source.set("KEY2", "value2");
    assert_eq!(source.len(), 2);

    let removed = source.remove("KEY1");
    assert!(removed.is_some());
    assert_eq!(removed.unwrap().key.as_str(), "KEY1");
    assert_eq!(source.len(), 1);
}

#[test]
fn test_memory_source_remove_nonexistent() {
    let source = MemorySource::new();

    source.set("KEY1", "value1");

    let removed = source.remove("KEY2");
    assert!(removed.is_none());
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
    assert!(source.is_empty());
}

#[test]
fn test_memory_source_is_empty() {
    let source = MemorySource::new();
    assert!(source.is_empty());

    source.set("KEY1", "value1");
    assert!(!source.is_empty());
}

#[test]
fn test_memory_source_len() {
    let source = MemorySource::new();
    assert_eq!(source.len(), 0);

    for i in 0..100 {
        source.set(format!("KEY{}", i), format!("value{}", i));
    }

    assert_eq!(source.len(), 100);
}

#[test]
fn test_memory_source_overwrite() {
    let source = MemorySource::new();

    source.set("KEY1", "value1");
    source.set("KEY2", "value2");

    source.set("KEY1", "value1-updated");

    let snapshot = source.load().unwrap();
    assert_eq!(snapshot.variables.len(), 2);

    let key1 = snapshot
        .variables
        .iter()
        .find(|v| v.key.as_str() == "KEY1")
        .unwrap();
    assert_eq!(key1.raw_value.as_str(), "value1-updated");
}

#[test]
fn test_memory_source_version_tracking() {
    let source = MemorySource::new();

    let snapshot1 = source.load().unwrap();
    let v1 = snapshot1.version.unwrap();

    source.set("KEY1", "value1");
    let snapshot2 = source.load().unwrap();
    let v2 = snapshot2.version.unwrap();

    assert!(v2 > v1);

    source.set("KEY2", "value2");
    let snapshot3 = source.load().unwrap();
    let v3 = snapshot3.version.unwrap();

    assert!(v3 > v2);
}

#[test]
fn test_memory_source_load() {
    let source = MemorySource::new();

    source.set("KEY1", "value1");
    source.set("KEY2", "value2");
    source.set("KEY3", "value3");

    let snapshot = source.load().unwrap();
    assert_eq!(snapshot.variables.len(), 3);
    assert_eq!(snapshot.source_id.as_str(), "memory");
    assert!(snapshot.timestamp.elapsed().as_secs() < 1);
    assert!(snapshot.version.is_some());
}

#[test]
fn test_memory_source_has_changed() {
    let source = MemorySource::new();

    assert!(source.has_changed());
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
fn test_memory_source_empty_load() {
    let source = MemorySource::new();

    let snapshot = source.load().unwrap();
    assert_eq!(snapshot.variables.len(), 0);
    assert_eq!(snapshot.version.unwrap(), 0);
}

#[test]
fn test_memory_source_special_characters() {
    let source = MemorySource::new();

    source.set("KEY-WITH-DASHES", "value");
    source.set("KEY_WITH_UNDERSCORE", "value");
    source.set("key.with.dots", "value");

    let snapshot = source.load().unwrap();
    assert_eq!(snapshot.variables.len(), 3);
}

#[test]
fn test_memory_source_large_values() {
    let source = MemorySource::new();

    let large_value = "a".repeat(10000);
    source.set("LARGE_KEY", large_value.clone());

    let snapshot = source.load().unwrap();
    assert_eq!(snapshot.variables.len(), 1);
    assert_eq!(snapshot.variables[0].raw_value.len(), 10000);
}

#[test]
fn test_memory_source_unicode() {
    let source = MemorySource::new();

    source.set("UNICODE_KEY", "日本語");
    source.set("EMOJI_KEY", "🌽");
    source.set("ARABIC_KEY", "مرحبا");

    let snapshot = source.load().unwrap();
    assert_eq!(snapshot.variables.len(), 3);
}

#[test]
fn test_memory_source_order_preserved() {
    let source = MemorySource::new();

    for i in 0..10 {
        source.set(format!("KEY{}", i), format!("value{}", i));
    }

    let snapshot = source.load().unwrap();
    assert_eq!(snapshot.variables.len(), 10);

    for (i, var) in snapshot.variables.iter().enumerate() {
        assert_eq!(var.key.as_str(), format!("KEY{}", i));
        assert_eq!(var.raw_value.as_str(), format!("value{}", i));
    }
}

#[test]
fn test_memory_source_many_operations() {
    let source = MemorySource::new();

    for i in 0..1000 {
        source.set(format!("KEY{}", i), format!("value{}", i));
    }

    assert_eq!(source.len(), 1000);

    for i in 0..1000 {
        if i % 2 == 0 {
            source.remove(&format!("KEY{}", i));
        }
    }

    assert_eq!(source.len(), 500);
}

#[test]
fn test_memory_source_set_after_load() {
    let source = MemorySource::new();

    source.set("KEY1", "value1");
    let snapshot1 = source.load().unwrap();
    assert_eq!(snapshot1.variables.len(), 1);

    source.set("KEY2", "value2");
    let snapshot2 = source.load().unwrap();
    assert_eq!(snapshot2.variables.len(), 2);
}

#[test]
fn test_memory_source_remove_and_set() {
    let source = MemorySource::new();

    source.set("KEY1", "value1");
    source.set("KEY2", "value2");

    source.remove("KEY1");
    assert_eq!(source.len(), 1);

    source.set("KEY1", "new-value");
    assert_eq!(source.len(), 2);
}

#[test]
fn test_memory_source_clone_behavior() {
    let source1 = MemorySource::new();
    let source2 = MemorySource::new();

    source1.set("KEY1", "value1");
    source2.set("KEY2", "value2");

    let snapshot1 = source1.load().unwrap();
    let snapshot2 = source2.load().unwrap();

    assert_eq!(snapshot1.variables.len(), 1);
    assert_eq!(snapshot2.variables.len(), 1);
    assert_ne!(
        snapshot1.variables[0].key.as_str(),
        snapshot2.variables[0].key.as_str()
    );
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
fn test_memory_source_value_types() {
    let source = MemorySource::new();

    source.set("STRING_KEY", "string");
    source.set("NUMERIC_KEY", "12345");
    source.set("EMPTY_KEY", "");
    source.set("WHITESPACE_KEY", "   ");

    let snapshot = source.load().unwrap();
    assert_eq!(snapshot.variables.len(), 4);
}

#[test]
fn test_memory_source_key_case_sensitivity() {
    let source = MemorySource::new();

    source.set("lowercase_key", "value1");
    source.set("UPPERCASE_KEY", "value2");
    source.set("MixedCase_Key", "value3");

    let snapshot = source.load().unwrap();
    assert_eq!(snapshot.variables.len(), 3);

    let keys: Vec<_> = snapshot.variables.iter().map(|v| v.key.as_str()).collect();
    assert!(keys.contains(&"lowercase_key"));
    assert!(keys.contains(&"UPPERCASE_KEY"));
    assert!(keys.contains(&"MixedCase_Key"));
}
