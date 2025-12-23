//! Runtime configuration.

use std::path::PathBuf;

use bock_common::BockPaths;

/// Runtime configuration options.
#[derive(Debug, Clone)]
pub struct RuntimeConfig {
    /// Paths for runtime data.
    pub paths: BockPaths,
    /// Whether to use rootless mode.
    pub rootless: bool,
    /// Whether to use systemd cgroups.
    pub systemd_cgroup: bool,
    /// Default command timeout (seconds).
    pub timeout: u64,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            paths: BockPaths::new(),
            rootless: false,
            systemd_cgroup: false,
            timeout: 30,
        }
    }
}

impl RuntimeConfig {
    /// Create a rootless configuration.
    #[must_use]
    pub fn rootless() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        let root = home.join(".local/share/bock");

        Self {
            paths: BockPaths::with_root(root),
            rootless: true,
            systemd_cgroup: false,
            timeout: 30,
        }
    }

    /// Set the root directory.
    #[must_use]
    pub fn with_root(mut self, root: impl Into<PathBuf>) -> Self {
        self.paths = BockPaths::with_root(root);
        self
    }

    /// Enable systemd cgroups.
    #[must_use]
    pub fn with_systemd_cgroup(mut self) -> Self {
        self.systemd_cgroup = true;
        self
    }

    /// Set the default timeout.
    #[must_use]
    pub fn with_timeout(mut self, timeout: u64) -> Self {
        self.timeout = timeout;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config() {
        let config = RuntimeConfig::default();
        assert!(!config.rootless);
        assert!(!config.systemd_cgroup);
        assert_eq!(config.timeout, 30);
    }

    #[test]
    fn rootless_config() {
        let config = RuntimeConfig::rootless();
        assert!(config.rootless);
    }

    #[test]
    fn builder_pattern() {
        let config = RuntimeConfig::default()
            .with_root("/custom/root")
            .with_systemd_cgroup()
            .with_timeout(60);

        assert!(config.systemd_cgroup);
        assert_eq!(config.timeout, 60);
    }
}
