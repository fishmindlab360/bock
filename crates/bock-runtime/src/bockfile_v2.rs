//! Bockfile v2 - Modern container image specification.
//!
//! A clean, intuitive format for defining container images with:
//! - Multi-format support (YAML, TOML, JSON)
//! - Environment variable interpolation
//! - Dynamic tag templates
//! - Per-stage security configuration
//! - Registry integration
#![allow(unsafe_code)]

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use bock_common::BockResult;
use serde::{Deserialize, Serialize};

/// Bockfile v2 - the complete container image specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bockfile {
    /// Base image configuration.
    pub base: BaseImage,

    /// Build arguments with optional env fallbacks.
    #[serde(default)]
    pub args: HashMap<String, ArgValue>,

    /// Image metadata.
    #[serde(default)]
    pub metadata: Metadata,

    /// Build stages.
    #[serde(default)]
    pub stages: Vec<Stage>,

    /// Runtime configuration.
    #[serde(default)]
    pub runtime: RuntimeConfig,

    /// Global security configuration (can be overridden per-stage).
    #[serde(default)]
    pub security: SecurityConfig,

    /// Registry configuration.
    #[serde(default)]
    pub registry: Option<RegistryConfig>,
}

/// Base image configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseImage {
    /// Image reference (e.g., "alpine:3.19", "ubuntu:22.04").
    pub from: String,

    /// Alias for referencing in multi-stage builds.
    #[serde(default)]
    pub alias: Option<String>,

    /// Version override (supports env: prefix for environment variables).
    /// If set, overrides the tag in `from`.
    #[serde(default)]
    pub version: Option<String>,

    /// Platform (e.g., "linux/amd64", "linux/arm64").
    #[serde(default)]
    pub platform: Option<String>,
}

/// Argument value with optional environment fallback.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ArgValue {
    /// Simple string value.
    Simple(String),

    /// Value with environment fallback.
    WithFallback {
        /// Default value.
        default: Option<String>,
        /// Environment variable to read from.
        #[serde(rename = "env")]
        env_var: Option<String>,
        /// Description for documentation.
        description: Option<String>,
    },
}

impl ArgValue {
    /// Resolve the argument value, checking environment if needed.
    pub fn resolve(&self) -> String {
        match self {
            ArgValue::Simple(s) => resolve_env_refs(s),
            ArgValue::WithFallback {
                default, env_var, ..
            } => {
                // First try env var
                if let Some(var) = env_var {
                    if let Ok(val) = std::env::var(var) {
                        return val;
                    }
                }
                // Fall back to default
                default
                    .clone()
                    .map(|d| resolve_env_refs(&d))
                    .unwrap_or_default()
            }
        }
    }
}

/// Image metadata.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Metadata {
    /// Image name.
    #[serde(default)]
    pub name: Option<String>,

    /// Version (semver recommended).
    #[serde(default)]
    pub version: Option<String>,

    /// Tag template (supports placeholders like {{version}}, {{git.sha}}).
    #[serde(default)]
    pub tag: Option<String>,

    /// Description.
    #[serde(default)]
    pub description: Option<String>,

    /// Authors.
    #[serde(default)]
    pub authors: Vec<String>,

    /// License.
    #[serde(default)]
    pub license: Option<String>,

    /// Custom labels.
    #[serde(default)]
    pub labels: HashMap<String, String>,
}

impl Metadata {
    /// Build the full image tag.
    pub fn build_tag(&self) -> Option<String> {
        if let Some(template) = &self.tag {
            Some(self.interpolate_tag(template))
        } else if let (Some(name), Some(version)) = (&self.name, &self.version) {
            Some(format!("{}:{}", name, version))
        } else {
            self.name.clone()
        }
    }

    fn interpolate_tag(&self, template: &str) -> String {
        let mut result = template.to_string();

        if let Some(name) = &self.name {
            result = result.replace("{{name}}", name);
        }
        if let Some(version) = &self.version {
            result = result.replace("{{version}}", version);
        }

        // Git placeholders
        if result.contains("{{git.sha}}") {
            let sha = get_git_sha().unwrap_or_else(|| "unknown".to_string());
            result = result.replace("{{git.sha}}", &sha);
        }
        if result.contains("{{git.sha_short}}") {
            let sha = get_git_sha()
                .map(|s| s[..7.min(s.len())].to_string())
                .unwrap_or_else(|| "unknown".to_string());
            result = result.replace("{{git.sha_short}}", &sha);
        }
        if result.contains("{{git.branch}}") {
            let branch = get_git_branch().unwrap_or_else(|| "unknown".to_string());
            result = result.replace("{{git.branch}}", &branch);
        }

        // Timestamp
        if result.contains("{{timestamp}}") {
            let ts = chrono::Utc::now().format("%Y%m%d%H%M%S").to_string();
            result = result.replace("{{timestamp}}", &ts);
        }

        result
    }
}

/// Build stage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Stage {
    /// Stage name (used for --target and --from references).
    pub name: String,

    /// Optional alias (alternative name for referencing).
    #[serde(default)]
    pub alias: Option<String>,

    /// Base image override for this stage (for multi-stage builds).
    #[serde(default)]
    pub from: Option<String>,

    /// Stage dependencies (built before this stage).
    #[serde(default)]
    pub depends: Vec<String>,

    /// Build steps.
    #[serde(default)]
    pub steps: Vec<Step>,

    /// Stage-specific security overrides.
    #[serde(default)]
    pub security: Option<SecurityConfig>,

    /// Cache configuration for this stage.
    #[serde(default)]
    pub cache: Option<CacheConfig>,

    /// Working directory for this stage.
    #[serde(default)]
    pub workdir: Option<String>,

    /// Environment variables for this stage.
    #[serde(default)]
    pub env: HashMap<String, String>,
}

/// Build step.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Step {
    /// Run a command.
    Run(RunStep),

    /// Copy files.
    Copy(CopyStep),

    /// Add files (with URL/archive support).
    Add(AddStep),

    /// Set working directory.
    Workdir(String),

    /// Set environment variable.
    Env(EnvStep),

    /// Set user.
    User(String),

    /// Set entrypoint.
    Entrypoint(Vec<String>),

    /// Set default command.
    Cmd(Vec<String>),

    /// Expose port.
    Expose(ExposeStep),

    /// Set volume.
    Volume(String),

    /// Set label.
    Label(HashMap<String, String>),

    /// Set shell.
    Shell(Vec<String>),

    /// Healthcheck.
    Healthcheck(HealthcheckStep),
}

/// Run step configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RunStep {
    /// Simple command string.
    Simple(String),

    /// Detailed run configuration.
    Detailed {
        /// Command to run.
        run: String,
        /// Working directory.
        #[serde(default)]
        workdir: Option<String>,
        /// Run as user.
        #[serde(default)]
        user: Option<String>,
        /// Cache mounts.
        #[serde(default)]
        cache: Vec<CacheMount>,
        /// Bind mounts.
        #[serde(default)]
        bind: Vec<BindMount>,
        /// Network mode.
        #[serde(default)]
        network: Option<String>,
        /// Security options.
        #[serde(default)]
        security: Option<String>,
    },
}

/// Copy step configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CopyStep {
    /// Simple copy (source to destination).
    Simple {
        /// Source path.
        from: String,
        /// Destination path.
        to: String,
    },

    /// Detailed copy configuration.
    Detailed {
        /// Source files/directories.
        copy: Vec<String>,
        /// Destination path.
        to: String,
        /// Copy from another stage.
        #[serde(default, rename = "from_stage")]
        from_stage: Option<String>,
        /// Owner (user:group).
        #[serde(default)]
        chown: Option<String>,
        /// Permissions.
        #[serde(default)]
        chmod: Option<String>,
        /// Exclude patterns.
        #[serde(default)]
        exclude: Vec<String>,
    },
}

/// Add step (like COPY but with URL/archive support).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddStep {
    /// Source (can be URL or local path).
    pub add: String,
    /// Destination.
    pub to: String,
    /// Checksum verification.
    #[serde(default)]
    pub checksum: Option<String>,
    /// Extract archives automatically.
    #[serde(default = "default_true")]
    pub extract: bool,
}

/// Environment variable step.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum EnvStep {
    /// Single env var.
    Single {
        /// Variable name.
        key: String,
        /// Variable value.
        value: String,
    },
    /// Multiple env vars.
    Multiple(HashMap<String, String>),
}

/// Expose port step.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ExposeStep {
    /// Simple port number.
    Port(u16),
    /// Port with protocol.
    Detailed {
        /// Port number.
        port: u16,
        /// Protocol (tcp/udp).
        protocol: Option<String>,
    },
}

/// Healthcheck configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthcheckStep {
    /// Command to run.
    pub cmd: Vec<String>,
    /// Interval between checks.
    #[serde(default)]
    pub interval: Option<String>,
    /// Timeout for check.
    #[serde(default)]
    pub timeout: Option<String>,
    /// Start period.
    #[serde(default)]
    pub start_period: Option<String>,
    /// Number of retries.
    #[serde(default)]
    pub retries: Option<u32>,
}

/// Cache mount configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheMount {
    /// Mount target path.
    pub target: String,
    /// Cache ID.
    #[serde(default)]
    pub id: Option<String>,
    /// Sharing mode (shared, private, locked).
    #[serde(default)]
    pub sharing: Option<String>,
}

/// Bind mount configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BindMount {
    /// Source path.
    pub source: String,
    /// Target path.
    pub target: String,
    /// Read-only.
    #[serde(default)]
    pub readonly: bool,
}

/// Cache configuration for a stage.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Enable/disable caching for this stage.
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Cache key.
    #[serde(default)]
    pub key: Option<String>,
}

/// Runtime configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RuntimeConfig {
    /// Entrypoint.
    #[serde(default)]
    pub entrypoint: Option<Vec<String>>,

    /// Default command.
    #[serde(default)]
    pub cmd: Option<Vec<String>>,

    /// Working directory.
    #[serde(default)]
    pub workdir: Option<String>,

    /// Environment variables.
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// User to run as.
    #[serde(default)]
    pub user: Option<String>,

    /// Exposed ports.
    #[serde(default)]
    pub ports: Vec<String>,

    /// Volumes.
    #[serde(default)]
    pub volumes: Vec<String>,

    /// Stop signal.
    #[serde(default)]
    pub stop_signal: Option<String>,

    /// Stop timeout.
    #[serde(default)]
    pub stop_timeout: Option<u32>,
}

/// Security configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SecurityConfig {
    /// User to run as.
    #[serde(default)]
    pub user: Option<String>,

    /// Capabilities to add.
    #[serde(default)]
    pub capabilities_add: Vec<String>,

    /// Capabilities to drop.
    #[serde(default)]
    pub capabilities_drop: Vec<String>,

    /// Enable no_new_privs.
    #[serde(default)]
    pub no_new_privs: Option<bool>,

    /// Seccomp profile.
    #[serde(default)]
    pub seccomp: Option<String>,

    /// AppArmor profile.
    #[serde(default)]
    pub apparmor: Option<String>,

    /// SELinux context.
    #[serde(default)]
    pub selinux: Option<String>,

    /// Read-only rootfs.
    #[serde(default)]
    pub readonly_rootfs: Option<bool>,
}

/// Registry configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryConfig {
    /// Registry URL or name.
    pub name: String,

    /// Push on successful build.
    #[serde(default)]
    pub push_on_build: bool,

    /// Additional tags to push.
    #[serde(default)]
    pub additional_tags: Vec<String>,

    /// Credentials reference (from credential store).
    #[serde(default)]
    pub credentials: Option<String>,
}

fn default_true() -> bool {
    true
}

// ============================================================================
// Parsing
// ============================================================================

impl Bockfile {
    /// Parse from any supported format (auto-detected by extension).
    pub fn from_file(path: &Path) -> BockResult<Self> {
        let content = fs::read_to_string(path)?;

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("yaml");

        match ext.to_lowercase().as_str() {
            "yaml" | "yml" => Self::from_yaml(&content),
            "toml" => Self::from_toml(&content),
            "json" => Self::from_json(&content),
            _ => Self::from_yaml(&content), // Default to YAML
        }
    }

    /// Parse from YAML.
    pub fn from_yaml(content: &str) -> BockResult<Self> {
        let value: serde_json::Value =
            serde_yaml::from_str(content).map_err(|e| bock_common::BockError::Config {
                message: format!("Failed to parse YAML structure: {}", e),
            })?;

        serde_json::from_value(value).map_err(|e| bock_common::BockError::Config {
            message: format!("Failed to interpret YAML as Bockfile: {}", e),
        })
    }

    /// Parse from TOML.
    pub fn from_toml(content: &str) -> BockResult<Self> {
        toml::from_str(content).map_err(|e| bock_common::BockError::Config {
            message: format!("Failed to parse TOML: {}", e),
        })
    }

    /// Parse from JSON.
    pub fn from_json(content: &str) -> BockResult<Self> {
        serde_json::from_str(content).map_err(|e| bock_common::BockError::Config {
            message: format!("Failed to parse JSON: {}", e),
        })
    }

    /// Serialize to YAML.
    pub fn to_yaml(&self) -> BockResult<String> {
        serde_yaml::to_string(self).map_err(|e| bock_common::BockError::Internal {
            message: format!("Failed to serialize to YAML: {}", e),
        })
    }

    /// Serialize to TOML.
    pub fn to_toml(&self) -> BockResult<String> {
        toml::to_string_pretty(self).map_err(|e| bock_common::BockError::Internal {
            message: format!("Failed to serialize to TOML: {}", e),
        })
    }

    /// Serialize to JSON.
    pub fn to_json(&self) -> BockResult<String> {
        serde_json::to_string_pretty(self).map_err(|e| bock_common::BockError::Internal {
            message: format!("Failed to serialize to JSON: {}", e),
        })
    }

    /// Resolve the base image with version overrides.
    pub fn resolve_base_image(&self) -> String {
        let mut image = self.base.from.clone();

        // Apply version override
        if let Some(version) = &self.base.version {
            let resolved_version = resolve_env_refs(version);

            // Replace or append version
            if let Some(idx) = image.rfind(':') {
                image = format!("{}:{}", &image[..idx], resolved_version);
            } else {
                image = format!("{}:{}", image, resolved_version);
            }
        }

        image
    }

    /// Resolve all args with environment fallbacks.
    pub fn resolve_args(&self) -> HashMap<String, String> {
        self.args
            .iter()
            .map(|(k, v)| (k.clone(), v.resolve()))
            .collect()
    }

    /// Get the final image tag.
    pub fn get_tag(&self) -> Option<String> {
        self.metadata.build_tag()
    }

    /// Find a stage by name or alias.
    pub fn find_stage(&self, name: &str) -> Option<&Stage> {
        self.stages
            .iter()
            .find(|s| s.name == name || s.alias.as_deref() == Some(name))
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// Resolve environment variable references in a string.
/// Supports formats: `env.VAR`, `${VAR}`, `$VAR`
fn resolve_env_refs(s: &str) -> String {
    let mut result = s.to_string();

    // Handle env.VAR format
    let env_prefix = "env.";
    if let Some(start) = result.find(env_prefix) {
        let rest = &result[start + env_prefix.len()..];
        let end = rest
            .find(|c: char| !c.is_alphanumeric() && c != '_')
            .unwrap_or(rest.len());
        let var_name = &rest[..end];

        if let Ok(value) = std::env::var(var_name) {
            result = result.replace(&format!("{}{}", env_prefix, var_name), &value);
        }
    }

    // Handle ${VAR} format
    while let Some(start) = result.find("${") {
        if let Some(end) = result[start..].find('}') {
            let var_name = &result[start + 2..start + end];
            let replacement = std::env::var(var_name).unwrap_or_default();
            result = format!(
                "{}{}{}",
                &result[..start],
                replacement,
                &result[start + end + 1..]
            );
        } else {
            break;
        }
    }

    result
}

/// Get current git SHA.
fn get_git_sha() -> Option<String> {
    std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

/// Get current git branch.
fn get_git_branch() -> Option<String> {
    std::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_YAML: &str = r#"
base:
  from: alpine:3.19
  alias: builder
  version: env.ALPINE_VERSION

args:
  APP_VERSION: "1.0.0"
  DEBUG:
    default: "false"
    env: DEBUG_MODE

metadata:
  name: my-app
  version: "1.0.0"
  tag: "{{name}}:{{version}}-{{git.sha_short}}"
  description: My awesome app

stages:
  - name: build
    steps:
      - run: apk add --no-cache build-base
      - copy:
          copy:
            - src/
          to: /app/src

  - name: runtime
    depends:
      - build
    steps:
      - copy:
          copy:
            - --from=build
            - /app/bin/myapp
          to: /usr/local/bin/
      - user: nobody
    security:
      user: nobody
      capabilities_drop:
        - ALL

runtime:
  entrypoint: ["/usr/local/bin/myapp"]
  cmd: ["--help"]
  env:
    APP_ENV: production

security:
  no_new_privs: true
  capabilities_drop:
    - ALL

registry:
  name: ghcr.io/myorg
  push_on_build: true
"#;

    #[test]
    fn test_parse_yaml() {
        let bockfile = Bockfile::from_yaml(SAMPLE_YAML).unwrap();

        assert_eq!(bockfile.base.from, "alpine:3.19");
        assert_eq!(bockfile.base.alias, Some("builder".to_string()));
        assert_eq!(bockfile.metadata.name, Some("my-app".to_string()));
        assert_eq!(bockfile.stages.len(), 2);
        assert_eq!(bockfile.stages[0].name, "build");
        assert_eq!(bockfile.stages[1].name, "runtime");
        assert!(bockfile.registry.is_some());
    }

    #[test]
    fn test_resolve_env_refs() {
        unsafe { std::env::set_var("TEST_VAR", "test_value") };

        let result = resolve_env_refs("prefix-env.TEST_VAR-suffix");
        assert!(result.contains("test_value"));

        let result = resolve_env_refs("prefix-${TEST_VAR}-suffix");
        assert_eq!(result, "prefix-test_value-suffix");

        unsafe { std::env::remove_var("TEST_VAR") };
    }
}
