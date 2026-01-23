use crate::error::AbundantisError;
use crate::Result;
use compact_str::CompactString;
use dashmap::DashMap;
use lru::LruCache;
use parking_lot::RwLock;
use std::collections::{HashMap, HashSet};
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[cfg(feature = "async")]
use maybe_async::must_be_async;
#[cfg(not(feature = "async"))]
use maybe_async::must_be_sync;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct CacheKey {
    pub key: CompactString,
    pub context_hash: u64,
}

impl CacheKey {
    pub fn new(key: impl Into<CompactString>, context_hash: u64) -> Self {
        Self {
            key: key.into(),
            context_hash,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CachedValue {
    pub value: Arc<ResolvedVariable>,
    pub cached_at: Instant,
}

#[derive(Debug, Clone)]
pub struct ResolvedVariable {
    pub key: CompactString,
    pub raw_value: CompactString,
    pub resolved_value: CompactString,
    pub source: super::source::VariableSource,
    pub description: Option<CompactString>,
    pub has_warnings: bool,
    pub interpolation_depth: u32,
}

#[derive(Debug, Clone)]
pub struct DependencyEdge {
    pub from: CompactString,
    pub to: CompactString,
    pub span: Option<(u32, u32)>,
}

#[derive(Debug, Clone)]
pub struct DependencyGraph {
    edges: Vec<DependencyEdge>,
    nodes: HashMap<CompactString, Vec<DependencyEdge>>,
}

impl DependencyGraph {
    pub fn new() -> Self {
        Self {
            edges: Vec::new(),
            nodes: HashMap::new(),
        }
    }

    pub fn add_edge(&mut self, from: CompactString, to: CompactString, span: Option<(u32, u32)>) {
        let edge = DependencyEdge {
            from: from.clone(),
            to: to.clone(),
            span,
        };
        self.edges.push(edge.clone());
        self.nodes.entry(from).or_default().push(edge);
    }

    pub fn detect_cycle(&self, start: &str) -> Vec<CompactString> {
        let mut visited = HashMap::new();
        let mut path = Vec::new();
        self.detect_cycle_with_state(start, &mut visited, &mut path)
    }

    pub fn detect_cycle_with_state(
        &self,
        start: &str,
        visited: &mut HashMap<CompactString, bool>,
        path: &mut Vec<CompactString>,
    ) -> Vec<CompactString> {
        visited.clear();
        path.clear();

        if self.dfs_detect_cycle(start, visited, path) {
            std::mem::take(path)
        } else {
            Vec::new()
        }
    }

    fn dfs_detect_cycle(
        &self,
        current: &str,
        visited: &mut HashMap<CompactString, bool>,
        path: &mut Vec<CompactString>,
    ) -> bool {
        if let Some(&in_path) = visited.get(current) {
            if in_path {
                return true;
            }
            return false;
        }

        visited.insert(CompactString::new(current), true);
        path.push(CompactString::new(current));

        if let Some(edges) = self.nodes.get(current) {
            for edge in edges {
                if self.dfs_detect_cycle(&edge.to, visited, path) {
                    return true;
                }
            }
        }

        path.pop();
        visited.insert(CompactString::new(current), false);
        false
    }

    pub fn get_dependencies(&self, key: &str) -> Vec<CompactString> {
        self.nodes
            .get(key)
            .map(|edges| edges.iter().map(|e| e.to.clone()).collect())
            .unwrap_or_default()
    }

    pub fn clear(&mut self) {
        self.edges.clear();
        self.nodes.clear();
    }
}

impl Default for DependencyGraph {
    fn default() -> Self {
        Self::new()
    }
}

pub struct ResolutionCache {
    hot_cache: Arc<RwLock<LruCache<CacheKey, CachedValue>>>,
    ttl_cache: Arc<DashMap<CacheKey, CachedValue>>,
    ttl: Duration,
    enabled: bool,
}

impl ResolutionCache {
    pub fn new(config: &super::config::CacheConfig) -> Self {
        let hot_size = NonZeroUsize::new(config.hot_cache_size.max(1))
            .unwrap_or(NonZeroUsize::new(1000).unwrap());

        Self {
            hot_cache: Arc::new(RwLock::new(LruCache::new(hot_size))),
            ttl_cache: Arc::new(DashMap::new()),
            ttl: config.ttl,
            enabled: config.enabled,
        }
    }

    pub fn get(&self, key: &CacheKey) -> Option<Arc<ResolvedVariable>> {
        if !self.enabled {
            return None;
        }

        let now = Instant::now();
        let ttl = self.ttl;

        if let Some(cached) = self.ttl_cache.get(key) {
            if now.duration_since(cached.cached_at) < ttl {
                return Some(Arc::clone(&cached.value));
            }
        }

        self.ttl_cache
            .remove_if(key, |_, cached| now.duration_since(cached.cached_at) >= ttl);

        let mut hot = self.hot_cache.write();
        if let Some(cached) = hot.get(key) {
            if now.duration_since(cached.cached_at) < ttl {
                return Some(Arc::clone(&cached.value));
            }
        }

        None
    }

    pub fn insert(&self, key: CacheKey, value: Arc<ResolvedVariable>) {
        if !self.enabled {
            return;
        }

        let cached = CachedValue {
            value,
            cached_at: Instant::now(),
        };

        self.ttl_cache.insert(key.clone(), cached.clone());

        let mut hot = self.hot_cache.write();
        hot.put(key, cached);
    }

    pub fn invalidate(&self, key: &CacheKey) {
        if !self.enabled {
            return;
        }

        self.ttl_cache.remove(key);
        let mut hot = self.hot_cache.write();
        hot.pop(key);
    }

    pub fn clear(&self) {
        self.ttl_cache.clear();
        let mut hot = self.hot_cache.write();
        hot.clear();
    }

    pub fn len(&self) -> usize {
        if !self.enabled {
            return 0;
        }
        self.ttl_cache.len() + self.hot_cache.read().len()
    }

    pub fn is_empty(&self) -> bool {
        !self.enabled || self.len() == 0
    }

    pub fn cleanup_expired(&self) {
        if !self.enabled {
            return;
        }

        let now = Instant::now();
        self.ttl_cache
            .retain(|_, cached| now.duration_since(cached.cached_at) < self.ttl);

        let mut hot = self.hot_cache.write();
        let keys_to_remove: Vec<CacheKey> = hot
            .iter()
            .filter(|(_, cached)| now.duration_since(cached.cached_at) >= self.ttl)
            .map(|(k, _)| k.clone())
            .collect();

        for key in keys_to_remove {
            hot.pop(&key);
        }
    }
}

pub struct ResolutionEngine {
    resolution_config: parking_lot::RwLock<super::config::ResolutionConfig>,
    interpolation_config: parking_lot::RwLock<super::config::InterpolationConfig>,
    cache: Arc<ResolutionCache>,
    graph: Arc<parking_lot::RwLock<DependencyGraph>>,
    graph_version: Arc<AtomicU64>,
}

impl ResolutionEngine {
    pub fn new(
        resolution: &super::config::ResolutionConfig,
        interpolation: &super::config::InterpolationConfig,
        cache: &super::config::CacheConfig,
    ) -> Self {
        Self {
            resolution_config: parking_lot::RwLock::new(resolution.clone()),
            interpolation_config: parking_lot::RwLock::new(interpolation.clone()),
            cache: Arc::new(ResolutionCache::new(cache)),
            graph: Arc::new(parking_lot::RwLock::new(DependencyGraph::new())),
            graph_version: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn update_resolution_config(&self, config: super::config::ResolutionConfig) {
        *self.resolution_config.write() = config;
        self.cache.clear();
        tracing::info!("Resolution config updated at runtime");
    }

    pub fn update_interpolation_config(&self, config: super::config::InterpolationConfig) {
        *self.interpolation_config.write() = config;
        self.cache.clear();
        tracing::info!("Interpolation config updated at runtime");
    }

    pub fn interpolation_enabled(&self) -> bool {
        self.interpolation_config.read().enabled
    }

    pub fn precedence(&self) -> Vec<super::config::SourcePrecedence> {
        self.resolution_config.read().precedence.clone()
    }

    fn snapshots_version(&self, snapshots: &[crate::source::SourceSnapshot]) -> u64 {
        snapshots.iter().filter_map(|s| s.version).sum()
    }

    fn maybe_rebuild_graph(&self, snapshots: &[crate::source::SourceSnapshot]) -> Result<()> {
        let current_version = self.snapshots_version(snapshots);
        let last_version = self.graph_version.load(Ordering::SeqCst);

        if current_version != last_version {
            self.build_dependency_graph(snapshots)?;
            self.graph_version.store(current_version, Ordering::SeqCst);
        }
        Ok(())
    }

    fn filter_snapshots_ref<'a>(
        &self,
        snapshots: &'a [crate::source::SourceSnapshot],
        file_source_filter: Option<&HashSet<crate::source::SourceId>>,
    ) -> Vec<&'a crate::source::SourceSnapshot> {
        match file_source_filter {
            Some(filter) if !filter.is_empty() => snapshots
                .iter()
                .filter(|snapshot| {
                    let source_id_str = snapshot.source_id.as_str();
                    let is_file_source =
                        source_id_str.starts_with("file:") && source_id_str.len() > 5;
                    if is_file_source {
                        filter.contains(&snapshot.source_id)
                    } else {
                        true
                    }
                })
                .collect(),

            Some(_) => snapshots
                .iter()
                .filter(|snapshot| {
                    let source_id_str = snapshot.source_id.as_str();
                    let is_file_source =
                        source_id_str.starts_with("file:") && source_id_str.len() > 5;
                    !is_file_source
                })
                .collect(),

            None => snapshots.iter().collect(),
        }
    }

    fn filter_by_source_type<'a>(
        &self,
        snapshots: &[&'a crate::source::SourceSnapshot],
    ) -> Vec<&'a crate::source::SourceSnapshot> {
        let config = self.resolution_config.read();
        let precedence = &config.precedence;

        if precedence.is_empty() {
            return Vec::new();
        }

        snapshots
            .iter()
            .filter(|snapshot| {
                let source_id_str = snapshot.source_id.as_str();

                let source_type = if source_id_str.starts_with("file:") {
                    crate::config::SourcePrecedence::File
                } else if source_id_str == "shell" || source_id_str.starts_with("shell:") {
                    crate::config::SourcePrecedence::Shell
                } else if source_id_str.starts_with("remote:") {
                    crate::config::SourcePrecedence::Remote
                } else {
                    return true;
                };

                precedence.contains(&source_type)
            })
            .copied()
            .collect()
    }

    fn resolve_inner(
        &self,
        key: &str,
        context: &super::workspace::WorkspaceContext,
        snapshots: &[crate::source::SourceSnapshot],
    ) -> Result<Option<Arc<ResolvedVariable>>> {
        let sorted_snapshots = self.sort_snapshots_by_file_order(snapshots);

        let mut resolved = None;

        for snapshot in &sorted_snapshots {
            if let Some(variable) = snapshot.variables.iter().find(|v| v.key.as_str() == key) {
                resolved = Some(self.resolve_variable(
                    variable,
                    snapshots,
                    context,
                    0,
                    &mut Vec::new(),
                )?);
            }
        }

        if let Some(ref var) = resolved {
            if var.has_warnings && self.resolution_config.read().type_check {
                return Err(AbundantisError::CircularDependency {
                    chain: format!("Cycle detected resolving '{}'", key),
                });
            }

            let context_hash = self.hash_context(context);
            let cache_key = CacheKey {
                key: CompactString::new(key),
                context_hash,
            };
            self.cache.insert(cache_key, Arc::clone(var));
        }

        Ok(resolved)
    }

    fn sort_snapshots_by_file_order<'a>(
        &self,
        snapshots: &'a [crate::source::SourceSnapshot],
    ) -> Vec<&'a crate::source::SourceSnapshot> {
        let config = self.resolution_config.read();
        let file_order = &config.files.order;

        let mut sorted: Vec<_> = snapshots.iter().collect();
        sorted.sort_by(|a, b| {
            let a_order = self.get_file_order_index(&a.source_id, file_order);
            let b_order = self.get_file_order_index(&b.source_id, file_order);
            a_order.cmp(&b_order)
        });

        sorted
    }

    fn get_file_order_index(
        &self,
        source_id: &crate::source::SourceId,
        file_order: &[CompactString],
    ) -> usize {
        let source_str = source_id.as_str();
        if !source_str.starts_with("file:") {
            return 0;
        }

        let path = &source_str[5..];
        let filename = std::path::Path::new(path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");

        for (i, pattern) in file_order.iter().enumerate() {
            if filename == pattern.as_str() || path.ends_with(pattern.as_str()) {
                return i + 1;
            }
        }

        file_order.len() + 1
    }

    fn sort_snapshot_refs_by_file_order<'a>(
        &self,
        snapshots: &[&'a crate::source::SourceSnapshot],
    ) -> Vec<&'a crate::source::SourceSnapshot> {
        let config = self.resolution_config.read();
        let file_order = &config.files.order;

        let mut sorted: Vec<_> = snapshots.iter().copied().collect();
        sorted.sort_by(|a, b| {
            let a_order = self.get_file_order_index(&a.source_id, file_order);
            let b_order = self.get_file_order_index(&b.source_id, file_order);
            a_order.cmp(&b_order)
        });

        sorted
    }

    fn all_variables_inner(
        &self,
        context: &super::workspace::WorkspaceContext,
        all_snapshots: &[crate::source::SourceSnapshot],
        filtered_snapshots: &[&crate::source::SourceSnapshot],
    ) -> Result<Vec<Arc<ResolvedVariable>>> {
        let type_filtered = self.filter_by_source_type(filtered_snapshots);

        let sorted = self.sort_snapshot_refs_by_file_order(&type_filtered);

        let mut seen_keys = std::collections::HashSet::new();
        let mut results = Vec::new();

        for snapshot in sorted {
            for variable in snapshot.variables.iter() {
                if !seen_keys.contains(&variable.key) {
                    let resolved = self.resolve_variable(
                        variable,
                        all_snapshots,
                        context,
                        0,
                        &mut Vec::new(),
                    )?;
                    results.push(resolved);
                    seen_keys.insert(variable.key.clone());
                }
            }
        }

        Ok(results)
    }

    #[cfg_attr(feature = "async", must_be_async)]
    #[cfg_attr(not(feature = "async"), must_be_sync)]
    pub async fn resolve(
        &self,
        key: &str,
        context: &super::workspace::WorkspaceContext,
        registry: &super::source::SourceRegistry,
    ) -> Result<Option<Arc<ResolvedVariable>>> {
        let context_hash = self.hash_context(context);
        let cache_key = CacheKey {
            key: CompactString::new(key),
            context_hash,
        };

        if let Some(cached) = self.cache.get(&cache_key) {
            return Ok(Some(cached));
        }

        let snapshots = registry.load_all().await.map_err(AbundantisError::Source)?;

        if self.resolution_config.read().type_check {
            self.maybe_rebuild_graph(&snapshots)?;
        }

        self.resolve_inner(key, context, &snapshots)
    }

    fn resolve_variable(
        &self,
        variable: &super::source::ParsedVariable,
        all_snapshots: &[crate::source::SourceSnapshot],
        context: &super::workspace::WorkspaceContext,
        depth: u32,
        visited: &mut Vec<CompactString>,
    ) -> Result<Arc<ResolvedVariable>> {
        let key = variable.key.clone();
        let interpolation_config = self.interpolation_config.read();
        let max_depth = interpolation_config.max_depth;

        if !interpolation_config.enabled {
            return Ok(Arc::new(ResolvedVariable {
                key: key.clone(),
                raw_value: variable.raw_value.clone(),
                resolved_value: variable.raw_value.clone(),
                source: variable.source.clone(),
                description: variable.description.clone(),
                has_warnings: false,
                interpolation_depth: 0,
            }));
        }

        if depth >= max_depth {
            return Err(AbundantisError::MaxDepthExceeded {
                key: key.as_str().to_string(),
                depth,
            });
        }

        if visited.contains(&key) {
            return Err(AbundantisError::CircularDependency {
                chain: visited
                    .iter()
                    .map(|k| k.as_str())
                    .collect::<Vec<_>>()
                    .join(" -> "),
            });
        }

        visited.push(key.clone());

        let resolved_value = self.interpolate_value_lazy(
            &variable.raw_value,
            all_snapshots,
            context,
            depth + 1,
            visited,
        );

        visited.pop();

        Ok(Arc::new(ResolvedVariable {
            key,
            raw_value: variable.raw_value.clone(),
            resolved_value,
            source: variable.source.clone(),
            description: variable.description.clone(),
            has_warnings: false,
            interpolation_depth: depth,
        }))
    }

    fn interpolate_value_lazy(
        &self,
        value: &str,
        all_snapshots: &[crate::source::SourceSnapshot],
        _context: &super::workspace::WorkspaceContext,
        depth: u32,
        visited: &mut Vec<CompactString>,
    ) -> CompactString {
        let interpolation_config = self.interpolation_config.read();
        let max_depth = interpolation_config.max_depth;

        if depth >= max_depth || !interpolation_config.enabled {
            return CompactString::new(value);
        }

        let mut germi = germi::Germi::with_config(germi::Config {
            max_depth: (max_depth - depth) as usize,
            ..Default::default()
        });

        let references = self.find_variable_references(value);
        for ref_key in references {
            if visited.contains(&ref_key) {
                continue;
            }

            for snapshot in all_snapshots {
                if let Some(variable) = snapshot.variables.iter().find(|v| v.key == ref_key) {
                    let resolved_value = self.interpolate_value_lazy(
                        &variable.raw_value,
                        all_snapshots,
                        _context,
                        depth + 1,
                        visited,
                    );
                    germi.add_variable(variable.key.as_str(), resolved_value.as_str());
                    break;
                }
            }
        }

        match germi.interpolate(value) {
            Ok(interpolated) => CompactString::new(interpolated.as_ref()),
            Err(e) => {
                tracing::warn!(
                    value = %value,
                    depth = %depth,
                    error = %e,
                    "Interpolation failed, returning original value"
                );
                CompactString::new(value)
            }
        }
    }

    #[cfg_attr(feature = "async", must_be_async)]
    #[cfg_attr(not(feature = "async"), must_be_sync)]
    pub async fn all_variables(
        &self,
        context: &super::workspace::WorkspaceContext,
        registry: &super::source::SourceRegistry,
    ) -> Result<Vec<Arc<ResolvedVariable>>> {
        let snapshots = registry.load_all().await.map_err(AbundantisError::Source)?;

        if self.resolution_config.read().type_check {
            self.maybe_rebuild_graph(&snapshots)?;
        }

        self.all_variables_inner(context, &snapshots, &snapshots.iter().collect::<Vec<_>>())
    }

    #[cfg_attr(feature = "async", must_be_async)]
    #[cfg_attr(not(feature = "async"), must_be_sync)]
    pub async fn resolve_with_filter(
        &self,
        key: &str,
        context: &super::workspace::WorkspaceContext,
        registry: &super::source::SourceRegistry,
        file_source_filter: Option<&HashSet<super::source::SourceId>>,
    ) -> Result<Option<Arc<ResolvedVariable>>> {
        let context_hash = self.hash_context(context);
        let cache_key = CacheKey {
            key: CompactString::new(key),
            context_hash,
        };

        if let Some(cached) = self.cache.get(&cache_key) {
            return Ok(Some(cached));
        }

        let snapshots = registry.load_all().await.map_err(AbundantisError::Source)?;
        let filtered_refs = self.filter_snapshots_ref(&snapshots, file_source_filter);

        let type_filtered = self.filter_by_source_type(&filtered_refs);

        if self.resolution_config.read().type_check {
            self.maybe_rebuild_graph(&snapshots)?;
        }

        let sorted_filtered = self.sort_snapshot_refs_by_file_order(&type_filtered);

        let mut resolved = None;

        for snapshot in sorted_filtered {
            if let Some(variable) = snapshot.variables.iter().find(|v| v.key.as_str() == key) {
                resolved = Some(self.resolve_variable(
                    variable,
                    &snapshots,
                    context,
                    0,
                    &mut Vec::new(),
                )?);
            }
        }

        if let Some(ref var) = resolved {
            if var.has_warnings && self.resolution_config.read().type_check {
                return Err(AbundantisError::CircularDependency {
                    chain: format!("Cycle detected resolving '{}'", key),
                });
            }
            self.cache.insert(cache_key, Arc::clone(var));
        }

        Ok(resolved)
    }

    #[cfg_attr(feature = "async", must_be_async)]
    #[cfg_attr(not(feature = "async"), must_be_sync)]
    pub async fn all_variables_with_filter(
        &self,
        context: &super::workspace::WorkspaceContext,
        registry: &super::source::SourceRegistry,
        file_source_filter: Option<&HashSet<super::source::SourceId>>,
    ) -> Result<Vec<Arc<ResolvedVariable>>> {
        let snapshots = registry.load_all().await.map_err(AbundantisError::Source)?;

        let filtered_refs = self.filter_snapshots_ref(&snapshots, file_source_filter);

        if self.resolution_config.read().type_check {
            self.maybe_rebuild_graph(&snapshots)?;
        }

        self.all_variables_inner(context, &snapshots, &filtered_refs)
    }

    fn hash_context(&self, context: &super::workspace::WorkspaceContext) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        context.workspace_root.hash(&mut hasher);
        context.package_root.hash(&mut hasher);
        context.package_name.hash(&mut hasher);
        for env_file in &context.env_files {
            env_file.hash(&mut hasher);
        }
        hasher.finish()
    }

    fn build_dependency_graph(&self, snapshots: &[crate::source::SourceSnapshot]) -> Result<()> {
        let mut graph = self.graph.write();
        graph.clear();

        for snapshot in snapshots {
            for variable in snapshot.variables.iter() {
                let references = self.find_variable_references(&variable.raw_value);
                for ref_key in references {
                    graph.add_edge(variable.key.clone(), ref_key, Some((0, 0)));
                }
            }
        }

        let mut visited = HashMap::new();
        let mut path = Vec::new();
        for snapshot in snapshots {
            for variable in snapshot.variables.iter() {
                let cycle =
                    graph.detect_cycle_with_state(variable.key.as_str(), &mut visited, &mut path);
                if !cycle.is_empty() {
                    let chain = cycle
                        .iter()
                        .map(|k| k.as_str())
                        .collect::<Vec<_>>()
                        .join(" -> ");
                    return Err(AbundantisError::CircularDependency {
                        chain: format!("{} -> {}", chain, variable.key),
                    });
                }
            }
        }

        Ok(())
    }

    fn find_variable_references(&self, value: &str) -> Vec<CompactString> {
        germi::find_variable_references(value)
            .into_iter()
            .map(CompactString::new)
            .collect()
    }

    pub fn cache(&self) -> &Arc<ResolutionCache> {
        &self.cache
    }

    pub fn graph(&self) -> &Arc<parking_lot::RwLock<DependencyGraph>> {
        &self.graph
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_basics() {
        let config = super::super::config::CacheConfig {
            enabled: true,
            hot_cache_size: 100,
            ttl: Duration::from_secs(60),
        };

        let cache = ResolutionCache::new(&config);
        assert!(cache.is_empty());

        let key = CacheKey {
            key: CompactString::new("TEST"),
            context_hash: 123,
        };

        let var = Arc::new(ResolvedVariable {
            key: CompactString::new("TEST"),
            raw_value: CompactString::new("value"),
            resolved_value: CompactString::new("value"),
            source: super::super::source::VariableSource::Memory,
            description: None,
            has_warnings: false,
            interpolation_depth: 0,
        });

        cache.insert(key.clone(), var.clone());
        assert!(!cache.is_empty());
        assert_eq!(cache.len(), 2);

        let retrieved = cache.get(&key).unwrap();
        assert_eq!(retrieved.key.as_str(), "TEST");
    }

    #[test]
    fn test_dependency_cycle_detection() {
        let mut graph = DependencyGraph::new();

        graph.add_edge(CompactString::new("A"), CompactString::new("B"), None);
        graph.add_edge(CompactString::new("B"), CompactString::new("C"), None);
        graph.add_edge(CompactString::new("C"), CompactString::new("A"), None);

        let cycle = graph.detect_cycle("A");
        assert!(!cycle.is_empty());
        assert!(cycle.contains(&CompactString::new("A")));
    }
}
