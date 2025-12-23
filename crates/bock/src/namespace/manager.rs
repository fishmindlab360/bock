#![allow(unsafe_code)]
//! Namespace manager.

use bock_common::BockResult;

use super::{IdMapping, NamespaceConfig};

/// Manages Linux namespaces for a container.
#[derive(Debug, Clone)]
pub struct NamespaceManager {
    config: NamespaceConfig,
    uid_mappings: Vec<IdMapping>,
    gid_mappings: Vec<IdMapping>,
}

impl NamespaceManager {
    /// Create a new namespace manager.
    pub fn new(config: NamespaceConfig) -> Self {
        Self {
            config,
            uid_mappings: Vec::new(),
            gid_mappings: Vec::new(),
        }
    }

    /// Add a UID mapping.
    pub fn add_uid_mapping(&mut self, mapping: IdMapping) {
        self.uid_mappings.push(mapping);
    }

    /// Add a GID mapping.
    pub fn add_gid_mapping(&mut self, mapping: IdMapping) {
        self.gid_mappings.push(mapping);
    }

    /// Enter namespaces by unsharing.
    #[cfg(target_os = "linux")]
    pub fn unshare(&self) -> BockResult<()> {
        let flags = self.config.to_unshare_flags();

        // Safety: We are creating new namespaces for container isolation.
        // This is the intended use case for unshare.
        unsafe {
            rustix::thread::unshare_unsafe(flags).map_err(|e| {
                bock_common::BockError::Internal {
                    message: format!("Failed to unshare namespaces: {}", e),
                }
            })?;
        }

        tracing::debug!(?flags, "Unshared namespaces");

        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    pub fn unshare(&self) -> BockResult<()> {
        Err(bock_common::BockError::Unsupported {
            feature: "namespaces".to_string(),
        })
    }

    /// Write UID mappings to /proc/[pid]/uid_map.
    #[cfg(target_os = "linux")]
    pub fn write_uid_map(&self, pid: u32) -> BockResult<()> {
        if self.uid_mappings.is_empty() {
            return Ok(());
        }

        let path = format!("/proc/{}/uid_map", pid);
        let content = self
            .uid_mappings
            .iter()
            .map(|m| format!("{} {} {}", m.container_id, m.host_id, m.size))
            .collect::<Vec<_>>()
            .join("\n");

        std::fs::write(&path, content).map_err(|e| bock_common::BockError::Internal {
            message: format!("Failed to write uid_map: {}", e),
        })?;

        tracing::debug!(path = %path, "Wrote UID mappings");

        Ok(())
    }

    /// Write GID mappings to /proc/[pid]/gid_map.
    #[cfg(target_os = "linux")]
    pub fn write_gid_map(&self, pid: u32) -> BockResult<()> {
        if self.gid_mappings.is_empty() {
            return Ok(());
        }

        // Deny setgroups first (required for unprivileged user namespaces)
        let setgroups_path = format!("/proc/{}/setgroups", pid);
        if std::path::Path::new(&setgroups_path).exists() {
            let _ = std::fs::write(&setgroups_path, "deny");
        }

        let path = format!("/proc/{}/gid_map", pid);
        let content = self
            .gid_mappings
            .iter()
            .map(|m| format!("{} {} {}", m.container_id, m.host_id, m.size))
            .collect::<Vec<_>>()
            .join("\n");

        std::fs::write(&path, content).map_err(|e| bock_common::BockError::Internal {
            message: format!("Failed to write gid_map: {}", e),
        })?;

        tracing::debug!(path = %path, "Wrote GID mappings");

        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    pub fn write_uid_map(&self, _pid: u32) -> BockResult<()> {
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    pub fn write_gid_map(&self, _pid: u32) -> BockResult<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn namespace_config_all() {
        let config = NamespaceConfig::all();
        assert!(config.user);
        assert!(config.pid);
        assert!(config.net);
        assert!(config.mount);
        assert!(config.uts);
        assert!(config.ipc);
        assert!(config.cgroup);
    }

    #[test]
    fn namespace_config_minimal() {
        let config = NamespaceConfig::minimal();
        assert!(!config.user);
        assert!(config.pid);
        assert!(!config.net);
        assert!(config.mount);
    }
}
