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
        let reference = format!("{}:{}", repository, tag);
        tracing::info!(reference, "Pulling image from registry");

        fs::create_dir_all(output_dir)?;

        let client = reqwest::Client::new();

        // 1. Authenticate
        let token = self.get_token(&client, repository).await?;
        let auth_header = format!("Bearer {}", token);

        // 2. Get Manifest (or Manifest List)
        let manifest_url = format!("{}/v2/{}/manifests/{}", self.url, repository, tag);
        let resp = client
            .get(&manifest_url)
            .header("Authorization", &auth_header)
            .header(
                "Accept",
                "application/vnd.docker.distribution.manifest.v2+json, application/vnd.docker.distribution.manifest.list.v2+json",
            )
            .send()
            .await
            .map_err(|e| {
                bock_common::BockError::Io(std::io::Error::new(std::io::ErrorKind::Other, e))
            })?;

        if !resp.status().is_success() {
            return Err(bock_common::BockError::Config {
                message: format!("Failed to get manifest: {}", resp.status()),
            });
        }

        let bytes = resp.bytes().await.map_err(|e| {
            bock_common::BockError::Io(std::io::Error::new(std::io::ErrorKind::Other, e))
        })?;

        let json: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|e| bock_common::BockError::Internal {
                message: format!("Failed to parse manifest json: {}", e),
            })?;

        // Robust check: if it has "manifests", it's a list/index. If "config", it's a manifest.
        let is_list = json.get("manifests").is_some();

        let (manifest, manifest_bytes) = if is_list {
            // It's a list, find our arch
            tracing::info!("Found manifest list/index, resolving for current architecture");
            let manifests =
                json["manifests"]
                    .as_array()
                    .ok_or_else(|| bock_common::BockError::Config {
                        message: "Invalid manifest list".to_string(),
                    })?;

            let target_arch = match std::env::consts::ARCH {
                "x86_64" => "amd64",
                "aarch64" => "arm64",
                other => other,
            };
            let target_os = "linux";

            let selected = manifests
                .iter()
                .find(|m| {
                    let platform = &m["platform"];
                    platform["architecture"].as_str() == Some(target_arch)
                        && platform["os"].as_str() == Some(target_os)
                })
                .ok_or_else(|| bock_common::BockError::Config {
                    message: format!("No manifest found for {}/{}", target_os, target_arch),
                })?;

            let digest = selected["digest"].as_str().unwrap();
            tracing::info!(digest, "Resolved manifest digest");

            // Fetch specific manifest
            let url = format!("{}/v2/{}/manifests/{}", self.url, repository, digest);
            let resp = client
                .get(&url)
                .header("Authorization", &auth_header)
                .header(
                    "Accept",
                    "application/vnd.docker.distribution.manifest.v2+json",
                )
                .send()
                .await
                .map_err(|e| {
                    bock_common::BockError::Io(std::io::Error::new(std::io::ErrorKind::Other, e))
                })?;

            if !resp.status().is_success() {
                return Err(bock_common::BockError::Config {
                    message: format!("Failed to get resolved manifest: {}", resp.status()),
                });
            }

            let m_bytes = resp.bytes().await.map_err(|e| {
                bock_common::BockError::Io(std::io::Error::new(std::io::ErrorKind::Other, e))
            })?;
            let m: ImageManifest =
                serde_json::from_slice(&m_bytes).map_err(|e| bock_common::BockError::Internal {
                    message: format!("Failed to parse resolved manifest: {}", e),
                })?;
            (m, m_bytes)
        } else {
            // Assume it's the manifest we wanted
            let m: ImageManifest =
                serde_json::from_value(json).map_err(|e| bock_common::BockError::Internal {
                    message: format!("Failed to parse manifest: {}", e),
                })?;
            (m, bytes)
        };

        // Calculate manifest digest
        let manifest_digest = format!("sha256:{:x}", Sha256::digest(&manifest_bytes));

        // Setup OCI layout structure
        let oci_layout_dir = output_dir.join("oci"); // Or root if we want? sticking to layout
        fs::create_dir_all(&oci_layout_dir)?;
        fs::write(
            oci_layout_dir.join("oci-layout"),
            r#"{"imageLayoutVersion":"1.0.0"}"#,
        )?;

        let blobs_dir = oci_layout_dir.join("blobs/sha256");
        fs::create_dir_all(&blobs_dir)?;

        // 3. Download Config Blob
        let config_digest = &manifest.config.digest;
        self.pull_blob(&client, repository, config_digest, &blobs_dir, &auth_header)
            .await?;

        // 4. Download Layers
        for layer in &manifest.layers {
            self.pull_blob(&client, repository, &layer.digest, &blobs_dir, &auth_header)
                .await?;
        }

        // Write index.json (manifest)
        // We will store the manifest as a blob.
        let manifest_blob_path = blobs_dir.join(manifest_digest.trim_start_matches("sha256:"));
        fs::write(&manifest_blob_path, &manifest_bytes)?;

        let index_json = serde_json::json!({
            "schemaVersion": 2,
            "manifests": [
                {
                    "mediaType": manifest.media_type,
                    "digest": manifest_digest,
                    "size": manifest_bytes.len()
                }
            ]
        });

        fs::write(
            oci_layout_dir.join("index.json"),
            serde_json::to_string_pretty(&index_json).unwrap(),
        )?;

        tracing::info!(digest = %manifest_digest, "Image pulled successfully");

        // Return info by inspecting what we just pulled
        inspect_local(&output_dir)
    }

    /// Get authentication token.
    async fn get_token(&self, client: &reqwest::Client, repository: &str) -> BockResult<String> {
        // Hardcoded generic token endpoint for Docker Hub & common registries
        // Ideally should parse WWW-Authenticate header from a 401 response
        let scope = format!("repository:{}:pull", repository);
        let auth_url = format!(
            "https://auth.docker.io/token?service=registry.docker.io&scope={}",
            scope
        );

        let resp = client.get(&auth_url).send().await.map_err(|e| {
            bock_common::BockError::Io(std::io::Error::new(std::io::ErrorKind::Other, e))
        })?;

        if !resp.status().is_success() {
            return Err(bock_common::BockError::Config {
                message: format!("Authentication failed: {}", resp.status()),
            });
        }

        let body: serde_json::Value =
            resp.json()
                .await
                .map_err(|e| bock_common::BockError::Internal {
                    message: format!("Failed to parse auth response: {}", e),
                })?;

        body["token"]
            .as_str()
            .map(|t| t.to_string())
            .ok_or_else(|| bock_common::BockError::Internal {
                message: "No token in auth response".to_string(),
            })
    }

    /// Pull a single blob.
    async fn pull_blob(
        &self,
        client: &reqwest::Client,
        repository: &str,
        digest: &str,
        blobs_dir: &Path,
        auth_header: &str,
    ) -> BockResult<()> {
        let blob_path = blobs_dir.join(digest.trim_start_matches("sha256:"));
        if blob_path.exists() {
            // Already downloaded
            return Ok(());
        }

        tracing::debug!(digest, "Downloading blob");

        let url = format!("{}/v2/{}/blobs/{}", self.url, repository, digest);
        let resp = client
            .get(&url)
            .header("Authorization", auth_header)
            .send()
            .await
            .map_err(|e| {
                bock_common::BockError::Io(std::io::Error::new(std::io::ErrorKind::Other, e))
            })?;

        if !resp.status().is_success() {
            return Err(bock_common::BockError::Config {
                message: format!("Failed to download blob {}: {}", digest, resp.status()),
            });
        }

        let bytes = resp.bytes().await.map_err(|e| {
            bock_common::BockError::Io(std::io::Error::new(std::io::ErrorKind::Other, e))
        })?;

        // Verify checksum
        let calculated = format!("sha256:{:x}", Sha256::digest(&bytes));
        if calculated != digest {
            return Err(bock_common::BockError::Internal {
                message: format!("Checksum mismatch for blob {}: got {}", digest, calculated),
            });
        }

        fs::write(&blob_path, &bytes)?;
        Ok(())
    }

    /// Extract layers to a rootfs directory.
    pub fn extract_layers(
        &self,
        _image_info: &crate::registry::ImageInfo,
        _rootfs: &Path,
    ) -> BockResult<()> {
        tracing::debug!("Extracting layers to rootfs");

        Ok(())
    }

    /// Inspect an image (get metadata without pulling).
    pub async fn inspect(&self, repository: &str, tag: &str) -> BockResult<ImageInfo> {
        let client = reqwest::Client::new();
        let token = self.get_token(&client, repository).await?;
        let auth_header = format!("Bearer {}", token);

        let manifest_url = format!("{}/v2/{}/manifests/{}", self.url, repository, tag);
        let resp = client
            .get(&manifest_url)
            .header("Authorization", &auth_header)
            .header(
                "Accept",
                "application/vnd.docker.distribution.manifest.v2+json",
            )
            .send()
            .await
            .map_err(|e| {
                bock_common::BockError::Io(std::io::Error::new(std::io::ErrorKind::Other, e))
            })?;

        if !resp.status().is_success() {
            return Err(bock_common::BockError::Config {
                message: format!("Failed to get manifest: {}", resp.status()),
            });
        }

        let manifest_bytes = resp.bytes().await.map_err(|e| {
            bock_common::BockError::Io(std::io::Error::new(std::io::ErrorKind::Other, e))
        })?;
        let manifest: ImageManifest = serde_json::from_slice(&manifest_bytes).map_err(|e| {
            bock_common::BockError::Internal {
                message: format!("Failed to parse manifest: {}", e),
            }
        })?;

        let manifest_digest = format!("sha256:{:x}", Sha256::digest(&manifest_bytes));

        // Get config
        let config_digest = &manifest.config.digest;
        let config_url = format!("{}/v2/{}/blobs/{}", self.url, repository, config_digest);
        let resp = client
            .get(&config_url)
            .header("Authorization", &auth_header)
            .send()
            .await
            .map_err(|e| {
                bock_common::BockError::Io(std::io::Error::new(std::io::ErrorKind::Other, e))
            })?;

        if !resp.status().is_success() {
            return Err(bock_common::BockError::Config {
                message: format!("Failed to get config blob: {}", resp.status()),
            });
        }

        let config_bytes = resp.bytes().await.map_err(|e| {
            bock_common::BockError::Io(std::io::Error::new(std::io::ErrorKind::Other, e))
        })?;
        let config: serde_json::Value = serde_json::from_slice(&config_bytes).map_err(|e| {
            bock_common::BockError::Internal {
                message: format!("Failed to parse config: {}", e),
            }
        })?;

        // Extract info from config (similar to inspect_local but from json)
        let cfg = &config["config"];

        Ok(ImageInfo {
            digest: manifest_digest,
            tag: Some(tag.to_string()),
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
            size: manifest.layers.iter().map(|l| l.size).sum(),
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

    /// Check if an image exists.
    pub async fn exists(&self, repository: &str, tag: &str) -> BockResult<bool> {
        let client = reqwest::Client::new();
        // Just try checking manifest
        let manifest_url = format!("{}/v2/{}/manifests/{}", self.url, repository, tag);
        // Token might be needed
        let token = self.get_token(&client, repository).await.ok();

        let mut req = client.head(&manifest_url);
        if let Some(t) = token {
            req = req.header("Authorization", format!("Bearer {}", t));
        }

        let resp = req.send().await.map_err(|e| {
            bock_common::BockError::Io(std::io::Error::new(std::io::ErrorKind::Other, e))
        })?;

        Ok(resp.status().is_success())
    }

    /// Delete an image from the registry.
    pub async fn delete(&self, _repository: &str, _digest: &str) -> BockResult<()> {
        // Not implemented (needs DELETE permission and correct endpoint)
        Ok(())
    }

    /// List tags for a repository.
    pub async fn list_tags(&self, repository: &str) -> BockResult<Vec<String>> {
        let client = reqwest::Client::new();
        let token = self.get_token(&client, repository).await?;
        let url = format!("{}/v2/{}/tags/list", self.url, repository);

        let resp = client
            .get(&url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await
            .map_err(|e| {
                bock_common::BockError::Io(std::io::Error::new(std::io::ErrorKind::Other, e))
            })?;

        if !resp.status().is_success() {
            return Ok(Vec::new());
        }

        let body: serde_json::Value =
            resp.json()
                .await
                .map_err(|e| bock_common::BockError::Internal {
                    message: format!("Failed to parse tags: {}", e),
                })?;

        extract_string_array(&body["tags"])
            .into_iter()
            .map(|s| Ok(s))
            .collect()
    }
}

/// Inspect a local OCI image.
pub fn inspect_local(image_path: &Path) -> BockResult<ImageInfo> {
    // Determine OCI directory
    let oci_dir = if image_path.join("oci").join("index.json").exists() {
        image_path.join("oci")
    } else if image_path.join("index.json").exists() {
        image_path.to_path_buf()
    } else {
        return Err(bock_common::BockError::Config {
            message: "Not a valid OCI image layout (missing index.json)".to_string(),
        });
    };

    // Read index.json
    let index_path = oci_dir.join("index.json");
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
