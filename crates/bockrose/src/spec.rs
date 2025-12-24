//! bockrose specification parsing.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// bockrose specification (bockrose.yaml).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BockoseSpec {
    /// Specification version.
    #[serde(default = "default_version")]
    pub version: String,

    /// Stack name.
    #[serde(default)]
    pub name: Option<String>,

    /// Global configuration.
    #[serde(default)]
    pub config: GlobalConfig,

    /// Networks.
    #[serde(default)]
    pub networks: HashMap<String, NetworkSpec>,

    /// Volumes.
    #[serde(default)]
    pub volumes: HashMap<String, VolumeSpec>,

    /// Services.
    pub services: HashMap<String, ServiceSpec>,

    /// Base path (directory containing the spec file).
    #[serde(skip)]
    pub base_path: PathBuf,
}

fn default_version() -> String {
    "1".to_string()
}

/// Global configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct GlobalConfig {
    /// Default restart policy.
    #[serde(default)]
    pub restart_policy: Option<String>,
    /// Logging configuration.
    #[serde(default)]
    pub logging: Option<LoggingConfig>,
}

/// Logging configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    /// Logging driver.
    #[serde(default = "default_log_driver")]
    pub driver: String,
    /// Driver options.
    #[serde(default)]
    pub options: HashMap<String, String>,
}

fn default_log_driver() -> String {
    "json-file".to_string()
}

/// Network specification.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetworkSpec {
    /// Network driver.
    #[serde(default = "default_network_driver")]
    pub driver: String,
    /// Internal network (no external access).
    #[serde(default)]
    pub internal: bool,
    /// IPAM configuration.
    #[serde(default)]
    pub ipam: Option<IpamConfig>,
}

fn default_network_driver() -> String {
    "bridge".to_string()
}

/// IPAM configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpamConfig {
    /// Subnet.
    #[serde(default)]
    pub subnet: Option<String>,
    /// Gateway.
    #[serde(default)]
    pub gateway: Option<String>,
}

/// Volume specification.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VolumeSpec {
    /// Volume driver.
    #[serde(default = "default_volume_driver")]
    pub driver: String,
    /// Driver options.
    #[serde(default)]
    pub driver_opts: HashMap<String, String>,
}

fn default_volume_driver() -> String {
    "local".to_string()
}

/// Service specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceSpec {
    /// Image to use.
    #[serde(default)]
    pub image: Option<String>,

    /// Build configuration.
    #[serde(default)]
    pub build: Option<BuildConfig>,

    /// Command override.
    #[serde(default)]
    pub command: Vec<String>,

    /// Entrypoint override.
    #[serde(default)]
    pub entrypoint: Vec<String>,

    /// Environment variables.
    #[serde(default)]
    pub environment: HashMap<String, String>,

    /// Volume mounts.
    #[serde(default)]
    pub volumes: Vec<String>,

    /// Port mappings.
    #[serde(default)]
    pub ports: Vec<String>,

    /// Networks to connect to.
    #[serde(default)]
    pub networks: Vec<String>,

    /// Service dependencies.
    #[serde(default)]
    pub depends_on: DependsOn,

    /// Health check.
    #[serde(default)]
    pub healthcheck: Option<HealthcheckSpec>,

    /// Restart policy.
    #[serde(default)]
    pub restart: Option<String>,

    /// Deploy configuration.
    #[serde(default)]
    pub deploy: Option<DeployConfig>,

    /// Resource limits.
    #[serde(default)]
    pub resources: Option<ResourceConfig>,
}

/// Build configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum BuildConfig {
    /// Simple context path.
    Path(String),
    /// Full build configuration.
    Full {
        /// Build context.
        context: String,
        /// Bockfile path.
        #[serde(default)]
        file: Option<String>,
        /// Build arguments.
        #[serde(default)]
        args: HashMap<String, String>,
    },
}

/// Service dependencies.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DependsOn {
    /// Simple list of service names.
    #[default]
    None,
    /// List of service names.
    Simple(Vec<String>),
    /// Full dependency configuration.
    Full(HashMap<String, DependencyCondition>),
}

/// Dependency condition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyCondition {
    /// Condition to wait for.
    pub condition: String,
}

/// Healthcheck specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthcheckSpec {
    /// Command to run.
    #[serde(default)]
    pub cmd: Vec<String>,
    /// HTTP healthcheck.
    #[serde(default)]
    pub http: Option<String>,
    /// Check interval.
    #[serde(default = "default_interval")]
    pub interval: String,
    /// Check timeout.
    #[serde(default = "default_timeout")]
    pub timeout: String,
    /// Retries before unhealthy.
    #[serde(default = "default_retries")]
    pub retries: u32,
    /// Start period.
    #[serde(default)]
    pub start_period: Option<String>,
}

fn default_interval() -> String {
    "30s".to_string()
}

fn default_timeout() -> String {
    "10s".to_string()
}

fn default_retries() -> u32 {
    3
}

/// Deploy configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeployConfig {
    /// Number of replicas.
    #[serde(default = "default_replicas")]
    pub replicas: u32,
    /// Resource limits.
    #[serde(default)]
    pub resources: Option<ResourceConfig>,
}

fn default_replicas() -> u32 {
    1
}

/// Resource configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResourceConfig {
    /// Memory limit.
    #[serde(default)]
    pub memory: Option<String>,
    /// CPU limit.
    #[serde(default)]
    pub cpu: Option<String>,
}

impl BockoseSpec {
    /// Parse from YAML.
    pub fn from_yaml(yaml: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(yaml)
    }

    /// Parse from file.
    pub fn from_file(path: &PathBuf) -> Result<Self, BockoseSpecError> {
        let content = std::fs::read_to_string(path).map_err(|e| BockoseSpecError::Io(e))?;
        let mut spec = Self::from_yaml(&content).map_err(|e| BockoseSpecError::Parse(e))?;
        spec.base_path = path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
        Ok(spec)
    }

    /// Get the stack name.
    pub fn stack_name(&self) -> String {
        self.name.clone().unwrap_or_else(|| "default".to_string())
    }
}

/// bockrose specification parsing errors.
#[derive(Debug, thiserror::Error)]
pub enum BockoseSpecError {
    /// I/O error.
    #[error("Failed to read bockrose.yaml: {0}")]
    Io(#[from] std::io::Error),
    /// Parse error.
    #[error("Failed to parse bockrose.yaml: {0}")]
    Parse(#[from] serde_yaml::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_basic_bockose() {
        let yaml = r#"
version: "1"
name: my-stack

services:
  web:
    image: nginx:alpine
    ports:
      - "80:80"

  api:
    build: ./api
    ports:
      - "3000:3000"
    depends_on:
      - db

  db:
    image: postgres:16
    environment:
      POSTGRES_PASSWORD: secret
    volumes:
      - db-data:/var/lib/postgresql/data

volumes:
  db-data:
"#;

        let spec = BockoseSpec::from_yaml(yaml).unwrap();
        assert_eq!(spec.stack_name(), "my-stack");
        assert_eq!(spec.services.len(), 3);
        assert!(spec.services.contains_key("web"));
        assert!(spec.volumes.contains_key("db-data"));
    }
}
