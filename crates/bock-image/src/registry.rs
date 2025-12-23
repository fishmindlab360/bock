//! Container registry client.

use bock_common::BockResult;

/// Registry client for pulling images.
pub struct RegistryClient {
    /// Base URL of the registry.
    base_url: String,
}

impl RegistryClient {
    /// Create a new registry client.
    #[must_use]
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
        }
    }

    /// Create a client for Docker Hub.
    #[must_use]
    pub fn docker_hub() -> Self {
        Self::new("https://registry-1.docker.io")
    }

    /// Pull an image manifest.
    pub async fn get_manifest(&self, name: &str, reference: &str) -> BockResult<String> {
        tracing::debug!(name, reference, "Getting manifest");
        // TODO: Implement
        Ok(String::new())
    }

    /// Pull a blob.
    pub async fn get_blob(&self, name: &str, digest: &str) -> BockResult<Vec<u8>> {
        tracing::debug!(name, digest, "Getting blob");
        // TODO: Implement
        Ok(Vec::new())
    }
}
