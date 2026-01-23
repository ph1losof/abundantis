use super::traits::*;
use super::variable::{ParsedVariable, VariableSource};
use crate::error::SourceError;
use compact_str::CompactString;
use parking_lot::Mutex;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[cfg(feature = "file")]
pub struct FileSource {
    path: PathBuf,
    id: SourceId,
    last_modified: Mutex<Option<SystemTime>>,
    cached_vars: Mutex<Option<Vec<ParsedVariable>>>,
    version: Mutex<Option<u64>>,
    next_version: Mutex<u64>,
}

#[cfg(feature = "file")]
impl FileSource {
    pub fn new(path: impl AsRef<Path>) -> Result<Self, std::io::Error> {
        let path = path.as_ref().to_path_buf();

        if !path.exists() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("File not found: {}", path.display()),
            ));
        }

        let id = SourceId::new(format!("file:{}", path.display()));

        Ok(Self {
            path,
            id,
            last_modified: Mutex::new(None),
            cached_vars: Mutex::new(None),
            version: Mutex::new(None),
            next_version: Mutex::new(1),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn reload(&self) -> Result<(), std::io::Error> {
        *self.cached_vars.lock() = None;
        self.load().map_err(|e| match e {
            SourceError::SourceRead { reason, .. } => std::io::Error::other(reason),
            _ => std::io::Error::other(e.to_string()),
        })?;
        Ok(())
    }

    fn parse_file(&self) -> Result<Vec<ParsedVariable>, SourceError> {
        let content = std::fs::read_to_string(&self.path).map_err(|e| SourceError::SourceRead {
            source_name: self.path.display().to_string(),
            reason: e.to_string(),
        })?;

        if let Ok(metadata) = self.path.metadata() {
            if let Ok(modified) = metadata.modified() {
                *self.last_modified.lock() = Some(modified);
            }
        }

        let parsed = korni::parse_with_options(
            &content,
            korni::ParseOptions {
                track_positions: true,
                include_comments: false,
            },
        );
        let mut variables = Vec::with_capacity(parsed.len());

        for entry in parsed {
            if let korni::Entry::Pair(kv) = entry {
                let description = None;
                let offset = kv.key_span.map(|s| s.start.offset).unwrap_or(0);

                variables.push(ParsedVariable {
                    key: CompactString::new(&kv.key),
                    raw_value: CompactString::new(&kv.value),
                    source: VariableSource::File {
                        path: self.path.clone(),
                        offset,
                    },
                    description,
                    is_commented: kv.is_comment,
                });
            }
        }

        Ok(variables)
    }

    fn check_modified(&self) -> bool {
        let last = self.last_modified.lock();
        if last.is_none() {
            return true;
        }

        if let Ok(metadata) = self.path.metadata() {
            if let Ok(current) = metadata.modified() {
                return Some(current) != *last;
            }
        }

        true
    }

    pub fn set_variable(
        &self,
        key: impl Into<CompactString>,
        value: impl Into<CompactString>,
    ) -> Result<(), SourceError> {
        let key = key.into();
        let value = value.into();

        let content = std::fs::read_to_string(&self.path).map_err(|e| SourceError::SourceRead {
            source_name: self.path.display().to_string(),
            reason: e.to_string(),
        })?;

        let mut lines: Vec<String> = content.lines().map(|s| s.to_string()).collect();
        let mut key_found = false;
        let key_str = key.as_str();

        for (idx, line) in lines.iter_mut().enumerate() {
            if let Some(equal_pos) = line.find('=') {
                let line_key = &line[..equal_pos].trim();
                if *line_key == key_str {
                    let prefix = &line[..=equal_pos];
                    let mut new_line = String::with_capacity(prefix.len() + value.len());
                    new_line.push_str(prefix);
                    new_line.push_str(value.as_str());
                    lines[idx] = new_line;
                    key_found = true;
                    break;
                }
            }
        }

        if !key_found {
            return Err(SourceError::UnsupportedOperation {
                operation: "set_variable".into(),
                source_type: "FileSource".into(),
                reason: format!("Key '{}' not found in file", key_str),
            });
        }

        let new_content = lines.join("\n");
        std::fs::write(&self.path, new_content).map_err(|e| SourceError::SourceRead {
            source_name: self.path.display().to_string(),
            reason: format!("Failed to write file: {}", e),
        })?;

        *self.cached_vars.lock() = None;
        {
            let mut next = self.next_version.lock();
            *next += 1;
        }

        Ok(())
    }

    pub fn remove_variable(
        &self,
        key: impl Into<CompactString>,
    ) -> Result<ParsedVariable, SourceError> {
        let key = key.into();
        let key_str = key.as_str();

        let content = std::fs::read_to_string(&self.path).map_err(|e| SourceError::SourceRead {
            source_name: self.path.display().to_string(),
            reason: e.to_string(),
        })?;

        let vars = self.parse_file()?;
        let removed = vars.iter().find(|v| v.key.as_str() == key_str).cloned();

        let removed = match removed {
            Some(v) => v,
            None => {
                return Err(SourceError::UnsupportedOperation {
                    operation: "remove_variable".into(),
                    source_type: "FileSource".into(),
                    reason: format!("Key '{}' not found in file", key_str),
                });
            }
        };

        let lines: Vec<String> = content
            .lines()
            .filter(|line| {
                if let Some(equal_pos) = line.find('=') {
                    let line_key = &line[..equal_pos].trim();
                    *line_key != key_str
                } else {
                    true
                }
            })
            .map(|s| s.to_string())
            .collect();

        let new_content = lines.join("\n");
        std::fs::write(&self.path, new_content).map_err(|e| SourceError::SourceRead {
            source_name: self.path.display().to_string(),
            reason: format!("Failed to write file: {}", e),
        })?;

        *self.cached_vars.lock() = None;
        {
            let mut next = self.next_version.lock();
            *next += 1;
        }

        Ok(removed)
    }

    pub fn get_path(&self) -> &Path {
        &self.path
    }

    pub fn increment_version(&self) -> u64 {
        let mut next = self.next_version.lock();
        let v = *next;
        *next += 1;
        v
    }

    pub fn get_version(&self) -> Option<u64> {
        *self.version.lock()
    }
}

#[cfg(feature = "file")]
impl EnvSource for FileSource {
    fn id(&self) -> &SourceId {
        &self.id
    }

    fn source_type(&self) -> SourceType {
        SourceType::File
    }

    fn priority(&self) -> Priority {
        Priority::FILE
    }

    fn capabilities(&self) -> SourceCapabilities {
        SourceCapabilities::READ | SourceCapabilities::WATCH | SourceCapabilities::CACHEABLE
    }

    fn load(&self) -> Result<SourceSnapshot, SourceError> {
        let version = {
            let cache = self.cached_vars.lock();
            if let Some(vars) = cache.as_ref() {
                if !self.check_modified() {
                    let v = *self.version.lock();
                    return Ok(SourceSnapshot {
                        source_id: self.id.clone(),
                        variables: vars.clone().into(),
                        timestamp: std::time::Instant::now(),
                        version: v,
                    });
                }
            }

            let mut next = self.next_version.lock();
            let v = *next;
            *next += 1;
            drop(next);
            v
        };

        let vars = self.parse_file()?;
        *self.cached_vars.lock() = Some(vars.clone());
        *self.version.lock() = Some(version);

        Ok(SourceSnapshot {
            source_id: self.id.clone(),
            variables: vars.into(),
            timestamp: std::time::Instant::now(),
            version: Some(version),
        })
    }

    fn has_changed(&self) -> bool {
        self.check_modified()
    }

    fn invalidate(&self) {
        *self.cached_vars.lock() = None;
        *self.last_modified.lock() = None;
    }
}

#[cfg(feature = "file")]
#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_parse_simple_file() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "KEY=value").unwrap();
        writeln!(file, "OTHER=123").unwrap();

        let source = FileSource::new(file.path()).unwrap();
        let snapshot = source.load().unwrap();

        assert_eq!(snapshot.variables.len(), 2);
        assert_eq!(snapshot.variables[0].key.as_str(), "KEY");
        assert_eq!(snapshot.variables[0].raw_value.as_str(), "value");
    }

    #[test]
    fn test_caching() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "KEY=value").unwrap();

        let source = FileSource::new(file.path()).unwrap();
        let _ = source.load().unwrap();

        assert!(!source.has_changed());
    }

    #[test]
    fn test_version_tracking() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "KEY=value").unwrap();

        let source = FileSource::new(file.path()).unwrap();
        let snapshot1 = source.load().unwrap();
        let v1 = snapshot1.version;

        writeln!(file, "KEY=updated").unwrap();
        let snapshot2 = source.load().unwrap();
        let v2 = snapshot2.version;

        assert!(v1.is_some());
        assert!(v2.is_some());
        assert_ne!(v1, v2);
        assert!(v2.unwrap() > v1.unwrap());
    }

    #[test]
    fn test_set_variable() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "KEY=value1").unwrap();
        writeln!(file, "OTHER=123").unwrap();

        let source = FileSource::new(file.path()).unwrap();

        source.set_variable("KEY", "value2").unwrap();

        let content = std::fs::read_to_string(file.path()).unwrap();
        assert!(content.contains("KEY=value2"));
        assert!(!content.contains("KEY=value1"));
    }

    #[test]
    fn test_remove_variable() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, "KEY=value1").unwrap();
        writeln!(file, "OTHER=123").unwrap();

        let source = FileSource::new(file.path()).unwrap();

        let removed = source.remove_variable("KEY").unwrap();
        assert_eq!(removed.key.as_str(), "KEY");

        let content = std::fs::read_to_string(file.path()).unwrap();
        assert!(!content.contains("KEY=value1"));
        assert!(content.contains("OTHER=123"));
    }
}
