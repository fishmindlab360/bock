//! Build cache management.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use bock_common::BockResult;
use serde::{Deserialize, Serialize};

const CACHE_METADATA_FILE: &str = "cache-metadata.json";

/// Build cache manager.
pub struct CacheManager {
    /// Cache directory.
    cache_dir: PathBuf,
    /// Cache metadata.
    metadata: CacheMetadata,
}

/// Cache metadata.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct CacheMetadata {
    /// Cache entries.
    entries: HashMap<String, CacheEntry>,
}

/// Cache entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheEntry {
    /// Layer digest.
    digest: String,
    /// Size in bytes.
    size: u64,
    /// Last access time (Unix timestamp).
    last_access: u64,
    /// Creation time (Unix timestamp).
    created: u64,
    /// Build command that created this layer.
    command: Option<String>,
}

impl CacheManager {
    /// Create a new cache manager.
    pub fn new(cache_dir: impl Into<PathBuf>) -> Self {
        let cache_dir = cache_dir.into();

        // Load existing metadata
        let metadata = Self::load_metadata(&cache_dir).unwrap_or_default();

        Self {
            cache_dir,
            metadata,
        }
    }

    /// Get cache directory.
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    /// Check if a layer is cached.
    pub fn has(&self, key: &str) -> bool {
        self.metadata.entries.contains_key(key) && self.cache_dir.join(key).exists()
    }

    /// Get cached layer path.
    pub fn get(&self, key: &str) -> Option<PathBuf> {
        if self.has(key) {
            Some(self.cache_dir.join(key))
        } else {
            None
        }
    }

    /// Get cache entry info.
    pub fn get_entry(&self, key: &str) -> Option<&CacheEntry> {
        self.metadata.entries.get(key)
    }

    /// Store a layer in cache.
    pub fn store(&mut self, key: &str, layer_path: &Path) -> BockResult<()> {
        let dest = self.cache_dir.join(key);

        // Create cache directory
        fs::create_dir_all(&self.cache_dir)?;

        // Copy or move layer to cache
        if layer_path.is_dir() {
            copy_dir_all(layer_path, &dest)?;
        } else if layer_path.is_file() {
            fs::copy(layer_path, &dest)?;
        }

        // Calculate size
        let size = calculate_size(&dest)?;

        // Get current timestamp
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Update metadata
        self.metadata.entries.insert(
            key.to_string(),
            CacheEntry {
                digest: key.to_string(),
                size,
                last_access: now,
                created: now,
                command: None,
            },
        );

        self.save_metadata()?;

        tracing::debug!(key, size, "Layer stored in cache");
        Ok(())
    }

    /// Store with command info.
    pub fn store_with_command(
        &mut self,
        key: &str,
        layer_path: &Path,
        command: &str,
    ) -> BockResult<()> {
        self.store(key, layer_path)?;

        if let Some(entry) = self.metadata.entries.get_mut(key) {
            entry.command = Some(command.to_string());
            self.save_metadata()?;
        }

        Ok(())
    }

    /// Touch entry to update access time.
    pub fn touch(&mut self, key: &str) -> BockResult<()> {
        if let Some(entry) = self.metadata.entries.get_mut(key) {
            entry.last_access = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            self.save_metadata()?;
        }
        Ok(())
    }

    /// Prune old cache entries.
    pub fn prune(&mut self, max_age_days: u64) -> BockResult<u64> {
        let now = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let max_age_secs = max_age_days * 24 * 60 * 60;
        let mut freed = 0u64;
        let mut to_remove = Vec::new();

        for (key, entry) in &self.metadata.entries {
            if now - entry.last_access > max_age_secs {
                to_remove.push(key.clone());
                freed += entry.size;
            }
        }

        for key in to_remove {
            let path = self.cache_dir.join(&key);
            if path.exists() {
                if path.is_dir() {
                    fs::remove_dir_all(&path).ok();
                } else {
                    fs::remove_file(&path).ok();
                }
            }
            self.metadata.entries.remove(&key);
        }

        self.save_metadata()?;

        tracing::info!(max_age_days, freed_bytes = freed, "Cache pruned");
        Ok(freed)
    }

    /// Clear entire cache.
    pub fn clear(&mut self) -> BockResult<u64> {
        let mut freed = 0u64;

        for entry in self.metadata.entries.values() {
            freed += entry.size;
        }

        // Remove all cache files
        if self.cache_dir.exists() {
            for entry in fs::read_dir(&self.cache_dir)? {
                let entry = entry?;
                let path = entry.path();

                // Don't delete metadata file yet
                if path
                    .file_name()
                    .map(|n| n == CACHE_METADATA_FILE)
                    .unwrap_or(false)
                {
                    continue;
                }

                if path.is_dir() {
                    fs::remove_dir_all(&path).ok();
                } else {
                    fs::remove_file(&path).ok();
                }
            }
        }

        self.metadata.entries.clear();
        self.save_metadata()?;

        tracing::info!(freed_bytes = freed, "Cache cleared");
        Ok(freed)
    }

    /// List cache entries.
    pub fn list(&self) -> Vec<CacheInfo> {
        self.metadata
            .entries
            .iter()
            .map(|(key, entry)| CacheInfo {
                key: key.clone(),
                digest: entry.digest.clone(),
                size: entry.size,
                created: entry.created,
                last_access: entry.last_access,
                command: entry.command.clone(),
            })
            .collect()
    }

    /// Get total cache size.
    pub fn total_size(&self) -> u64 {
        self.metadata.entries.values().map(|e| e.size).sum()
    }

    /// Get entry count.
    pub fn entry_count(&self) -> usize {
        self.metadata.entries.len()
    }

    /// Load metadata from disk.
    fn load_metadata(cache_dir: &Path) -> Option<CacheMetadata> {
        let path = cache_dir.join(CACHE_METADATA_FILE);
        let content = fs::read_to_string(&path).ok()?;
        serde_json::from_str(&content).ok()
    }

    /// Save metadata to disk.
    fn save_metadata(&self) -> BockResult<()> {
        fs::create_dir_all(&self.cache_dir)?;

        let path = self.cache_dir.join(CACHE_METADATA_FILE);
        let content = serde_json::to_string_pretty(&self.metadata).map_err(|e| {
            bock_common::BockError::Internal {
                message: format!("Failed to serialize cache metadata: {}", e),
            }
        })?;

        fs::write(&path, content)?;
        Ok(())
    }
}

/// Cache entry information.
#[derive(Debug, Clone)]
pub struct CacheInfo {
    /// Cache key.
    pub key: String,
    /// Layer digest.
    pub digest: String,
    /// Size in bytes.
    pub size: u64,
    /// Creation time (Unix timestamp).
    pub created: u64,
    /// Last access time (Unix timestamp).
    pub last_access: u64,
    /// Build command.
    pub command: Option<String>,
}

impl CacheInfo {
    /// Format size as human-readable string.
    pub fn size_human(&self) -> String {
        format_size(self.size)
    }
}

/// Copy directory recursively.
fn copy_dir_all(src: &Path, dst: &Path) -> BockResult<()> {
    fs::create_dir_all(dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dst_path = dst.join(entry.file_name());

        if ty.is_dir() {
            copy_dir_all(&entry.path(), &dst_path)?;
        } else {
            fs::copy(entry.path(), &dst_path)?;
        }
    }

    Ok(())
}

/// Calculate size of path.
fn calculate_size(path: &Path) -> BockResult<u64> {
    let mut total = 0;

    if path.is_file() {
        return Ok(fs::metadata(path)?.len());
    }

    for entry in walkdir::WalkDir::new(path) {
        if let Ok(e) = entry {
            if e.file_type().is_file() {
                if let Ok(meta) = e.metadata() {
                    total += meta.len();
                }
            }
        }
    }

    Ok(total)
}

/// Format size as human-readable string.
fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_manager() {
        let temp_dir = tempfile::tempdir().unwrap();
        let cache = CacheManager::new(temp_dir.path());

        assert!(!cache.has("test-key"));
        assert_eq!(cache.entry_count(), 0);
    }

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(500), "500 B");
        assert_eq!(format_size(2048), "2.00 KB");
        assert_eq!(format_size(2 * 1024 * 1024), "2.00 MB");
    }
}
