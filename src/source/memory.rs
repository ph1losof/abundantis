use super::traits::*;
use super::variable::{ParsedVariable, VariableSource};
use crate::error::SourceError;
use compact_str::CompactString;
use indexmap::IndexMap;
use parking_lot::Mutex;

pub struct MemorySource {
    id: SourceId,
    variables: Mutex<IndexMap<CompactString, ParsedVariable>>,
    version: Mutex<u64>,
}

impl MemorySource {
    pub fn new() -> Self {
        Self {
            id: SourceId::new("memory"),
            variables: Mutex::new(IndexMap::new()),
            version: Mutex::new(0),
        }
    }

    pub fn set(&self, key: impl Into<CompactString>, value: impl Into<CompactString>) {
        let key = key.into();
        let value = value.into();
        let mut vars = self.variables.lock();
        vars.insert(
            key.clone(),
            ParsedVariable {
                key: key.clone(),
                raw_value: value,
                source: VariableSource::Memory,
                description: None,
                is_commented: false,
            },
        );
        *self.version.lock() += 1;
    }

    pub fn set_with_description(
        &self,
        key: impl Into<CompactString>,
        value: impl Into<CompactString>,
        description: impl Into<CompactString>,
    ) {
        let key = key.into();
        let value = value.into();
        let description = description.into();
        let mut vars = self.variables.lock();
        vars.insert(
            key.clone(),
            ParsedVariable {
                key: key.clone(),
                raw_value: value,
                source: VariableSource::Memory,
                description: Some(description),
                is_commented: false,
            },
        );
        *self.version.lock() += 1;
    }

    pub fn remove(&self, key: &str) -> Option<ParsedVariable> {
        let mut vars = self.variables.lock();
        let removed = vars.swap_remove(key);
        if removed.is_some() {
            *self.version.lock() += 1;
        }
        removed
    }

    pub fn clear(&self) {
        let mut vars = self.variables.lock();
        vars.clear();
        *self.version.lock() += 1;
    }

    pub fn len(&self) -> usize {
        self.variables.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.variables.lock().is_empty()
    }
}

impl Default for MemorySource {
    fn default() -> Self {
        Self::new()
    }
}

impl EnvSource for MemorySource {
    fn id(&self) -> &SourceId {
        &self.id
    }

    fn source_type(&self) -> SourceType {
        SourceType::Memory
    }

    fn priority(&self) -> Priority {
        Priority::MEMORY
    }

    fn capabilities(&self) -> SourceCapabilities {
        SourceCapabilities::READ | SourceCapabilities::WRITE | SourceCapabilities::CACHEABLE
    }

    fn load(&self) -> Result<SourceSnapshot, SourceError> {
        let vars: Vec<ParsedVariable> = self.variables.lock().values().cloned().collect();

        Ok(SourceSnapshot {
            source_id: self.id.clone(),
            variables: vars.into(),
            timestamp: std::time::Instant::now(),
            version: Some(*self.version.lock()),
        })
    }

    fn has_changed(&self) -> bool {
        true
    }

    fn invalidate(&self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_source() {
        let source = MemorySource::new();

        source.set("KEY1", "value1");
        source.set("KEY2", "value2");

        let snapshot = source.load().unwrap();
        assert_eq!(snapshot.variables.len(), 2);
        assert_eq!(snapshot.variables[0].key.as_str(), "KEY1");
        assert_eq!(snapshot.variables[0].raw_value.as_str(), "value1");
    }

    #[test]
    fn test_remove() {
        let source = MemorySource::new();

        source.set("KEY1", "value1");
        assert_eq!(source.len(), 1);

        let removed = source.remove("KEY1");
        assert!(removed.is_some());
        assert_eq!(source.len(), 0);
    }

    #[test]
    fn test_version() {
        let source = MemorySource::new();

        source.set("KEY1", "value1");
        let v1 = source.load().unwrap().version.unwrap();

        source.set("KEY2", "value2");
        let v2 = source.load().unwrap().version.unwrap();

        assert!(v2 > v1);
    }
}
