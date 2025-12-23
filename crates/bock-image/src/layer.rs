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
        let dest = self.layer_path(digest);
        if dest.exists() {
            return Ok(dest);
        }

        tracing::debug!(digest, "Extracting layer");

        std::fs::create_dir_all(&dest).map_err(|e| bock_common::BockError::Io(e))?;

        // Detect compression
        let reader: Box<dyn std::io::Read> = if data.starts_with(&[0x1f, 0x8b]) {
            Box::new(flate2::read::GzDecoder::new(data))
        } else if data.starts_with(&[0x28, 0xb5, 0x2f, 0xfd]) {
            Box::new(zstd::stream::read::Decoder::new(data).map_err(|e| bock_common::BockError::Io(e))?)
        } else {
            Box::new(data)
        };

        let mut archive = tar::Archive::new(reader);
        archive.set_preserve_permissions(true);
        archive.set_unpack_xattrs(true);

        archive.unpack(&dest).map_err(|e| bock_common::BockError::Io(e))?;

        Ok(dest)
    }
}
