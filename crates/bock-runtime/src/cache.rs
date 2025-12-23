//! Build cache management.

use std::path::PathBuf;

use bock_common::BockResult;

/// Build cache manager.
pub struct CacheManager {
    /// Cache directory.
    cache_dir: PathBuf,
}

impl CacheManager {
    /// Create a new cache manager.
    pub fn new(cache_dir: impl Into<PathBuf>) -> Self {
        Self {
            cache_dir: cache_dir.into(),
        }
    }

    /// Check if a layer is cached.
    pub fn has(&self, key: &str) -> bool {
        self.cache_dir.join(key).exists()
    }

    /// Get cached layer path.
    pub fn get(&self, key: &str) -> Option<PathBuf> {
        let path = self.cache_dir.join(key);
        if path.exists() { Some(path) } else { None }
    }

    /// Store a layer in cache.
    pub fn store(&self, key: &str, layer_path: &PathBuf) -> BockResult<()> {
        tracing::debug!(key, "Storing layer in cache");
        // TODO: Implement
        Ok(())
    }

    /// Prune old cache entries.
    pub fn prune(&self, max_age_days: u64) -> BockResult<u64> {
        tracing::info!(max_age_days, "Pruning cache");
        // TODO: Implement
        Ok(0)
    }
}
