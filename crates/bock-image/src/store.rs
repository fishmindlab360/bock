//! Image store.
//!
//! This module provides local storage for container images in OCI format.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use bock_common::BockResult;
use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const IMAGES_DIR: &str = "images";
const BLOBS_DIR: &str = "blobs/sha256";
const REPOSITORIES_FILE: &str = "repositories.json";

/// Local image store.
pub struct ImageStore {
    /// Storage root directory.
    root: PathBuf,
    /// Repository index.
    repositories: HashMap<String, ImageIndex>,
}

/// Image index for a repository.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct ImageIndex {
    /// Tag to digest mapping.
    tags: HashMap<String, String>,
}

/// Stored image metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredImage {
    /// Image reference (name:tag).
    pub reference: String,
    /// Manifest digest.
    pub digest: String,
    /// Config digest.
    pub config_digest: String,
    /// Layer digests.
    pub layers: Vec<String>,
    /// Total size in bytes.
    pub size: u64,
    /// Created timestamp.
    pub created: Option<String>,
    /// Architecture.
    pub architecture: String,
    /// OS.
    pub os: String,
}

/// OCI Image Manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageManifest {
    /// Schema version.
    #[serde(rename = "schemaVersion")]
    pub schema_version: u32,
    /// Media type.
    #[serde(rename = "mediaType", default)]
    pub media_type: Option<String>,
    /// Config descriptor.
    pub config: Descriptor,
    /// Layer descriptors.
    pub layers: Vec<Descriptor>,
}

/// OCI Image Config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageConfig {
    /// Architecture.
    #[serde(default)]
    pub architecture: String,
    /// OS.
    #[serde(default)]
    pub os: String,
    /// Created timestamp.
    #[serde(default)]
    pub created: Option<String>,
    /// Config.
    #[serde(default)]
    pub config: RuntimeConfig,
    /// Rootfs.
    #[serde(default)]
    pub rootfs: Rootfs,
}

/// Runtime configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RuntimeConfig {
    /// Entrypoint.
    #[serde(rename = "Entrypoint", default)]
    pub entrypoint: Option<Vec<String>>,
    /// Cmd.
    #[serde(rename = "Cmd", default)]
    pub cmd: Option<Vec<String>>,
    /// Working directory.
    #[serde(rename = "WorkingDir", default)]
    pub working_dir: Option<String>,
    /// Environment variables.
    #[serde(rename = "Env", default)]
    pub env: Option<Vec<String>>,
    /// User.
    #[serde(rename = "User", default)]
    pub user: Option<String>,
}

/// Rootfs configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Rootfs {
    /// Type.
    #[serde(rename = "type", default)]
    pub fs_type: String,
    /// Layer diff IDs.
    #[serde(default)]
    pub diff_ids: Vec<String>,
}

/// Content descriptor.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Descriptor {
    /// Media type.
    #[serde(rename = "mediaType")]
    pub media_type: String,
    /// Content digest.
    pub digest: String,
    /// Content size.
    pub size: u64,
}

impl ImageStore {
    /// Create a new image store.
    pub fn new(root: impl Into<PathBuf>) -> BockResult<Self> {
        let root = root.into();

        // Create directories
        fs::create_dir_all(root.join(IMAGES_DIR))?;
        fs::create_dir_all(root.join(BLOBS_DIR))?;

        // Load existing repositories
        let repositories = Self::load_repositories(&root)?;

        Ok(Self { root, repositories })
    }

    /// Get the root directory.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Get the blobs directory.
    pub fn blobs_dir(&self) -> PathBuf {
        self.root.join(BLOBS_DIR)
    }

    /// Load repositories index from disk.
    fn load_repositories(root: &Path) -> BockResult<HashMap<String, ImageIndex>> {
        let path = root.join(REPOSITORIES_FILE);

        if path.exists() {
            let content = fs::read_to_string(&path)?;
            serde_json::from_str(&content).map_err(|e| bock_common::BockError::Internal {
                message: format!("Failed to parse repositories: {}", e),
            })
        } else {
            Ok(HashMap::new())
        }
    }

    /// Save repositories index to disk.
    fn save_repositories(&self) -> BockResult<()> {
        let path = self.root.join(REPOSITORIES_FILE);
        let content = serde_json::to_string_pretty(&self.repositories).map_err(|e| {
            bock_common::BockError::Internal {
                message: format!("Failed to serialize repositories: {}", e),
            }
        })?;
        fs::write(&path, content)?;
        Ok(())
    }

    /// Save an image to the store.
    pub fn save(
        &mut self,
        reference: &str,
        manifest_bytes: &[u8],
        config_bytes: &[u8],
        layers: &[(String, Vec<u8>)],
    ) -> BockResult<StoredImage> {
        tracing::info!(reference, "Saving image to store");

        // Parse reference
        let (name, tag) = Self::parse_reference(reference)?;

        // Store manifest
        let manifest_digest = self.store_blob(manifest_bytes)?;

        // Store config
        let config_digest = self.store_blob(config_bytes)?;

        // Parse manifest
        let _manifest: ImageManifest = serde_json::from_slice(manifest_bytes).map_err(|e| {
            bock_common::BockError::Internal {
                message: format!("Failed to parse manifest: {}", e),
            }
        })?;

        // Parse config
        let config: ImageConfig =
            serde_json::from_slice(config_bytes).map_err(|e| bock_common::BockError::Internal {
                message: format!("Failed to parse config: {}", e),
            })?;

        // Store layers
        let mut layer_digests = Vec::new();
        let mut total_size = manifest_bytes.len() as u64 + config_bytes.len() as u64;

        for (expected_digest, layer_data) in layers {
            let digest = self.store_blob(layer_data)?;

            // Verify digest
            if !expected_digest.is_empty() && &digest != expected_digest {
                tracing::warn!(
                    expected = %expected_digest,
                    actual = %digest,
                    "Layer digest mismatch (content may differ)"
                );
            }

            layer_digests.push(digest);
            total_size += layer_data.len() as u64;
        }

        // Update repository index
        let index = self.repositories.entry(name.clone()).or_default();
        index.tags.insert(tag.clone(), manifest_digest.clone());
        self.save_repositories()?;

        let stored = StoredImage {
            reference: reference.to_string(),
            digest: manifest_digest,
            config_digest,
            layers: layer_digests,
            size: total_size,
            created: config.created.clone(),
            architecture: config.architecture.clone(),
            os: config.os.clone(),
        };

        tracing::info!(
            reference,
            digest = %stored.digest,
            layers = stored.layers.len(),
            size = stored.size,
            "Image saved"
        );

        Ok(stored)
    }

    /// Load an image from the store.
    pub fn load(&self, reference: &str) -> BockResult<Option<StoredImage>> {
        tracing::debug!(reference, "Loading image from store");

        let (name, tag) = Self::parse_reference(reference)?;

        // Get digest from repository index
        let digest = match self.repositories.get(&name) {
            Some(index) => match index.tags.get(&tag) {
                Some(d) => d.clone(),
                None => return Ok(None),
            },
            None => return Ok(None),
        };

        // Load manifest
        let manifest_bytes = self.get_blob(&digest)?;
        let manifest_bytes = match manifest_bytes {
            Some(b) => b,
            None => return Ok(None),
        };

        let manifest: ImageManifest = serde_json::from_slice(&manifest_bytes).map_err(|e| {
            bock_common::BockError::Internal {
                message: format!("Failed to parse manifest: {}", e),
            }
        })?;

        // Load config
        let config_bytes = self.get_blob(&manifest.config.digest)?;
        let config_bytes = match config_bytes {
            Some(b) => b,
            None => return Ok(None),
        };

        let config: ImageConfig = serde_json::from_slice(&config_bytes).map_err(|e| {
            bock_common::BockError::Internal {
                message: format!("Failed to parse config: {}", e),
            }
        })?;

        // Calculate total size
        let mut total_size = manifest_bytes.len() as u64 + config_bytes.len() as u64;
        let mut layer_digests = Vec::new();

        for layer in &manifest.layers {
            layer_digests.push(layer.digest.clone());
            total_size += layer.size;
        }

        Ok(Some(StoredImage {
            reference: reference.to_string(),
            digest,
            config_digest: manifest.config.digest.clone(),
            layers: layer_digests,
            size: total_size,
            created: config.created.clone(),
            architecture: config.architecture.clone(),
            os: config.os.clone(),
        }))
    }

    /// List all stored images.
    pub fn list(&self) -> BockResult<Vec<StoredImage>> {
        let mut images = Vec::new();

        for (name, index) in &self.repositories {
            for (tag, _) in &index.tags {
                let reference = format!("{}:{}", name, tag);
                if let Some(image) = self.load(&reference)? {
                    images.push(image);
                }
            }
        }

        Ok(images)
    }

    /// Get an image by reference.
    pub fn get(&self, reference: &str) -> BockResult<Option<StoredImage>> {
        self.load(reference)
    }

    /// Delete an image.
    pub fn delete(&mut self, reference: &str) -> BockResult<bool> {
        tracing::info!(reference, "Deleting image from store");

        let (name, tag) = Self::parse_reference(reference)?;

        // Remove from repository index
        if let Some(index) = self.repositories.get_mut(&name) {
            if index.tags.remove(&tag).is_some() {
                // If no more tags, remove the repository
                if index.tags.is_empty() {
                    self.repositories.remove(&name);
                }
                self.save_repositories()?;
                tracing::info!(reference, "Image deleted");
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Store a blob and return its digest.
    pub fn store_blob(&self, data: &[u8]) -> BockResult<String> {
        let hash = Sha256::digest(data);
        let digest = format!("sha256:{:x}", hash);

        let blob_path = self.blob_path(&digest);

        if !blob_path.exists() {
            if let Some(parent) = blob_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::write(&blob_path, data)?;
            tracing::debug!(digest = %digest, size = data.len(), "Blob stored");
        }

        Ok(digest)
    }

    /// Get a blob by digest.
    pub fn get_blob(&self, digest: &str) -> BockResult<Option<Vec<u8>>> {
        let blob_path = self.blob_path(digest);

        if blob_path.exists() {
            let data = fs::read(&blob_path)?;
            Ok(Some(data))
        } else {
            Ok(None)
        }
    }

    /// Check if a blob exists.
    pub fn has_blob(&self, digest: &str) -> bool {
        self.blob_path(digest).exists()
    }

    /// Get the path for a blob.
    fn blob_path(&self, digest: &str) -> PathBuf {
        let hash = digest.strip_prefix("sha256:").unwrap_or(digest);
        self.root.join(BLOBS_DIR).join(hash)
    }

    /// Parse a reference into (name, tag).
    fn parse_reference(reference: &str) -> BockResult<(String, String)> {
        if let Some(idx) = reference.rfind(':') {
            let tag = &reference[idx + 1..];
            // Make sure it's not a port number
            if !tag.contains('/') && tag.parse::<u16>().is_err() {
                return Ok((reference[..idx].to_string(), tag.to_string()));
            }
        }

        Ok((reference.to_string(), "latest".to_string()))
    }

    /// Extract layers to a directory.
    pub fn extract_layers(&self, image: &StoredImage, dest: &Path) -> BockResult<()> {
        tracing::info!(
            reference = %image.reference,
            dest = %dest.display(),
            layers = image.layers.len(),
            "Extracting image layers"
        );

        fs::create_dir_all(dest)?;

        for (i, digest) in image.layers.iter().enumerate() {
            let layer_data =
                self.get_blob(digest)?
                    .ok_or_else(|| bock_common::BockError::Internal {
                        message: format!("Layer not found: {}", digest),
                    })?;

            // Decompress and extract tar
            self.extract_layer(&layer_data, dest)?;

            tracing::debug!(
                layer = i + 1,
                total = image.layers.len(),
                digest = %digest,
                "Layer extracted"
            );
        }

        tracing::info!(dest = %dest.display(), "Layers extracted");
        Ok(())
    }

    /// Extract a single layer (gzipped tar).
    fn extract_layer(&self, layer_data: &[u8], dest: &Path) -> BockResult<()> {
        // Try gzip decompression
        let decoder = GzDecoder::new(layer_data);
        let mut archive = tar::Archive::new(decoder);

        archive
            .unpack(dest)
            .map_err(|e| bock_common::BockError::Internal {
                message: format!("Failed to extract layer: {}", e),
            })?;

        Ok(())
    }

    /// Calculate total store size.
    pub fn total_size(&self) -> BockResult<u64> {
        let mut total = 0;

        for entry in walkdir::WalkDir::new(&self.root) {
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

    /// Garbage collect unused blobs.
    pub fn gc(&mut self) -> BockResult<u64> {
        tracing::info!("Running garbage collection");

        // Collect all referenced digests
        let mut referenced = std::collections::HashSet::new();

        for (_name, index) in &self.repositories {
            for (_tag, digest) in &index.tags {
                referenced.insert(digest.clone());

                // Load manifest to get layer digests
                if let Some(manifest_bytes) = self.get_blob(digest)? {
                    if let Ok(manifest) = serde_json::from_slice::<ImageManifest>(&manifest_bytes) {
                        referenced.insert(manifest.config.digest.clone());
                        for layer in &manifest.layers {
                            referenced.insert(layer.digest.clone());
                        }
                    }
                }
            }
        }

        // Remove unreferenced blobs
        let mut freed = 0u64;
        let blobs_dir = self.blobs_dir();

        if blobs_dir.exists() {
            for entry in fs::read_dir(&blobs_dir)? {
                let entry = entry?;
                let path = entry.path();

                if path.is_file() {
                    if let Some(name) = path.file_name() {
                        let digest = format!("sha256:{}", name.to_string_lossy());

                        if !referenced.contains(&digest) {
                            if let Ok(meta) = path.metadata() {
                                freed += meta.len();
                            }
                            fs::remove_file(&path)?;
                            tracing::debug!(digest = %digest, "Removed unreferenced blob");
                        }
                    }
                }
            }
        }

        tracing::info!(freed_bytes = freed, "Garbage collection complete");
        Ok(freed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_reference() {
        let (name, tag) = ImageStore::parse_reference("nginx:1.21").unwrap();
        assert_eq!(name, "nginx");
        assert_eq!(tag, "1.21");

        let (name, tag) = ImageStore::parse_reference("alpine").unwrap();
        assert_eq!(name, "alpine");
        assert_eq!(tag, "latest");

        let (name, tag) = ImageStore::parse_reference("registry.io/user/image:v1").unwrap();
        assert_eq!(name, "registry.io/user/image");
        assert_eq!(tag, "v1");
    }

    #[test]
    fn test_store_blob() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = ImageStore::new(temp_dir.path()).unwrap();

        let data = b"hello world";
        let digest = store.store_blob(data).unwrap();

        assert!(digest.starts_with("sha256:"));
        assert!(store.has_blob(&digest));

        let retrieved = store.get_blob(&digest).unwrap().unwrap();
        assert_eq!(retrieved, data);
    }
}
