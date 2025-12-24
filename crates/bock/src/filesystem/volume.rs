//! Volume management for containers.
//!
//! This module provides utilities for creating, managing, and mounting
//! named volumes that persist across container lifecycles.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use bock_common::BockResult;

/// Default volume storage directory.
const DEFAULT_VOLUME_DIR: &str = "/var/lib/bock/volumes";

/// Volume manager for container volumes.
pub struct VolumeManager {
    /// Base directory for volume storage.
    base_dir: PathBuf,
    /// Cached volume metadata.
    volumes: HashMap<String, Volume>,
}

/// A named volume.
#[derive(Debug, Clone)]
pub struct Volume {
    /// Volume name.
    pub name: String,
    /// Path to volume data.
    pub path: PathBuf,
    /// Volume driver (local, nfs, etc).
    pub driver: String,
    /// Volume labels.
    pub labels: HashMap<String, String>,
    /// Creation timestamp.
    pub created: chrono::DateTime<chrono::Utc>,
}

/// Volume mount specification.
#[derive(Debug, Clone)]
pub struct VolumeMount {
    /// Volume name or host path.
    pub source: String,
    /// Container mount point.
    pub target: PathBuf,
    /// Read-only mount.
    pub readonly: bool,
}

impl VolumeManager {
    /// Create a new volume manager with default directory.
    pub fn new() -> BockResult<Self> {
        Self::with_base_dir(PathBuf::from(DEFAULT_VOLUME_DIR))
    }

    /// Create a volume manager with custom base directory.
    pub fn with_base_dir(base_dir: PathBuf) -> BockResult<Self> {
        // Ensure base directory exists
        if !base_dir.exists() {
            fs::create_dir_all(&base_dir)?;
        }

        let mut manager = Self {
            base_dir,
            volumes: HashMap::new(),
        };

        // Load existing volumes
        manager.load_volumes()?;

        Ok(manager)
    }

    /// Create a new named volume.
    pub fn create(&mut self, name: &str, labels: HashMap<String, String>) -> BockResult<Volume> {
        if self.volumes.contains_key(name) {
            return Err(bock_common::BockError::Config {
                message: format!("Volume '{}' already exists", name),
            });
        }

        let volume_path = self.base_dir.join(name);
        fs::create_dir_all(&volume_path)?;

        let volume = Volume {
            name: name.to_string(),
            path: volume_path.clone(),
            driver: "local".to_string(),
            labels,
            created: chrono::Utc::now(),
        };

        // Save metadata
        let metadata_path = volume_path.join("_metadata.json");
        let metadata =
            serde_json::to_string_pretty(&VolumeMetadata::from(&volume)).map_err(|e| {
                bock_common::BockError::Internal {
                    message: format!("Failed to serialize volume metadata: {}", e),
                }
            })?;
        fs::write(&metadata_path, metadata)?;

        self.volumes.insert(name.to_string(), volume.clone());

        tracing::info!(name, path = %volume_path.display(), "Volume created");
        Ok(volume)
    }

    /// Get an existing volume by name.
    pub fn get(&self, name: &str) -> Option<&Volume> {
        self.volumes.get(name)
    }

    /// Get or create a volume.
    pub fn get_or_create(&mut self, name: &str) -> BockResult<Volume> {
        if let Some(vol) = self.volumes.get(name) {
            return Ok(vol.clone());
        }
        self.create(name, HashMap::new())
    }

    /// List all volumes.
    pub fn list(&self) -> Vec<&Volume> {
        self.volumes.values().collect()
    }

    /// Remove a volume.
    pub fn remove(&mut self, name: &str, force: bool) -> BockResult<()> {
        let volume = self
            .volumes
            .remove(name)
            .ok_or_else(|| bock_common::BockError::Config {
                message: format!("Volume '{}' not found", name),
            })?;

        // Check if volume is in use (would need container tracking)
        if !force {
            // TODO: Check if volume is mounted by any container
        }

        // Remove volume directory
        if volume.path.exists() {
            fs::remove_dir_all(&volume.path)?;
        }

        tracing::info!(name, "Volume removed");
        Ok(())
    }

    /// Prune unused volumes.
    pub fn prune(&mut self) -> BockResult<Vec<String>> {
        // TODO: Track volume usage and remove unused ones
        // For now, return empty list
        Ok(Vec::new())
    }

    /// Mount a volume into a container rootfs.
    pub fn mount_volume(&self, volume_mount: &VolumeMount, rootfs: &Path) -> BockResult<()> {
        let volume =
            self.get(&volume_mount.source)
                .ok_or_else(|| bock_common::BockError::Config {
                    message: format!("Volume '{}' not found", volume_mount.source),
                })?;

        let target = rootfs.join(
            volume_mount
                .target
                .strip_prefix("/")
                .unwrap_or(&volume_mount.target),
        );

        // Ensure target directory exists
        if !target.exists() {
            fs::create_dir_all(&target)?;
        }

        // Create bind mount
        crate::filesystem::bind_mount(&volume.path, &target, volume_mount.readonly)?;

        tracing::debug!(
            volume = %volume.name,
            target = %target.display(),
            readonly = volume_mount.readonly,
            "Volume mounted"
        );

        Ok(())
    }

    /// Load existing volumes from disk.
    fn load_volumes(&mut self) -> BockResult<()> {
        if !self.base_dir.exists() {
            return Ok(());
        }

        for entry in fs::read_dir(&self.base_dir)? {
            let entry = entry?;
            let path = entry.path();

            if !path.is_dir() {
                continue;
            }

            let metadata_path = path.join("_metadata.json");
            if !metadata_path.exists() {
                continue;
            }

            let content = fs::read_to_string(&metadata_path)?;
            let metadata: VolumeMetadata =
                serde_json::from_str(&content).map_err(|e| bock_common::BockError::Internal {
                    message: format!("Failed to parse volume metadata: {}", e),
                })?;

            let volume = Volume {
                name: metadata.name.clone(),
                path: path.clone(),
                driver: metadata.driver,
                labels: metadata.labels,
                created: metadata.created,
            };

            self.volumes.insert(metadata.name, volume);
        }

        tracing::debug!(count = self.volumes.len(), "Loaded existing volumes");
        Ok(())
    }
}

impl Default for VolumeManager {
    fn default() -> Self {
        Self::new().unwrap_or_else(|_| Self {
            base_dir: PathBuf::from(DEFAULT_VOLUME_DIR),
            volumes: HashMap::new(),
        })
    }
}

/// Volume metadata for persistence.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct VolumeMetadata {
    name: String,
    driver: String,
    labels: HashMap<String, String>,
    created: chrono::DateTime<chrono::Utc>,
}

impl From<&Volume> for VolumeMetadata {
    fn from(vol: &Volume) -> Self {
        Self {
            name: vol.name.clone(),
            driver: vol.driver.clone(),
            labels: vol.labels.clone(),
            created: vol.created,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_volume_mount_creation() {
        let mount = VolumeMount {
            source: "mydata".to_string(),
            target: PathBuf::from("/app/data"),
            readonly: false,
        };
        assert_eq!(mount.source, "mydata");
        assert_eq!(mount.target, PathBuf::from("/app/data"));
        assert!(!mount.readonly);
    }
}
