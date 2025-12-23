//! OCI Image Specification types.
//!
//! Based on the OCI Image Specification v1.1.0:
//! <https://github.com/opencontainers/image-spec>

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// OCI Image Manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageManifest {
    /// Schema version (must be 2).
    pub schema_version: u32,
    /// Media type of the manifest.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
    /// Artifact type (for artifacts).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact_type: Option<String>,
    /// Image configuration descriptor.
    pub config: Descriptor,
    /// Image layers.
    pub layers: Vec<Descriptor>,
    /// Subject (for referrers).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<Descriptor>,
    /// Annotations.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub annotations: HashMap<String, String>,
}

/// OCI Image Index (multi-architecture manifest).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageIndex {
    /// Schema version (must be 2).
    pub schema_version: u32,
    /// Media type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,
    /// Artifact type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact_type: Option<String>,
    /// Manifest list.
    pub manifests: Vec<ManifestDescriptor>,
    /// Subject (for referrers).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject: Option<Descriptor>,
    /// Annotations.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub annotations: HashMap<String, String>,
}

/// Content descriptor.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Descriptor {
    /// Media type of the referenced content.
    pub media_type: String,
    /// Content digest.
    pub digest: String,
    /// Content size in bytes.
    pub size: i64,
    /// URLs for downloading.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub urls: Vec<String>,
    /// Annotations.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub annotations: HashMap<String, String>,
    /// Data (base64-encoded, for small content).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<String>,
    /// Artifact type.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact_type: Option<String>,
}

/// Manifest descriptor with platform information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestDescriptor {
    /// Base descriptor.
    #[serde(flatten)]
    pub descriptor: Descriptor,
    /// Platform information.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform: Option<Platform>,
}

/// Platform specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Platform {
    /// Operating system.
    pub os: String,
    /// Architecture.
    pub architecture: String,
    /// OS version.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub os_version: Option<String>,
    /// OS features.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub os_features: Vec<String>,
    /// Architecture variant.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
}

impl Platform {
    /// Create a platform for linux/amd64.
    #[must_use]
    pub fn linux_amd64() -> Self {
        Self {
            os: "linux".to_string(),
            architecture: "amd64".to_string(),
            os_version: None,
            os_features: Vec::new(),
            variant: None,
        }
    }

    /// Create a platform for linux/arm64.
    #[must_use]
    pub fn linux_arm64() -> Self {
        Self {
            os: "linux".to_string(),
            architecture: "arm64".to_string(),
            os_version: None,
            os_features: Vec::new(),
            variant: None,
        }
    }
}

/// OCI Image Configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageConfig {
    /// Creation timestamp.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,
    /// Author.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// Architecture.
    pub architecture: String,
    /// Operating system.
    pub os: String,
    /// OS version.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub os_version: Option<String>,
    /// OS features.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub os_features: Vec<String>,
    /// Architecture variant.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
    /// Execution parameters.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<ExecutionConfig>,
    /// Rootfs information.
    pub rootfs: RootFs,
    /// History entries.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub history: Vec<HistoryEntry>,
}

/// Execution configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct ExecutionConfig {
    /// User.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<String>,
    /// Exposed ports.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub exposed_ports: HashMap<String, HashMap<(), ()>>,
    /// Environment variables.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub env: Vec<String>,
    /// Entrypoint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub entrypoint: Option<Vec<String>>,
    /// Default command.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cmd: Option<Vec<String>>,
    /// Volumes.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub volumes: HashMap<String, HashMap<(), ()>>,
    /// Working directory.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_dir: Option<String>,
    /// Labels.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub labels: HashMap<String, String>,
    /// Stop signal.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_signal: Option<String>,
    /// Memory limit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory: Option<i64>,
    /// Memory swap limit.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub memory_swap: Option<i64>,
    /// CPU shares.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_shares: Option<i64>,
    /// Healthcheck.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub healthcheck: Option<Healthcheck>,
}

/// Healthcheck configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Healthcheck {
    /// Healthcheck command.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub test: Option<Vec<String>>,
    /// Interval between checks (nanoseconds).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub interval: Option<i64>,
    /// Timeout for each check (nanoseconds).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout: Option<i64>,
    /// Number of retries before unhealthy.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retries: Option<i32>,
    /// Start period (grace period).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_period: Option<i64>,
}

/// Root filesystem information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RootFs {
    /// Type (must be "layers").
    #[serde(rename = "type")]
    pub fs_type: String,
    /// Layer diff IDs (uncompressed digests).
    pub diff_ids: Vec<String>,
}

impl Default for RootFs {
    fn default() -> Self {
        Self {
            fs_type: "layers".to_string(),
            diff_ids: Vec::new(),
        }
    }
}

/// History entry for an image layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct HistoryEntry {
    /// Creation timestamp.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created: Option<String>,
    /// Author.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    /// Command that created this layer.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_by: Option<String>,
    /// Comment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comment: Option<String>,
    /// Whether this is an empty layer.
    #[serde(default)]
    pub empty_layer: bool,
}

/// Common media types.
pub mod media_types {
    /// OCI image manifest media type.
    pub const MANIFEST: &str = "application/vnd.oci.image.manifest.v1+json";
    /// OCI image index media type.
    pub const INDEX: &str = "application/vnd.oci.image.index.v1+json";
    /// OCI image config media type.
    pub const CONFIG: &str = "application/vnd.oci.image.config.v1+json";
    /// OCI layer media type (tar+gzip).
    pub const LAYER_TAR_GZIP: &str = "application/vnd.oci.image.layer.v1.tar+gzip";
    /// OCI layer media type (tar+zstd).
    pub const LAYER_TAR_ZSTD: &str = "application/vnd.oci.image.layer.v1.tar+zstd";
    /// OCI layer media type (uncompressed tar).
    pub const LAYER_TAR: &str = "application/vnd.oci.image.layer.v1.tar";

    /// Docker manifest v2 schema 2 media type.
    pub const DOCKER_MANIFEST: &str = "application/vnd.docker.distribution.manifest.v2+json";
    /// Docker manifest list media type.
    pub const DOCKER_INDEX: &str = "application/vnd.docker.distribution.manifest.list.v2+json";
    /// Docker image config media type.
    pub const DOCKER_CONFIG: &str = "application/vnd.docker.container.image.v1+json";
    /// Docker layer media type.
    pub const DOCKER_LAYER: &str = "application/vnd.docker.image.rootfs.diff.tar.gzip";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_serialization() {
        let manifest = ImageManifest {
            schema_version: 2,
            media_type: Some(media_types::MANIFEST.to_string()),
            artifact_type: None,
            config: Descriptor {
                media_type: media_types::CONFIG.to_string(),
                digest: "sha256:abc123".to_string(),
                size: 1024,
                urls: Vec::new(),
                annotations: HashMap::new(),
                data: None,
                artifact_type: None,
            },
            layers: vec![Descriptor {
                media_type: media_types::LAYER_TAR_GZIP.to_string(),
                digest: "sha256:layer1".to_string(),
                size: 10240,
                urls: Vec::new(),
                annotations: HashMap::new(),
                data: None,
                artifact_type: None,
            }],
            subject: None,
            annotations: HashMap::new(),
        };

        let json = serde_json::to_string_pretty(&manifest).unwrap();
        assert!(json.contains("schemaVersion"));
        assert!(json.contains("sha256:abc123"));
    }

    #[test]
    fn platform_creation() {
        let platform = Platform::linux_amd64();
        assert_eq!(platform.os, "linux");
        assert_eq!(platform.architecture, "amd64");
    }
}
