use abundantis::source::{
    ParsedVariable, Priority, SourceCapabilities, SourceId, SourceSnapshot, SourceType,
    VariableSource,
};
use std::path::PathBuf;

#[test]
fn test_source_id_creation() {
    let id = SourceId::new("test-source");
    assert_eq!(id.as_str(), "test-source");

    let id2: SourceId = "test-source".into();
    assert_eq!(id, id2);

    let id3: SourceId = String::from("test-source").into();
    assert_eq!(id, id3);
}

#[test]
fn test_source_id_display() {
    let id = SourceId::new("test");
    let s = format!("{}", id);
    assert_eq!(s, "test");
}

#[test]
fn test_source_priorities() {
    assert!(Priority::SHELL > Priority::FILE);
    assert!(Priority::FILE > Priority::MEMORY);
    assert_eq!(Priority::SHELL.0, 100);
    assert_eq!(Priority::FILE.0, 50);
    assert_eq!(Priority::MEMORY.0, 30);
    assert_eq!(Priority::REMOTE.0, 75);
}

#[test]
fn test_source_capabilities() {
    let caps = SourceCapabilities::READ | SourceCapabilities::CACHEABLE;
    assert!(caps.contains(SourceCapabilities::READ));
    assert!(caps.contains(SourceCapabilities::CACHEABLE));
    assert!(!caps.contains(SourceCapabilities::WRITE));
    assert!(!caps.contains(SourceCapabilities::ASYNC_ONLY));
}

#[test]
fn test_source_capabilities_defaults() {
    let caps = SourceCapabilities::default();
    assert!(caps.contains(SourceCapabilities::READ));
    assert!(caps.contains(SourceCapabilities::CACHEABLE));
    assert!(!caps.contains(SourceCapabilities::WRITE));
}

#[test]
fn test_source_type_equality() {
    assert_eq!(SourceType::File, SourceType::File);
    assert_eq!(SourceType::Shell, SourceType::Shell);
    assert_eq!(SourceType::Memory, SourceType::Memory);
    assert_eq!(SourceType::Remote, SourceType::Remote);
}

#[test]
fn test_source_type_hashing() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(SourceType::File);
    set.insert(SourceType::Shell);
    set.insert(SourceType::Memory);
    set.insert(SourceType::Remote);
    assert_eq!(set.len(), 4);
}

#[test]
fn test_parsed_variable_simple() {
    let var = ParsedVariable::simple("KEY", "value", VariableSource::Memory);

    assert_eq!(var.key.as_str(), "KEY");
    assert_eq!(var.raw_value.as_str(), "value");
    assert!(matches!(var.source, VariableSource::Memory));
    assert_eq!(var.description, None);
    assert!(!var.is_commented);
}

#[test]
fn test_parsed_variable_with_description() {
    let var = ParsedVariable {
        key: "KEY".into(),
        raw_value: "value".into(),
        source: VariableSource::Memory,
        description: Some("Test description".into()),
        is_commented: false,
    };

    assert_eq!(var.description.as_deref(), Some("Test description"));
}

#[test]
fn test_variable_source_file() {
    let source = VariableSource::File {
        path: PathBuf::from("/path/to/.env"),
        offset: 42,
    };

    assert_eq!(source.file_path(), Some(&PathBuf::from("/path/to/.env")));
    assert_eq!(
        source.file_path().unwrap().display().to_string(),
        "/path/to/.env"
    );
}

#[test]
fn test_variable_source_shell() {
    let source = VariableSource::Shell;
    assert_eq!(source.file_path(), None);
}

#[test]
fn test_variable_source_remote() {
    let source = VariableSource::Remote {
        provider: "vault".into(),
        path: Some("secret/path".into()),
    };

    assert_eq!(source.file_path(), None);
}

#[test]
fn test_source_snapshot_creation() {
    let vars = vec![
        ParsedVariable::simple("KEY1", "val1", VariableSource::Memory),
        ParsedVariable::simple("KEY2", "val2", VariableSource::Memory),
    ];

    let snapshot = SourceSnapshot {
        source_id: SourceId::new("test"),
        variables: vars.into(),
        timestamp: std::time::Instant::now(),
        version: Some(1),
    };

    assert_eq!(snapshot.variables.len(), 2);
    assert_eq!(snapshot.source_id.as_str(), "test");
    assert_eq!(snapshot.version, Some(1));
    assert!(snapshot.timestamp.elapsed().as_secs() < 1);
}

#[test]
fn test_source_snapshot_arc_conversion() {
    let vars = vec![ParsedVariable::simple(
        "KEY",
        "value",
        VariableSource::Memory,
    )];

    let snapshot = SourceSnapshot {
        source_id: SourceId::new("test"),
        variables: vars.into(),
        timestamp: std::time::Instant::now(),
        version: None,
    };

    let arc_vars = std::sync::Arc::clone(&snapshot.variables);
    assert_eq!(arc_vars.len(), 1);
    assert_eq!(arc_vars[0].key.as_str(), "KEY");
}

#[test]
fn test_source_id_clone() {
    let id1 = SourceId::new("test");
    let id2 = id1.clone();
    assert_eq!(id1, id2);
    assert_eq!(id1.as_str(), id2.as_str());
}

#[test]
fn test_source_id_hash() {
    use std::collections::HashMap;
    let id1 = SourceId::new("test1");
    let id2 = SourceId::new("test2");

    let mut map = HashMap::new();
    map.insert(id1.clone(), "value1");
    map.insert(id2.clone(), "value2");

    assert_eq!(map.get(&id1), Some(&"value1"));
    assert_eq!(map.get(&id2), Some(&"value2"));
}

#[test]
fn test_priority_ordering() {
    let priorities = [Priority::MEMORY, Priority::FILE, Priority::SHELL];

    let mut sorted = priorities.to_vec();
    sorted.sort();

    assert_eq!(sorted[0], Priority::MEMORY);
    assert_eq!(sorted[1], Priority::FILE);
    assert_eq!(sorted[2], Priority::SHELL);
}

#[test]
fn test_priority_comparison() {
    assert!(Priority::SHELL > Priority::FILE);
    assert!(Priority::FILE > Priority::MEMORY);
    assert!(Priority::REMOTE > Priority::FILE);
    assert!(Priority::SHELL > Priority::MEMORY);

    assert_eq!(Priority::SHELL, Priority::SHELL);
    assert_ne!(Priority::SHELL, Priority::FILE);
}

#[test]
fn test_source_capabilities_combinations() {
    let _all_caps = SourceCapabilities::all();

    let read_caps = SourceCapabilities::READ;
    let write_caps = SourceCapabilities::WRITE;
    let combined = read_caps | write_caps;

    assert!(combined.contains(SourceCapabilities::READ));
    assert!(combined.contains(SourceCapabilities::WRITE));
    assert!(!combined.contains(SourceCapabilities::WATCH));
}

#[test]
fn test_source_capabilities_bits() {
    let caps = SourceCapabilities::READ | SourceCapabilities::WRITE | SourceCapabilities::WATCH;
    assert_eq!(caps.bits(), 0b00000111);
}

#[test]
fn test_variable_source_debug() {
    let source = VariableSource::File {
        path: PathBuf::from("/test/.env"),
        offset: 42,
    };

    let debug_str = format!("{:?}", source);
    assert!(debug_str.contains("File"));
    assert!(debug_str.contains("/test/.env"));
}

#[test]
fn test_parsed_variable_debug() {
    let var = ParsedVariable::simple("KEY", "value", VariableSource::Memory);
    let debug_str = format!("{:?}", var);
    assert!(debug_str.contains("KEY"));
    assert!(debug_str.contains("value"));
    assert!(debug_str.contains("Memory"));
}

#[test]
fn test_source_snapshot_display() {
    let snapshot = SourceSnapshot {
        source_id: SourceId::new("test-source"),
        variables: vec![].into(),
        timestamp: std::time::Instant::now(),
        version: Some(42),
    };

    let display = format!("{:?}", snapshot);
    assert!(display.contains("test-source"));
}

#[test]
fn test_multiple_source_ids() {
    let ids = [
        SourceId::new("source1"),
        SourceId::new("source2"),
        SourceId::new("source3"),
    ];

    assert_eq!(ids.len(), 3);
    assert_ne!(ids[0], ids[1]);
    assert_ne!(ids[1], ids[2]);
    assert_ne!(ids[0], ids[2]);
}

#[test]
fn test_source_id_with_special_chars() {
    let id = SourceId::new("my-source_v1");
    assert_eq!(id.as_str(), "my-source_v1");
}

#[test]
fn test_source_timestamp_creation() {
    let snapshot = SourceSnapshot {
        source_id: SourceId::new("test"),
        variables: vec![].into(),
        timestamp: std::time::Instant::now(),
        version: None,
    };

    let elapsed = snapshot.timestamp.elapsed();
    assert!(elapsed.as_millis() < 100);
}

#[test]
fn test_variable_source_equality() {
    let source1 = VariableSource::Shell;
    let source2 = VariableSource::Shell;
    assert_eq!(source1, source2);

    let source3 = VariableSource::Memory;
    assert_ne!(source1, source3);
}

#[test]
fn test_parsed_variable_with_commented() {
    let var = ParsedVariable {
        key: "KEY".into(),
        raw_value: "value".into(),
        source: VariableSource::Memory,
        description: None,
        is_commented: true,
    };

    assert!(var.is_commented);
}

#[test]
fn test_source_version_tracking() {
    let snapshot1 = SourceSnapshot {
        source_id: SourceId::new("test"),
        variables: vec![].into(),
        timestamp: std::time::Instant::now(),
        version: Some(1),
    };

    let snapshot2 = SourceSnapshot {
        source_id: SourceId::new("test"),
        variables: vec![].into(),
        timestamp: std::time::Instant::now(),
        version: Some(2),
    };

    assert!(snapshot1.version.unwrap() < snapshot2.version.unwrap());
}

#[test]
fn test_source_capabilities_iterators() {
    let caps = SourceCapabilities::READ | SourceCapabilities::WRITE;

    assert!(caps.contains(SourceCapabilities::READ));
    assert!(caps.contains(SourceCapabilities::WRITE));
}

#[test]
fn test_priority_ord() {
    assert!(Priority::SHELL > Priority::FILE);
    assert!(Priority::FILE > Priority::MEMORY);
    assert!(Priority::REMOTE > Priority::FILE);

    let mut vec = vec![Priority::FILE, Priority::SHELL, Priority::MEMORY];
    vec.sort();

    assert_eq!(vec, vec![Priority::MEMORY, Priority::FILE, Priority::SHELL]);
}

#[test]
fn test_source_id_partial_eq() {
    let id = SourceId::new("test-source-123");
    assert!(id.as_str().starts_with("test-source"));
    assert!(id.as_str().ends_with("123"));
}
