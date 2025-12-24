//! Registry operations for image push/pull/inspect.

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use bock_common::BockResult;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Registry client for image operations.
pub struct Registry {
    /// Registry URL.
    url: String,
    /// Authentication credentials.
    auth: Option<RegistryAuth>,
}

/// Registry authentication.
#[derive(Debug, Clone)]
pub struct RegistryAuth {
    /// Username.
    pub username: String,
    /// Password or token.
    pub password: String,
}

/// Image manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageManifest {
    /// Schema version.
    #[serde(rename = "schemaVersion")]
    pub schema_version: u32,
    /// Media type.
    #[serde(rename = "mediaType")]
    pub media_type: String,
    /// Config descriptor.
    pub config: Descriptor,
    /// Layer descriptors.
    pub layers: Vec<Descriptor>,
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

/// Image information.
#[derive(Debug, Clone)]
pub struct ImageInfo {
    /// Image digest.
    pub digest: String,
    /// Image tag.
    pub tag: Option<String>,
    /// Architecture.
    pub architecture: String,
    /// OS.
    pub os: String,
    /// Created timestamp.
    pub created: Option<String>,
    /// Author.
    pub author: Option<String>,
    /// Number of layers.
    pub layer_count: usize,
    /// Total size in bytes.
    pub size: u64,
    /// Entrypoint.
    pub entrypoint: Vec<String>,
    /// Cmd.
    pub cmd: Vec<String>,
    /// Working directory.
    pub workdir: Option<String>,
    /// Environment variables.
    pub env: Vec<String>,
    /// Exposed ports.
    pub exposed_ports: Vec<String>,
    /// Labels.
    pub labels: HashMap<String, String>,
}

impl Registry {
    /// Create a new registry client.
    pub fn new(url: &str) -> Self {
        Self {
            url: url.to_string(),
            auth: None,
        }
    }

    /// DockerHub registry.
    pub fn dockerhub() -> Self {
        Self::new("https://registry-1.docker.io")
    }

    /// GitHub Container Registry.
    pub fn ghcr() -> Self {
        Self::new("https://ghcr.io")
    }

    /// Set authentication.
    pub fn with_auth(mut self, auth: RegistryAuth) -> Self {
        self.auth = Some(auth);
        self
    }

    /// Push an image to the registry.
    pub async fn push(&self, image_path: &Path, repository: &str, tag: &str) -> BockResult<String> {
        tracing::info!(repository, tag, "Pushing image to registry");

        // Read OCI layout
        let oci_layout_path = image_path.join("oci-layout");
        if !oci_layout_path.exists() {
            return Err(bock_common::BockError::Config {
                message: "Not a valid OCI image layout".to_string(),
            });
        }

        // Read manifest
        let manifest_path = image_path.join("index.json");
        if !manifest_path.exists() {
            return Err(bock_common::BockError::Config {
                message: "Missing index.json".to_string(),
            });
        }

        let manifest_content = fs::read_to_string(&manifest_path)?;

        // Calculate manifest digest
        let digest = format!("sha256:{:x}", Sha256::digest(manifest_content.as_bytes()));

        // TODO: Implement actual HTTP push to registry
        // 1. Check if blobs exist (HEAD /v2/<name>/blobs/<digest>)
        // 2. Upload missing blobs (POST /v2/<name>/blobs/uploads/, PATCH, PUT)
        // 3. Upload manifest (PUT /v2/<name>/manifests/<tag>)

        tracing::info!(
            url = %self.url,
            repository,
            tag,
            digest = %digest,
            "Image push simulated (actual HTTP calls not implemented)"
        );

        Ok(digest)
    }

    /// Pull an image from the registry.
    pub async fn pull(
        &self,
        repository: &str,
        tag: &str,
        output_dir: &Path,
    ) -> BockResult<ImageInfo> {
        tracing::info!(repository, tag, "Pulling image from registry");

        fs::create_dir_all(output_dir)?;

        // TODO: Implement actual HTTP pull from registry
        // 1. Get manifest (GET /v2/<name>/manifests/<tag>)
        // 2. Download config blob
        // 3. Download layer blobs
        // 4. Extract layers to rootfs

        tracing::info!(
            url = %self.url,
            repository,
            tag,
            "Image pull simulated (actual HTTP calls not implemented)"
        );

        // Return placeholder info
        Ok(ImageInfo {
            digest: "sha256:placeholder".to_string(),
            tag: Some(tag.to_string()),
            architecture: std::env::consts::ARCH.to_string(),
            os: "linux".to_string(),
            created: None,
            author: None,
            layer_count: 0,
            size: 0,
            entrypoint: Vec::new(),
            cmd: vec!["/bin/sh".to_string()],
            workdir: None,
            env: Vec::new(),
            exposed_ports: Vec::new(),
            labels: HashMap::new(),
        })
    }

    /// Inspect an image (get metadata without pulling).
    pub async fn inspect(&self, repository: &str, tag: &str) -> BockResult<ImageInfo> {
        tracing::info!(repository, tag, "Inspecting image");

        // TODO: Implement actual HTTP inspect
        // GET /v2/<name>/manifests/<tag>
        // GET /v2/<name>/blobs/<config-digest>

        tracing::info!(
            url = %self.url,
            repository,
            tag,
            "Image inspect simulated (actual HTTP calls not implemented)"
        );

        Ok(ImageInfo {
            digest: "sha256:placeholder".to_string(),
            tag: Some(tag.to_string()),
            architecture: std::env::consts::ARCH.to_string(),
            os: "linux".to_string(),
            created: None,
            author: None,
            layer_count: 0,
            size: 0,
            entrypoint: Vec::new(),
            cmd: vec!["/bin/sh".to_string()],
            workdir: None,
            env: Vec::new(),
            exposed_ports: Vec::new(),
            labels: HashMap::new(),
        })
    }

    /// Check if an image exists.
    pub async fn exists(&self, repository: &str, tag: &str) -> BockResult<bool> {
        // HEAD /v2/<name>/manifests/<tag>
        tracing::debug!(repository, tag, "Checking if image exists");

        // TODO: Implement actual HTTP check
        Ok(false)
    }

    /// Delete an image from the registry.
    pub async fn delete(&self, repository: &str, digest: &str) -> BockResult<()> {
        tracing::info!(repository, digest, "Deleting image from registry");

        // DELETE /v2/<name>/manifests/<digest>
        // Note: Many registries don't support this

        Ok(())
    }

    /// List tags for a repository.
    pub async fn list_tags(&self, repository: &str) -> BockResult<Vec<String>> {
        tracing::debug!(repository, "Listing tags");

        // GET /v2/<name>/tags/list

        // TODO: Implement actual HTTP call
        Ok(Vec::new())
    }
}

/// Inspect a local OCI image.
pub fn inspect_local(image_path: &Path) -> BockResult<ImageInfo> {
    let oci_dir = if image_path.join("oci-layout").exists() {
        image_path.to_path_buf()
    } else if image_path.join("oci").join("oci-layout").exists() {
        image_path.join("oci")
    } else {
        return Err(bock_common::BockError::Config {
            message: "Not a valid OCI image layout".to_string(),
        });
    };

    // Read index.json
    let index_path = oci_dir.join("index.json");
    if !index_path.exists() {
        return Err(bock_common::BockError::Config {
            message: "Missing index.json".to_string(),
        });
    }

    let index_content = fs::read_to_string(&index_path)?;
    let index: serde_json::Value =
        serde_json::from_str(&index_content).map_err(|e| bock_common::BockError::Internal {
            message: format!("Failed to parse index.json: {}", e),
        })?;

    // Get manifest digest
    let manifests =
        index["manifests"]
            .as_array()
            .ok_or_else(|| bock_common::BockError::Config {
                message: "Invalid index.json: missing manifests".to_string(),
            })?;

    if manifests.is_empty() {
        return Err(bock_common::BockError::Config {
            message: "No manifests in image".to_string(),
        });
    }

    let manifest_digest =
        manifests[0]["digest"]
            .as_str()
            .ok_or_else(|| bock_common::BockError::Config {
                message: "Missing manifest digest".to_string(),
            })?;

    // Read config from blobs
    let digest_parts: Vec<&str> = manifest_digest.split(':').collect();
    if digest_parts.len() != 2 {
        return Err(bock_common::BockError::Config {
            message: "Invalid manifest digest format".to_string(),
        });
    }

    let blobs_dir = oci_dir.join("blobs").join(digest_parts[0]);
    let config_path = blobs_dir.join(digest_parts[1]);

    if !config_path.exists() {
        return Err(bock_common::BockError::Config {
            message: format!("Config blob not found: {}", manifest_digest),
        });
    }

    let config_content = fs::read_to_string(&config_path)?;
    let config: serde_json::Value =
        serde_json::from_str(&config_content).map_err(|e| bock_common::BockError::Internal {
            message: format!("Failed to parse config: {}", e),
        })?;

    // Extract info from config
    let cfg = &config["config"];

    Ok(ImageInfo {
        digest: manifest_digest.to_string(),
        tag: None,
        architecture: config["architecture"]
            .as_str()
            .unwrap_or("unknown")
            .to_string(),
        os: config["os"].as_str().unwrap_or("linux").to_string(),
        created: config["created"].as_str().map(String::from),
        author: config["author"].as_str().map(String::from),
        layer_count: config["rootfs"]["diff_ids"]
            .as_array()
            .map(|a| a.len())
            .unwrap_or(0),
        size: calculate_image_size(&blobs_dir)?,
        entrypoint: extract_string_array(&cfg["Entrypoint"]),
        cmd: extract_string_array(&cfg["Cmd"]),
        workdir: cfg["WorkingDir"].as_str().map(String::from),
        env: extract_string_array(&cfg["Env"]),
        exposed_ports: cfg["ExposedPorts"]
            .as_object()
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default(),
        labels: cfg["Labels"]
            .as_object()
            .map(|m| {
                m.iter()
                    .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                    .collect()
            })
            .unwrap_or_default(),
    })
}

fn extract_string_array(value: &serde_json::Value) -> Vec<String> {
    value
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

fn calculate_image_size(blobs_dir: &Path) -> BockResult<u64> {
    let mut total = 0;

    if blobs_dir.exists() {
        for entry in fs::read_dir(blobs_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_file() {
                total += entry.metadata()?.len();
            }
        }
    }

    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_creation() {
        let reg = Registry::dockerhub();
        assert!(reg.url.contains("docker"));

        let ghcr = Registry::ghcr();
        assert!(ghcr.url.contains("ghcr"));
    }

    #[test]
    fn test_extract_string_array() {
        let json = serde_json::json!(["a", "b", "c"]);
        let arr = extract_string_array(&json);
        assert_eq!(arr, vec!["a", "b", "c"]);
    }
}
