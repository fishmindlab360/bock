//! Bockfile parsing and schema.

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Bockfile specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bockfile {
    /// Specification version.
    #[serde(default = "default_version")]
    pub version: String,

    /// Base image.
    pub from: String,

    /// Image metadata.
    #[serde(default)]
    pub metadata: Metadata,

    /// Build arguments.
    #[serde(default)]
    pub args: HashMap<String, String>,

    /// Build stages.
    #[serde(default)]
    pub stages: Vec<Stage>,

    /// Runtime configuration.
    #[serde(default)]
    pub runtime: RuntimeConfig,

    /// Security settings.
    #[serde(default)]
    pub security: SecurityConfig,
}

fn default_version() -> String {
    "1".to_string()
}

/// Image metadata.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Metadata {
    /// Image name.
    #[serde(default)]
    pub name: Option<String>,
    /// Image version.
    #[serde(default)]
    pub version: Option<String>,
    /// Authors.
    #[serde(default)]
    pub authors: Vec<String>,
    /// Labels.
    #[serde(default)]
    pub labels: HashMap<String, String>,
}

/// Build stage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stage {
    /// Stage name.
    pub name: String,
    /// Base image for this stage (overrides global from).
    #[serde(default)]
    pub from: Option<String>,
    /// Dependencies on other stages.
    #[serde(default)]
    pub depends: Vec<String>,
    /// Build steps.
    #[serde(default)]
    pub steps: Vec<Step>,
}

/// Build step.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Step {
    /// Run a command.
    Run(RunStep),
    /// Copy files.
    Copy(CopyStep),
    /// Set user.
    User(UserStep),
    /// Set working directory.
    Workdir(WorkdirStep),
    /// Set environment variable.
    Env(EnvStep),
}

/// Run command step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunStep {
    /// Command to run.
    pub run: String,
    /// Working directory.
    #[serde(default)]
    pub workdir: Option<String>,
    /// Environment variables.
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Cache mounts.
    #[serde(default)]
    pub cache: Vec<CacheMount>,
}

/// Copy files step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopyStep {
    /// Files to copy (source paths or patterns).
    pub copy: Vec<String>,
    /// Destination path.
    pub to: String,
    /// Source stage (for multi-stage copies).
    #[serde(default)]
    pub from: Option<String>,
}

/// User step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserStep {
    /// User (uid:gid format).
    pub user: String,
}

/// Workdir step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkdirStep {
    /// Working directory.
    pub workdir: String,
}

/// Env step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvStep {
    /// Environment variable name.
    pub env: String,
    /// Environment variable value.
    pub value: String,
}

/// Cache mount for RUN commands.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheMount {
    /// Path to cache.
    pub path: String,
    /// Cache key (with variable substitution).
    #[serde(default)]
    pub key: Option<String>,
}

/// Runtime configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RuntimeConfig {
    /// Entrypoint.
    #[serde(default)]
    pub entrypoint: Vec<String>,
    /// Default command.
    #[serde(default)]
    pub cmd: Vec<String>,
    /// Working directory.
    #[serde(default)]
    pub workdir: Option<String>,
    /// Environment variables.
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Exposed ports.
    #[serde(default)]
    pub expose: Vec<String>,
    /// Healthcheck.
    #[serde(default)]
    pub healthcheck: Option<Healthcheck>,
}

/// Healthcheck configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Healthcheck {
    /// Command to run.
    pub cmd: Vec<String>,
    /// Interval between checks.
    #[serde(default = "default_interval")]
    pub interval: String,
    /// Timeout for each check.
    #[serde(default = "default_timeout")]
    pub timeout: String,
    /// Number of retries.
    #[serde(default = "default_retries")]
    pub retries: u32,
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

/// Security configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// User to run as.
    #[serde(default)]
    pub user: Option<String>,
    /// Read-only root filesystem.
    #[serde(default)]
    pub read_only_rootfs: bool,
    /// No new privileges.
    #[serde(default = "default_no_new_privs")]
    pub no_new_privileges: bool,
    /// Capabilities.
    #[serde(default)]
    pub capabilities: CapabilityConfig,
}

fn default_no_new_privs() -> bool {
    true
}

/// Capability configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CapabilityConfig {
    /// Capabilities to drop.
    #[serde(default)]
    pub drop: Vec<String>,
    /// Capabilities to add.
    #[serde(default)]
    pub add: Vec<String>,
}

impl Bockfile {
    /// Parse a Bockfile from YAML.
    pub fn from_yaml(yaml: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(yaml)
    }

    /// Parse a Bockfile from a file.
    pub fn from_file(path: &PathBuf) -> Result<Self, BockfileError> {
        let content = std::fs::read_to_string(path).map_err(|e| BockfileError::Io(e))?;
        Self::from_yaml(&content).map_err(|e| BockfileError::Parse(e))
    }
}

/// Bockfile parsing errors.
#[derive(Debug, thiserror::Error)]
pub enum BockfileError {
    /// I/O error.
    #[error("Failed to read Bockfile: {0}")]
    Io(#[from] std::io::Error),
    /// Parse error.
    #[error("Failed to parse Bockfile: {0}")]
    Parse(#[from] serde_yaml::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_basic_bockfile() {
        let yaml = r#"
version: "1"
from: alpine:3.19

metadata:
  name: my-app
  version: 1.0.0

stages:
  - name: build
    steps:
      - run: apk add --no-cache nodejs npm
      - copy: ["package.json"]
        to: /app/
      - run: npm install
        workdir: /app

runtime:
  entrypoint: ["node"]
  cmd: ["/app/index.js"]
  workdir: /app
"#;

        let bockfile = Bockfile::from_yaml(yaml).unwrap();
        assert_eq!(bockfile.from, "alpine:3.19");
        assert_eq!(bockfile.metadata.name.as_deref(), Some("my-app"));
        assert_eq!(bockfile.stages.len(), 1);
        assert_eq!(bockfile.stages[0].name, "build");
    }
}
