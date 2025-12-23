use std::collections::HashMap;

use bock_common::{BockError, BockResult};
use reqwest::{Client, StatusCode};
use serde::Deserialize;

/// Registry client for pulling images.
pub struct RegistryClient {
    client: Client,
    base_url: String,
    token: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TokenResponse {
    token: Option<String>,
    // Some registries use access_token
    access_token: Option<String>,
}

impl RegistryClient {
    /// Create a new registry client.
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.into(),
            token: None,
        }
    }

    /// Create a client for Docker Hub.
    pub fn docker_hub() -> Self {
        Self::new("https://registry-1.docker.io")
    }

    /// Pull an image manifest.
    pub async fn get_manifest(&mut self, name: &str, reference: &str) -> BockResult<String> {
        let url = format!("{}/v2/{}/manifests/{}", self.base_url, name, reference);
        tracing::debug!(url = %url, "Getting manifest");

        let response = self
            .client
            .get(&url)
            .header("Accept", "application/vnd.docker.distribution.manifest.v2+json")
            .header("Accept", "application/vnd.oci.image.manifest.v1+json")
            .bearer_auth(self.token.as_deref().unwrap_or(""))
            .send()
            .await
            .map_err(|e| BockError::Network {
                message: format!("Failed to request manifest: {}", e),
            })?;

        if response.status() == StatusCode::UNAUTHORIZED {
            self.authenticate(name, &response).await?;
            // Retry
            return Box::pin(self.get_manifest(name, reference)).await;
        }

        if !response.status().is_success() {
            return Err(BockError::Registry {
                message: format!("Registry error: {}", response.status()),
            });
        }

        let text = response.text().await.map_err(|e| BockError::Network {
            message: format!("Failed to read manifest body: {}", e),
        })?;

        Ok(text)
    }

    /// Pull a blob.
    pub async fn get_blob(&mut self, name: &str, digest: &str) -> BockResult<Vec<u8>> {
        let url = format!("{}/v2/{}/blobs/{}", self.base_url, name, digest);
        tracing::debug!(url = %url, "Getting blob");

        let response = self
            .client
            .get(&url)
            .bearer_auth(self.token.as_deref().unwrap_or(""))
            .send()
            .await
            .map_err(|e| BockError::Network {
                message: format!("Failed to request blob: {}", e),
            })?;

        if response.status() == StatusCode::UNAUTHORIZED {
            self.authenticate(name, &response).await?;
             // Retry
            return Box::pin(self.get_blob(name, digest)).await;
        }

        if !response.status().is_success() {
            return Err(BockError::Registry {
                message: format!("Registry error: {}", response.status()),
            });
        }

        let bytes = response.bytes().await.map_err(|e| BockError::Network {
            message: format!("Failed to read blob body: {}", e),
        })?;

        Ok(bytes.to_vec())
    }

    async fn authenticate(&mut self, repository: &str, response: &reqwest::Response) -> BockResult<()> {
        let auth_header = response
            .headers()
            .get("Www-Authenticate")
            .ok_or_else(|| BockError::Registry {
                message: "Missing Www-Authenticate header".to_string(),
            })?
            .to_str()
            .map_err(|_| BockError::Registry {
                message: "Invalid Www-Authenticate header".to_string(),
            })?;

        tracing::debug!(header = auth_header, "Authenticating");

        // Parse Bearer realm="...",service="...",scope="..."
        let parts: Vec<&str> = auth_header
            .trim_start_matches("Bearer ")
            .split(',')
            .collect();

        let mut params = HashMap::new();
        for part in parts {
            let kv: Vec<&str> = part.splitn(2, '=').collect();
            if kv.len() == 2 {
                let key = kv[0].trim();
                let value = kv[1].trim().trim_matches('"');
                params.insert(key, value);
            }
        }

        let realm = params.get("realm").ok_or_else(|| BockError::Registry {
            message: "Missing realm in Www-Authenticate".to_string(),
        })?;
        let service = params.get("service").ok_or_else(|| BockError::Registry {
            message: "Missing service in Www-Authenticate".to_string(),
        })?;

        // Construct scope if not present or incorrect
        // Docker Hub typically returns scope in the header, but sometimes we need to construct it
        // e.g. repository:library/alpine:pull
        let scope = params.get("scope").map(|s| s.to_string()).unwrap_or_else(|| {
             format!("repository:{}:pull", repository)
        });

        let url = format!("{}?service={}&scope={}", realm, service, scope);
        tracing::debug!(url = %url, "Requesting token");

        let token_resp: TokenResponse = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| BockError::Network {
                message: format!("Failed to request token: {}", e),
            })?
            .json()
            .await
            .map_err(|e| BockError::Registry {
                message: format!("Failed to parse token response: {}", e),
            })?;

        self.token = Some(token_resp.token.or(token_resp.access_token).ok_or_else(|| {
            BockError::Registry {
                message: "No token in response".to_string(),
            }
        })?);

        Ok(())
    }
}
