use dashmap::DashMap;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Clone)]
pub struct PathCache {
    resolved: Arc<DashMap<PathBuf, PathBuf>>,

    fallback: Arc<RwLock<HashMap<PathBuf, PathBuf>>>,

    stats: Arc<RwLock<CacheStats>>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct CacheStats {
    hits: usize,
    misses: usize,
    errors: usize,
}

impl PathCache {
    pub fn new() -> Self {
        Self {
            resolved: Arc::new(DashMap::new()),
            fallback: Arc::new(RwLock::new(HashMap::new())),
            stats: Arc::new(RwLock::new(CacheStats::default())),
        }
    }

    pub fn canonicalize(&self, path: &Path) -> PathBuf {
        let path_buf = path.to_path_buf();

        if let Some(cached) = self.resolved.get(&path_buf) {
            self.stats.write().hits += 1;
            return cached.clone();
        }

        {
            let fallback = self.fallback.read();
            if let Some(cached) = fallback.get(&path_buf) {
                self.stats.write().hits += 1;
                return cached.clone();
            }
        }

        self.stats.write().misses += 1;
        let resolved = match path.canonicalize() {
            Ok(c) => {
                self.resolved.insert(path_buf.clone(), c.clone());
                c
            }
            Err(_) => {
                self.stats.write().errors += 1;

                self.fallback
                    .write()
                    .insert(path_buf.clone(), path_buf.clone());
                path_buf
            }
        };

        resolved
    }

    pub fn canonicalize_many(&self, paths: &[PathBuf]) -> Vec<PathBuf> {
        paths.iter().map(|p| self.canonicalize(p)).collect()
    }

    pub fn invalidate(&self, path: &Path) {
        let path_buf = path.to_path_buf();
        self.resolved.remove(&path_buf);
        self.fallback.write().remove(&path_buf);
    }

    pub fn clear(&self) {
        self.resolved.clear();
        self.fallback.write().clear();
        self.stats.write().hits = 0;
        self.stats.write().misses = 0;
        self.stats.write().errors = 0;
    }

    pub fn stats(&self) -> CacheStats {
        let stats = self.stats.read();
        CacheStats {
            hits: stats.hits,
            misses: stats.misses,
            errors: stats.errors,
        }
    }

    pub fn len(&self) -> usize {
        self.resolved.len() + self.fallback.read().len()
    }

    pub fn is_empty(&self) -> bool {
        self.resolved.is_empty() && self.fallback.read().is_empty()
    }

    pub fn hit_rate(&self) -> f64 {
        let stats = self.stats.read();
        let total = stats.hits + stats.misses;
        if total == 0 {
            return 0.0;
        }
        stats.hits as f64 / total as f64
    }
}

impl Default for PathCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canonicalize_cache() {
        let cache = PathCache::new();

        let path = PathBuf::from(".");
        let result1 = cache.canonicalize(&path);
        let stats = cache.stats();
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.hits, 0);

        let result2 = cache.canonicalize(&path);
        assert_eq!(result1, result2);
        let stats = cache.stats();
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.hits, 1);
    }

    #[test]
    fn test_hit_rate() {
        let cache = PathCache::new();
        let path = PathBuf::from(".");

        for _ in 0..10 {
            cache.canonicalize(&path);
        }

        let rate = cache.hit_rate();
        assert!(rate >= 0.9);
    }

    #[test]
    fn test_invalidate() {
        let cache = PathCache::new();
        let path = PathBuf::from(".");

        cache.canonicalize(&path);
        assert!(!cache.is_empty());

        cache.invalidate(&path);
        assert!(cache.is_empty() || cache.len() == 1);
    }

    #[test]
    fn test_clear() {
        let cache = PathCache::new();
        cache.canonicalize(&PathBuf::from("."));
        cache.canonicalize(&PathBuf::from(".."));

        assert!(!cache.is_empty());
        cache.clear();
        assert!(cache.is_empty());
        assert_eq!(cache.stats().hits, 0);
    }

    #[test]
    fn test_nonexistent_path() {
        let cache = PathCache::new();
        let path = PathBuf::from("/nonexistent/path/that/does/not/exist");

        let result = cache.canonicalize(&path);
        assert_eq!(result, path);

        let stats = cache.stats();
        assert_eq!(stats.errors, 1);
    }
}
