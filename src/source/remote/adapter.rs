//! Adapter that wraps RemoteSource to implement AsyncEnvSource.

use super::traits::{AuthConfig, RemoteSource, ScopeSelection};
use crate::error::SourceError;
use crate::source::traits::{
    AsyncEnvSource, Priority, SourceCapabilities, SourceId, SourceMetadata, SourceSnapshot,
    SourceType,
};
use async_trait::async_trait;
use compact_str::CompactString;
use parking_lot::RwLock;
use std::sync::Arc;
use std::time::Instant;

/// Adapter that wraps a RemoteSource to implement AsyncEnvSource.
///
/// This allows remote sources to be registered in the SourceRegistry
/// alongside other async sources.
pub struct RemoteSourceAdapter {
    inner: Arc<dyn RemoteSource>,
    scope: RwLock<ScopeSelection>,
    source_id: SourceId,
    cached_snapshot: RwLock<Option<SourceSnapshot>>,
    last_refreshed: RwLock<Option<Instant>>,
    error_count: RwLock<u32>,
    last_error: RwLock<Option<CompactString>>,
}

impl RemoteSourceAdapter {
    /// Creates a new adapter wrapping the given remote source.
    pub fn new(source: Arc<dyn RemoteSource>) -> Self {
        let source_id = SourceId::new(format!("remote:{}", source.id().as_str()));
        Self {
            inner: source,
            scope: RwLock::new(ScopeSelection::new()),
            source_id,
            cached_snapshot: RwLock::new(None),
            last_refreshed: RwLock::new(None),
            error_count: RwLock::new(0),
            last_error: RwLock::new(None),
        }
    }

    /// Returns a reference to the inner RemoteSource.
    pub fn inner(&self) -> &Arc<dyn RemoteSource> {
        &self.inner
    }

    /// Returns the current scope selection.
    pub fn scope(&self) -> ScopeSelection {
        self.scope.read().clone()
    }

    /// Sets the scope selection for fetching secrets.
    pub fn set_scope(&self, scope: ScopeSelection) {
        *self.scope.write() = scope;
        // Clear cache when scope changes
        *self.cached_snapshot.write() = None;
    }

    /// Authenticates with the remote provider.
    pub async fn authenticate(&self, config: &AuthConfig) -> Result<(), SourceError> {
        self.inner.authenticate(config).await
    }

    /// Returns the last error, if any.
    pub fn last_error(&self) -> Option<CompactString> {
        self.last_error.read().clone()
    }

    /// Clears the cached snapshot, forcing a refresh on next load.
    pub fn invalidate_cache(&self) {
        *self.cached_snapshot.write() = None;
    }
}

#[async_trait]
impl AsyncEnvSource for RemoteSourceAdapter {
    fn id(&self) -> &SourceId {
        &self.source_id
    }

    fn source_type(&self) -> SourceType {
        SourceType::Remote
    }

    fn priority(&self) -> Priority {
        Priority::REMOTE
    }

    fn capabilities(&self) -> SourceCapabilities {
        self.inner.capabilities()
    }

    async fn load(&self) -> Result<SourceSnapshot, SourceError> {
        // Check auth status first
        let auth_status = self.inner.auth_status().await;
        if !auth_status.is_authenticated() {
            return Err(SourceError::Authentication {
                source_name: self.source_id.as_str().to_string(),
            });
        }

        // Use cached snapshot if available and not changed
        // Note: We must drop the guard before the await to avoid Send issues
        let cached = self.cached_snapshot.read().clone();
        if let Some(cached) = cached {
            if !self.inner.has_changed().await {
                return Ok(cached);
            }
        }

        // Fetch secrets for current scope
        let scope = self.scope.read().clone();
        match self.inner.fetch_secrets(&scope).await {
            Ok(snapshot) => {
                // Rewrite the snapshot's source_id to use the adapter's prefixed ID
                // This ensures filtering by source type works correctly (e.g., "remote:doppler")
                let snapshot = SourceSnapshot {
                    source_id: self.source_id.clone(),
                    ..snapshot
                };
                *self.cached_snapshot.write() = Some(snapshot.clone());
                *self.last_refreshed.write() = Some(Instant::now());
                *self.error_count.write() = 0;
                *self.last_error.write() = None;
                Ok(snapshot)
            }
            Err(e) => {
                *self.error_count.write() += 1;
                *self.last_error.write() = Some(e.to_string().into());
                Err(e)
            }
        }
    }

    async fn refresh(&self) -> Result<bool, SourceError> {
        // Clear cache to force refresh
        self.invalidate_cache();

        // Try to load new data
        match self.load().await {
            Ok(_) => Ok(true),
            Err(e) => Err(e),
        }
    }

    fn metadata(&self) -> SourceMetadata {
        let info = self.inner.provider_info();
        SourceMetadata {
            display_name: Some(info.display_name),
            description: info.description,
            last_refreshed: *self.last_refreshed.read(),
            error_count: *self.error_count.read(),
        }
    }
}

impl std::fmt::Debug for RemoteSourceAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteSourceAdapter")
            .field("source_id", &self.source_id)
            .field("scope", &*self.scope.read())
            .field("has_cached", &self.cached_snapshot.read().is_some())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    // Tests would go here with mock implementations
}
