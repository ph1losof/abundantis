use super::traits::*;
use super::variable::{ParsedVariable, VariableSource};
use crate::error::SourceError;
use ahash::AHasher;
use compact_str::CompactString;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

#[cfg(feature = "shell")]
pub struct ShellSource {
    id: SourceId,
    cached: Mutex<Option<HashMap<String, String>>>,
    cached_hash: Mutex<Option<u64>>,
}

#[cfg(feature = "shell")]
impl ShellSource {
    pub fn new() -> Self {
        Self {
            id: SourceId::new("shell:process"),
            cached: Mutex::new(None),
            cached_hash: Mutex::new(None),
        }
    }

    pub fn refresh(&self) {
        *self.cached.lock() = None;
        *self.cached_hash.lock() = None;
    }

    fn get_env(&self) -> HashMap<String, String> {
        let mut cache = self.cached.lock();
        if let Some(ref env) = *cache {
            return env.clone();
        }

        let env: HashMap<String, String> = std::env::vars().collect();
        *cache = Some(env.clone());
        env
    }

    fn compute_env_hash(&self) -> u64 {
        let mut hasher = AHasher::default();
        let mut vars: Vec<_> = std::env::vars().collect();
        // Sort for consistent hashing
        vars.sort_by(|a, b| a.0.cmp(&b.0));
        for (key, value) in vars {
            key.hash(&mut hasher);
            value.hash(&mut hasher);
        }
        hasher.finish()
    }
}

#[cfg(feature = "shell")]
impl Default for ShellSource {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "shell")]
impl EnvSource for ShellSource {
    fn id(&self) -> &SourceId {
        &self.id
    }

    fn source_type(&self) -> SourceType {
        SourceType::Shell
    }

    fn priority(&self) -> Priority {
        Priority::SHELL
    }

    fn capabilities(&self) -> SourceCapabilities {
        SourceCapabilities::READ | SourceCapabilities::CACHEABLE
    }

    fn load(&self) -> Result<SourceSnapshot, SourceError> {
        let env = self.get_env();
        let vars: Vec<ParsedVariable> = env
            .into_iter()
            .map(|(key, value)| ParsedVariable {
                key: CompactString::new(&key),
                raw_value: CompactString::new(&value),
                source: VariableSource::Shell,
                description: None,
                is_commented: false,
            })
            .collect();

        // Update cached hash after loading
        *self.cached_hash.lock() = Some(self.compute_env_hash());

        Ok(SourceSnapshot {
            source_id: self.id.clone(),
            variables: vars.into(),
            timestamp: std::time::Instant::now(),
            version: None,
        })
    }

    fn has_changed(&self) -> bool {
        let current_hash = self.compute_env_hash();
        *self.cached_hash.lock() != Some(current_hash)
    }

    fn invalidate(&self) {
        self.refresh();
    }
}

#[cfg(feature = "shell")]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_source() {
        std::env::set_var("CORNCOPIA_TEST_VAR", "test_value");

        let source = ShellSource::new();
        let snapshot = source.load().unwrap();

        let test_var = snapshot
            .variables
            .iter()
            .find(|v| v.key.as_str() == "CORNCOPIA_TEST_VAR");
        assert!(test_var.is_some());
        assert_eq!(test_var.unwrap().raw_value.as_str(), "test_value");

        std::env::remove_var("CORNCOPIA_TEST_VAR");
    }

    #[test]
    fn test_priority() {
        let source = ShellSource::new();
        assert_eq!(source.priority(), Priority::SHELL);
    }
}
