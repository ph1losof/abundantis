use super::traits::*;
use super::variable::{ParsedVariable, VariableSource};
use crate::error::SourceError;
use compact_str::CompactString;
use parking_lot::Mutex;
use std::collections::HashMap;

#[cfg(feature = "shell")]
pub struct ShellSource {
    id: SourceId,
    cached: Mutex<Option<HashMap<String, String>>>,
}

#[cfg(feature = "shell")]
impl ShellSource {
    pub fn new() -> Self {
        Self {
            id: SourceId::new("shell:process"),
            cached: Mutex::new(None),
        }
    }

    pub fn refresh(&self) {
        *self.cached.lock() = None;
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

        Ok(SourceSnapshot {
            source_id: self.id.clone(),
            variables: vars.into(),
            timestamp: std::time::Instant::now(),
            version: None,
        })
    }

    fn has_changed(&self) -> bool {
        false
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
