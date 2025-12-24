//! Copy-on-write layer management.
//!
//! This module provides utilities for managing container image layers
//! with copy-on-write semantics using OverlayFS.

use std::fs;
use std::path::{Path, PathBuf};

use bock_common::BockResult;

/// Layer store for managing container image layers.
pub struct LayerStore {
    /// Base directory for layers.
    base_dir: PathBuf,
}

impl LayerStore {
    /// Create a new layer store.
    pub fn new(base_dir: &Path) -> BockResult<Self> {
        fs::create_dir_all(base_dir)?;

        Ok(Self {
            base_dir: base_dir.to_path_buf(),
        })
    }

    /// Get the path for a layer.
    pub fn layer_path(&self, layer_id: &str) -> PathBuf {
        self.base_dir.join(layer_id)
    }

    /// Create a new layer.
    pub fn create_layer(&self, layer_id: &str) -> BockResult<Layer> {
        let layer_path = self.layer_path(layer_id);
        let diff_path = layer_path.join("diff");
        let work_path = layer_path.join("work");

        fs::create_dir_all(&diff_path)?;
        fs::create_dir_all(&work_path)?;

        tracing::debug!(layer_id, path = %layer_path.display(), "Layer created");

        Ok(Layer {
            id: layer_id.to_string(),
            path: layer_path,
            parent: None,
        })
    }

    /// Create a child layer with a parent.
    pub fn create_child_layer(&self, layer_id: &str, parent_id: &str) -> BockResult<Layer> {
        let layer_path = self.layer_path(layer_id);
        let diff_path = layer_path.join("diff");
        let work_path = layer_path.join("work");

        fs::create_dir_all(&diff_path)?;
        fs::create_dir_all(&work_path)?;

        tracing::debug!(
            layer_id,
            parent_id,
            path = %layer_path.display(),
            "Child layer created"
        );

        Ok(Layer {
            id: layer_id.to_string(),
            path: layer_path,
            parent: Some(parent_id.to_string()),
        })
    }

    /// Get an existing layer.
    pub fn get_layer(&self, layer_id: &str) -> BockResult<Layer> {
        let layer_path = self.layer_path(layer_id);

        if !layer_path.exists() {
            return Err(bock_common::BockError::Internal {
                message: format!("Layer not found: {}", layer_id),
            });
        }

        // Try to read parent from metadata
        let parent = self.read_layer_parent(layer_id)?;

        Ok(Layer {
            id: layer_id.to_string(),
            path: layer_path,
            parent,
        })
    }

    /// Delete a layer.
    pub fn delete_layer(&self, layer_id: &str) -> BockResult<()> {
        let layer_path = self.layer_path(layer_id);

        if layer_path.exists() {
            fs::remove_dir_all(&layer_path)?;
        }

        tracing::debug!(layer_id, "Layer deleted");
        Ok(())
    }

    /// List all layers.
    pub fn list_layers(&self) -> BockResult<Vec<String>> {
        let mut layers = Vec::new();

        if !self.base_dir.exists() {
            return Ok(layers);
        }

        for entry in fs::read_dir(&self.base_dir)? {
            let entry = entry?;
            if entry.path().is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    layers.push(name.to_string());
                }
            }
        }

        Ok(layers)
    }

    /// Read parent from layer metadata.
    fn read_layer_parent(&self, layer_id: &str) -> BockResult<Option<String>> {
        let metadata_path = self.layer_path(layer_id).join("parent");

        if metadata_path.exists() {
            let content = fs::read_to_string(&metadata_path)?;
            Ok(Some(content.trim().to_string()))
        } else {
            Ok(None)
        }
    }

    /// Write parent to layer metadata.
    pub fn set_layer_parent(&self, layer_id: &str, parent_id: &str) -> BockResult<()> {
        let metadata_path = self.layer_path(layer_id).join("parent");
        fs::write(&metadata_path, parent_id)?;
        Ok(())
    }

    /// Get the diff directory for a layer.
    pub fn diff_path(&self, layer_id: &str) -> PathBuf {
        self.layer_path(layer_id).join("diff")
    }

    /// Get the work directory for a layer.
    pub fn work_path(&self, layer_id: &str) -> PathBuf {
        self.layer_path(layer_id).join("work")
    }

    /// Build the lower directories string for OverlayFS.
    pub fn build_lower_dirs(&self, layer_ids: &[String]) -> String {
        layer_ids
            .iter()
            .map(|id| self.diff_path(id).to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join(":")
    }
}

/// A container image layer.
#[derive(Debug, Clone)]
pub struct Layer {
    /// Layer ID.
    pub id: String,
    /// Layer path on disk.
    pub path: PathBuf,
    /// Parent layer ID (if any).
    pub parent: Option<String>,
}

impl Layer {
    /// Get the diff directory.
    pub fn diff(&self) -> PathBuf {
        self.path.join("diff")
    }

    /// Get the work directory.
    pub fn work(&self) -> PathBuf {
        self.path.join("work")
    }

    /// Check if this layer has a parent.
    pub fn has_parent(&self) -> bool {
        self.parent.is_some()
    }
}

/// Calculate the size of a layer.
pub fn layer_size(path: &Path) -> BockResult<u64> {
    let mut total = 0;

    if !path.exists() {
        return Ok(0);
    }

    for entry in walkdir::WalkDir::new(path) {
        let entry = entry.map_err(|e| bock_common::BockError::Internal {
            message: format!("Failed to walk directory: {}", e),
        })?;

        if entry.file_type().is_file() {
            if let Ok(metadata) = entry.metadata() {
                total += metadata.len();
            }
        }
    }

    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_layer_creation() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = LayerStore::new(temp_dir.path()).unwrap();

        let layer = store.create_layer("test-layer").unwrap();
        assert_eq!(layer.id, "test-layer");
        assert!(layer.diff().exists());
        assert!(layer.work().exists());
    }

    #[test]
    fn test_layer_listing() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = LayerStore::new(temp_dir.path()).unwrap();

        store.create_layer("layer-1").unwrap();
        store.create_layer("layer-2").unwrap();

        let layers = store.list_layers().unwrap();
        assert_eq!(layers.len(), 2);
    }
}
