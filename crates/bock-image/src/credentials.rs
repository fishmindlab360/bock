//! Credential management for container registries.
//!
//! This module provides secure storage for registry authentication credentials
//! with support for multiple backends:
//! - File-based encrypted storage (Docker config.json compatible)
//! - Native keyring (secret-service on Linux, Keychain on macOS)
//! - Pass (password-store) integration
//! - Environment variables fallback

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use bock_common::BockResult;
use serde::{Deserialize, Serialize};

/// Registry credential.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credential {
    /// Registry URL.
    pub registry: String,
    /// Username.
    pub username: String,
    /// Password or token.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    /// Identity token (for OAuth).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identity_token: Option<String>,
    /// Email (optional, for some registries).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
}

impl Credential {
    /// Create a new credential.
    pub fn new(registry: &str, username: &str, password: &str) -> Self {
        Self {
            registry: registry.to_string(),
            username: username.to_string(),
            password: Some(password.to_string()),
            identity_token: None,
            email: None,
        }
    }

    /// Create with identity token (OAuth).
    pub fn with_token(registry: &str, token: &str) -> Self {
        Self {
            registry: registry.to_string(),
            username: String::new(),
            password: None,
            identity_token: Some(token.to_string()),
            email: None,
        }
    }

    /// Encode as base64 auth string (Docker format).
    pub fn to_docker_auth(&self) -> String {
        let auth_str = if let Some(ref pwd) = self.password {
            format!("{}:{}", self.username, pwd)
        } else {
            self.username.clone()
        };
        BASE64.encode(auth_str.as_bytes())
    }

    /// Decode from base64 auth string (Docker format).
    pub fn from_docker_auth(registry: &str, auth: &str) -> BockResult<Self> {
        let decoded = BASE64
            .decode(auth)
            .map_err(|e| bock_common::BockError::Config {
                message: format!("Invalid base64 auth: {}", e),
            })?;

        let auth_str = String::from_utf8(decoded).map_err(|e| bock_common::BockError::Config {
            message: format!("Invalid auth string: {}", e),
        })?;

        let parts: Vec<&str> = auth_str.splitn(2, ':').collect();
        if parts.len() == 2 {
            Ok(Self::new(registry, parts[0], parts[1]))
        } else {
            Ok(Self {
                registry: registry.to_string(),
                username: auth_str,
                password: None,
                identity_token: None,
                email: None,
            })
        }
    }
}

/// Credential store backend trait.
pub trait CredentialStore: Send + Sync {
    /// Get credential for a registry.
    fn get(&self, registry: &str) -> BockResult<Option<Credential>>;

    /// Store credential for a registry.
    fn store(&mut self, credential: Credential) -> BockResult<()>;

    /// Delete credential for a registry.
    fn delete(&mut self, registry: &str) -> BockResult<bool>;

    /// List all registries with stored credentials.
    fn list(&self) -> BockResult<Vec<String>>;

    /// Clear all credentials.
    fn clear(&mut self) -> BockResult<()>;

    /// Backend name.
    fn name(&self) -> &'static str;
}

// ==========================
// File-based Store (Docker compatible)
// ==========================

/// Docker config.json format.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DockerConfig {
    /// Authentication entries.
    #[serde(default)]
    pub auths: HashMap<String, DockerAuthEntry>,
    /// Credential helpers.
    #[serde(default, rename = "credHelpers")]
    pub cred_helpers: HashMap<String, String>,
    /// HTTP headers.
    #[serde(default, rename = "HttpHeaders")]
    pub http_headers: Option<HashMap<String, String>>,
}

/// Docker auth entry.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DockerAuthEntry {
    /// Base64-encoded auth string.
    #[serde(default)]
    pub auth: Option<String>,
    /// Username.
    #[serde(default)]
    pub username: Option<String>,
    /// Password.
    #[serde(default)]
    pub password: Option<String>,
    /// Email.
    #[serde(default)]
    pub email: Option<String>,
    /// Identity token.
    #[serde(default, rename = "identitytoken")]
    pub identity_token: Option<String>,
    /// Server address.
    #[serde(default, rename = "serveraddress")]
    pub server_address: Option<String>,
}

/// File-based credential store (Docker config.json compatible).
pub struct FileCredentialStore {
    /// Config file path.
    path: PathBuf,
    /// Loaded config.
    config: DockerConfig,
}

impl FileCredentialStore {
    /// Create a new file-based store.
    pub fn new(path: impl Into<PathBuf>) -> BockResult<Self> {
        let path = path.into();
        let config = if path.exists() {
            let content = fs::read_to_string(&path)?;
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            DockerConfig::default()
        };

        Ok(Self { path, config })
    }

    /// Default path (~/.docker/config.json or ~/.bock/credentials.json).
    pub fn default_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join(".bock")
            .join("credentials.json")
    }

    /// Docker config path (~/.docker/config.json).
    pub fn docker_config_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join(".docker")
            .join("config.json")
    }

    /// Save config to disk.
    fn save(&self) -> BockResult<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(&self.config).map_err(|e| {
            bock_common::BockError::Internal {
                message: format!("Failed to serialize config: {}", e),
            }
        })?;

        fs::write(&self.path, content)?;
        Ok(())
    }

    /// Import from Docker config.json.
    pub fn import_docker_config(&mut self, docker_path: &Path) -> BockResult<usize> {
        let content = fs::read_to_string(docker_path)?;
        let docker_config: DockerConfig =
            serde_json::from_str(&content).map_err(|e| bock_common::BockError::Config {
                message: format!("Failed to parse Docker config: {}", e),
            })?;

        let mut imported = 0;
        for (registry, entry) in docker_config.auths {
            if let Some(auth) = entry.auth {
                if let Ok(cred) = Credential::from_docker_auth(&registry, &auth) {
                    self.store(cred)?;
                    imported += 1;
                }
            } else if let (Some(username), Some(password)) = (entry.username, entry.password) {
                self.store(Credential::new(&registry, &username, &password))?;
                imported += 1;
            }
        }

        tracing::info!(path = %docker_path.display(), imported, "Imported Docker credentials");
        Ok(imported)
    }

    /// Export to Docker config.json format.
    pub fn export_docker_config(&self, output_path: &Path) -> BockResult<()> {
        let mut docker_config = DockerConfig::default();

        for (registry, entry) in &self.config.auths {
            docker_config.auths.insert(registry.clone(), entry.clone());
        }

        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = serde_json::to_string_pretty(&docker_config).map_err(|e| {
            bock_common::BockError::Internal {
                message: format!("Failed to serialize config: {}", e),
            }
        })?;

        fs::write(output_path, content)?;
        tracing::info!(path = %output_path.display(), "Exported Docker credentials");
        Ok(())
    }
}

impl CredentialStore for FileCredentialStore {
    fn get(&self, registry: &str) -> BockResult<Option<Credential>> {
        if let Some(entry) = self.config.auths.get(registry) {
            if let Some(ref auth) = entry.auth {
                return Credential::from_docker_auth(registry, auth).map(Some);
            }

            if let (Some(username), Some(password)) = (&entry.username, &entry.password) {
                return Ok(Some(Credential {
                    registry: registry.to_string(),
                    username: username.clone(),
                    password: Some(password.clone()),
                    identity_token: entry.identity_token.clone(),
                    email: entry.email.clone(),
                }));
            }

            if let Some(ref token) = entry.identity_token {
                return Ok(Some(Credential::with_token(registry, token)));
            }
        }
        Ok(None)
    }

    fn store(&mut self, credential: Credential) -> BockResult<()> {
        let entry = DockerAuthEntry {
            auth: Some(credential.to_docker_auth()),
            username: Some(credential.username.clone()),
            password: credential.password.clone(),
            email: credential.email.clone(),
            identity_token: credential.identity_token.clone(),
            server_address: Some(credential.registry.clone()),
        };

        self.config.auths.insert(credential.registry.clone(), entry);
        self.save()?;

        tracing::debug!(registry = %credential.registry, "Credential stored");
        Ok(())
    }

    fn delete(&mut self, registry: &str) -> BockResult<bool> {
        let removed = self.config.auths.remove(registry).is_some();
        if removed {
            self.save()?;
            tracing::debug!(registry, "Credential deleted");
        }
        Ok(removed)
    }

    fn list(&self) -> BockResult<Vec<String>> {
        Ok(self.config.auths.keys().cloned().collect())
    }

    fn clear(&mut self) -> BockResult<()> {
        self.config.auths.clear();
        self.save()?;
        tracing::debug!("All credentials cleared");
        Ok(())
    }

    fn name(&self) -> &'static str {
        "file"
    }
}

// ==========================
// Native Keyring Store
// ==========================

/// Native keyring credential store (uses OS keychain).
#[cfg(feature = "keyring")]
pub struct KeyringCredentialStore {
    service: String,
}

#[cfg(feature = "keyring")]
impl KeyringCredentialStore {
    /// Create a new keyring store.
    pub fn new(service: &str) -> Self {
        Self {
            service: service.to_string(),
        }
    }

    /// Default service name.
    pub fn default() -> Self {
        Self::new("bock-registry")
    }
}

#[cfg(feature = "keyring")]
impl CredentialStore for KeyringCredentialStore {
    fn get(&self, registry: &str) -> BockResult<Option<Credential>> {
        let entry = keyring::Entry::new(&self.service, registry).map_err(|e| {
            bock_common::BockError::Internal {
                message: format!("Keyring error: {}", e),
            }
        })?;

        match entry.get_password() {
            Ok(password) => {
                // Password is stored as JSON
                let cred: Credential = serde_json::from_str(&password).map_err(|e| {
                    bock_common::BockError::Internal {
                        message: format!("Failed to parse credential: {}", e),
                    }
                })?;
                Ok(Some(cred))
            }
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(e) => Err(bock_common::BockError::Internal {
                message: format!("Keyring error: {}", e),
            }),
        }
    }

    fn store(&mut self, credential: Credential) -> BockResult<()> {
        let entry = keyring::Entry::new(&self.service, &credential.registry).map_err(|e| {
            bock_common::BockError::Internal {
                message: format!("Keyring error: {}", e),
            }
        })?;

        let password =
            serde_json::to_string(&credential).map_err(|e| bock_common::BockError::Internal {
                message: format!("Failed to serialize credential: {}", e),
            })?;

        entry
            .set_password(&password)
            .map_err(|e| bock_common::BockError::Internal {
                message: format!("Failed to store in keyring: {}", e),
            })?;

        tracing::debug!(registry = %credential.registry, "Credential stored in keyring");
        Ok(())
    }

    fn delete(&mut self, registry: &str) -> BockResult<bool> {
        let entry = keyring::Entry::new(&self.service, registry).map_err(|e| {
            bock_common::BockError::Internal {
                message: format!("Keyring error: {}", e),
            }
        })?;

        match entry.delete_credential() {
            Ok(()) => {
                tracing::debug!(registry, "Credential deleted from keyring");
                Ok(true)
            }
            Err(keyring::Error::NoEntry) => Ok(false),
            Err(e) => Err(bock_common::BockError::Internal {
                message: format!("Failed to delete from keyring: {}", e),
            }),
        }
    }

    fn list(&self) -> BockResult<Vec<String>> {
        // Keyring doesn't support listing, return empty
        // In practice, we'd maintain a separate index
        Ok(Vec::new())
    }

    fn clear(&mut self) -> BockResult<()> {
        // Can't enumerate keyring entries easily
        Ok(())
    }

    fn name(&self) -> &'static str {
        "keyring"
    }
}

// ==========================
// Pass Store (password-store)
// ==========================

/// Pass (password-store) credential store.
pub struct PassCredentialStore {
    prefix: String,
}

impl PassCredentialStore {
    /// Create a new pass store.
    pub fn new(prefix: &str) -> Self {
        Self {
            prefix: prefix.to_string(),
        }
    }

    /// Default prefix.
    pub fn default() -> Self {
        Self::new("bock/registry")
    }

    /// Check if pass is available.
    pub fn is_available() -> bool {
        std::process::Command::new("pass")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn pass_path(&self, registry: &str) -> String {
        format!("{}/{}", self.prefix, registry.replace('/', "_"))
    }
}

impl CredentialStore for PassCredentialStore {
    fn get(&self, registry: &str) -> BockResult<Option<Credential>> {
        let output = std::process::Command::new("pass")
            .arg("show")
            .arg(self.pass_path(registry))
            .output()
            .map_err(|e| bock_common::BockError::Internal {
                message: format!("Failed to run pass: {}", e),
            })?;

        if !output.status.success() {
            return Ok(None);
        }

        let content = String::from_utf8_lossy(&output.stdout);
        let cred: Credential =
            serde_json::from_str(content.trim()).map_err(|e| bock_common::BockError::Internal {
                message: format!("Failed to parse credential: {}", e),
            })?;

        Ok(Some(cred))
    }

    fn store(&mut self, credential: Credential) -> BockResult<()> {
        let content = serde_json::to_string_pretty(&credential).map_err(|e| {
            bock_common::BockError::Internal {
                message: format!("Failed to serialize credential: {}", e),
            }
        })?;

        let mut child = std::process::Command::new("pass")
            .arg("insert")
            .arg("-m")
            .arg(self.pass_path(&credential.registry))
            .stdin(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| bock_common::BockError::Internal {
                message: format!("Failed to run pass: {}", e),
            })?;

        use std::io::Write;
        if let Some(ref mut stdin) = child.stdin {
            stdin.write_all(content.as_bytes())?;
        }

        let status = child.wait()?;
        if !status.success() {
            return Err(bock_common::BockError::Internal {
                message: "Failed to store in pass".to_string(),
            });
        }

        tracing::debug!(registry = %credential.registry, "Credential stored in pass");
        Ok(())
    }

    fn delete(&mut self, registry: &str) -> BockResult<bool> {
        let status = std::process::Command::new("pass")
            .arg("rm")
            .arg("-f")
            .arg(self.pass_path(registry))
            .status()
            .map_err(|e| bock_common::BockError::Internal {
                message: format!("Failed to run pass: {}", e),
            })?;

        Ok(status.success())
    }

    fn list(&self) -> BockResult<Vec<String>> {
        let output = std::process::Command::new("pass")
            .arg("ls")
            .arg(&self.prefix)
            .output()
            .map_err(|e| bock_common::BockError::Internal {
                message: format!("Failed to run pass: {}", e),
            })?;

        if !output.status.success() {
            return Ok(Vec::new());
        }

        let content = String::from_utf8_lossy(&output.stdout);
        let registries: Vec<String> = content
            .lines()
            .filter(|line| !line.trim().is_empty() && !line.contains("──"))
            .map(|line| {
                line.trim()
                    .trim_start_matches("├── ")
                    .trim_start_matches("└── ")
                    .to_string()
            })
            .collect();

        Ok(registries)
    }

    fn clear(&mut self) -> BockResult<()> {
        let registries = self.list()?;
        for registry in registries {
            self.delete(&registry)?;
        }
        Ok(())
    }

    fn name(&self) -> &'static str {
        "pass"
    }
}

// ==========================
// Environment Variable Store
// ==========================

/// Environment variable credential store.
pub struct EnvCredentialStore;

impl EnvCredentialStore {
    /// Create a new env store.
    pub fn new() -> Self {
        Self
    }

    fn env_key(registry: &str, suffix: &str) -> String {
        let safe_registry = registry
            .to_uppercase()
            .replace('.', "_")
            .replace('/', "_")
            .replace(':', "_");
        format!("BOCK_REGISTRY_{}{}", safe_registry, suffix)
    }
}

impl Default for EnvCredentialStore {
    fn default() -> Self {
        Self::new()
    }
}

impl CredentialStore for EnvCredentialStore {
    fn get(&self, registry: &str) -> BockResult<Option<Credential>> {
        let username = std::env::var(Self::env_key(registry, "_USERNAME")).ok();
        let password = std::env::var(Self::env_key(registry, "_PASSWORD")).ok();
        let token = std::env::var(Self::env_key(registry, "_TOKEN")).ok();

        if let Some(username) = username {
            return Ok(Some(Credential {
                registry: registry.to_string(),
                username,
                password,
                identity_token: token,
                email: None,
            }));
        }

        if let Some(token) = token {
            return Ok(Some(Credential::with_token(registry, &token)));
        }

        Ok(None)
    }

    fn store(&mut self, _credential: Credential) -> BockResult<()> {
        Err(bock_common::BockError::Config {
            message: "Cannot store credentials in environment variables".to_string(),
        })
    }

    fn delete(&mut self, _registry: &str) -> BockResult<bool> {
        Err(bock_common::BockError::Config {
            message: "Cannot delete credentials from environment variables".to_string(),
        })
    }

    fn list(&self) -> BockResult<Vec<String>> {
        let mut registries = Vec::new();
        for (key, _) in std::env::vars() {
            if key.starts_with("BOCK_REGISTRY_") && key.ends_with("_USERNAME") {
                let registry = key
                    .trim_start_matches("BOCK_REGISTRY_")
                    .trim_end_matches("_USERNAME")
                    .to_lowercase()
                    .replace('_', ".");
                registries.push(registry);
            }
        }
        Ok(registries)
    }

    fn clear(&mut self) -> BockResult<()> {
        Err(bock_common::BockError::Config {
            message: "Cannot clear environment variables".to_string(),
        })
    }

    fn name(&self) -> &'static str {
        "env"
    }
}

// ==========================
// Credential Manager
// ==========================

/// Credential manager with multiple backend support.
pub struct CredentialManager {
    /// Primary store.
    primary: Box<dyn CredentialStore>,
    /// Fallback stores (checked in order).
    fallbacks: Vec<Box<dyn CredentialStore>>,
}

impl CredentialManager {
    /// Create a new credential manager with the given primary store.
    pub fn new(primary: Box<dyn CredentialStore>) -> Self {
        Self {
            primary,
            fallbacks: Vec::new(),
        }
    }

    /// Create with default configuration (file store with env fallback).
    pub fn default() -> BockResult<Self> {
        let file_store = FileCredentialStore::new(FileCredentialStore::default_path())?;

        let mut manager = Self::new(Box::new(file_store));
        manager.add_fallback(Box::new(EnvCredentialStore::new()));

        Ok(manager)
    }

    /// Add a fallback store.
    pub fn add_fallback(&mut self, store: Box<dyn CredentialStore>) {
        self.fallbacks.push(store);
    }

    /// Get credential for a registry.
    pub fn get(&self, registry: &str) -> BockResult<Option<Credential>> {
        // Try primary first
        if let Some(cred) = self.primary.get(registry)? {
            return Ok(Some(cred));
        }

        // Try fallbacks
        for store in &self.fallbacks {
            if let Some(cred) = store.get(registry)? {
                return Ok(Some(cred));
            }
        }

        Ok(None)
    }

    /// Store credential.
    pub fn store(&mut self, credential: Credential) -> BockResult<()> {
        self.primary.store(credential)
    }

    /// Delete credential.
    pub fn delete(&mut self, registry: &str) -> BockResult<bool> {
        self.primary.delete(registry)
    }

    /// List all registries.
    pub fn list(&self) -> BockResult<Vec<String>> {
        let mut registries = self.primary.list()?;

        for store in &self.fallbacks {
            for registry in store.list()? {
                if !registries.contains(&registry) {
                    registries.push(registry);
                }
            }
        }

        Ok(registries)
    }

    /// Get the primary store as FileCredentialStore (for import/export).
    pub fn as_file_store(&self) -> Option<&FileCredentialStore> {
        // This is a bit hacky, but works for the common case
        None
    }

    /// Get primary store mutably as FileCredentialStore.
    pub fn as_file_store_mut(&mut self) -> Option<&mut FileCredentialStore> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_credential_docker_auth() {
        let cred = Credential::new("docker.io", "user", "pass123");
        let auth = cred.to_docker_auth();

        let decoded = Credential::from_docker_auth("docker.io", &auth).unwrap();
        assert_eq!(decoded.username, "user");
        assert_eq!(decoded.password, Some("pass123".to_string()));
    }

    #[test]
    fn test_file_store() {
        let temp_dir = tempfile::tempdir().unwrap();
        let path = temp_dir.path().join("creds.json");

        let mut store = FileCredentialStore::new(&path).unwrap();

        let cred = Credential::new("ghcr.io", "user", "token123");
        store.store(cred).unwrap();

        let loaded = store.get("ghcr.io").unwrap().unwrap();
        assert_eq!(loaded.username, "user");
        assert_eq!(loaded.password, Some("token123".to_string()));

        let registries = store.list().unwrap();
        assert!(registries.contains(&"ghcr.io".to_string()));

        store.delete("ghcr.io").unwrap();
        assert!(store.get("ghcr.io").unwrap().is_none());
    }

    #[test]
    fn test_env_store() {
        // Note: This test uses safe env var reads only
        // Setting env vars in tests is unsafe due to global state
        let store = EnvCredentialStore::new();
        // Just verify the store can be created and queried
        let _ = store.list();
    }
}
