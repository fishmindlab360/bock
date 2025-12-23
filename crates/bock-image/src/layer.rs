//! Image layer management.

use std::path::PathBuf;

use bock_common::BockResult;

/// Layer manager for extracting and caching layers.
pub struct LayerManager {
    /// Cache directory.
    cache_dir: PathBuf,
}

impl LayerManager {
    /// Create a new layer manager.
    pub fn new(cache_dir: impl Into<PathBuf>) -> Self {
        Self {
            cache_dir: cache_dir.into(),
        }
    }

    /// Get layer path by digest.
    pub fn layer_path(&self, digest: &str) -> PathBuf {
        let hash = digest.replace(':', "/");
        self.cache_dir.join(hash)
    }

    /// Check if layer exists in cache.
    pub fn has_layer(&self, digest: &str) -> bool {
        self.layer_path(digest).exists()
    }

    /// Extract a layer from a tarball.
    pub fn extract_layer(&self, digest: &str, data: &[u8]) -> BockResult<PathBuf> {
        tracing::debug!(digest, "Extracting layer");
        // TODO: Implement
        Ok(self.layer_path(digest))
    }
}
