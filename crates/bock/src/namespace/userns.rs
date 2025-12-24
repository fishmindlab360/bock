//! User namespace UID/GID mapping improvements.
//!
//! This module provides enhanced UID/GID mapping for user namespaces,
//! supporting rootless containers and improved isolation.

use std::fs;
use std::path::Path;

use bock_common::BockResult;

/// UID/GID mapping entry.
#[derive(Debug, Clone)]
pub struct IdMap {
    /// Container ID (start).
    pub container_id: u32,
    /// Host ID (start).
    pub host_id: u32,
    /// Range size.
    pub size: u32,
}

impl IdMap {
    /// Create a new ID mapping.
    pub fn new(container_id: u32, host_id: u32, size: u32) -> Self {
        Self {
            container_id,
            host_id,
            size,
        }
    }

    /// Create identity mapping (1:1).
    pub fn identity() -> Self {
        Self::new(0, 0, u32::MAX)
    }

    /// Create rootless mapping for current user.
    pub fn rootless() -> BockResult<Self> {
        let uid = unsafe { libc::getuid() };
        Ok(Self::new(0, uid, 1))
    }

    /// Format for /proc/<pid>/uid_map or gid_map.
    pub fn to_proc_format(&self) -> String {
        format!("{} {} {}", self.container_id, self.host_id, self.size)
    }
}

/// User namespace configuration.
#[derive(Debug, Clone, Default)]
pub struct UserNamespaceConfig {
    /// UID mappings.
    pub uid_mappings: Vec<IdMap>,
    /// GID mappings.
    pub gid_mappings: Vec<IdMap>,
}

impl UserNamespaceConfig {
    /// Create default (root) configuration.
    pub fn root() -> Self {
        Self {
            uid_mappings: vec![IdMap::new(0, 0, 1)],
            gid_mappings: vec![IdMap::new(0, 0, 1)],
        }
    }

    /// Create rootless configuration.
    pub fn rootless() -> BockResult<Self> {
        let uid = unsafe { libc::getuid() };
        let gid = unsafe { libc::getgid() };

        // Read subordinate UID/GID mappings
        let subuid = Self::read_subid("/etc/subuid", uid)?;
        let subgid = Self::read_subid("/etc/subgid", gid)?;

        let mut uid_mappings = vec![IdMap::new(0, uid, 1)];
        let mut gid_mappings = vec![IdMap::new(0, gid, 1)];

        if let Some((start, count)) = subuid {
            uid_mappings.push(IdMap::new(1, start, count));
        }

        if let Some((start, count)) = subgid {
            gid_mappings.push(IdMap::new(1, start, count));
        }

        Ok(Self {
            uid_mappings,
            gid_mappings,
        })
    }

    /// Read subordinate ID range from /etc/subuid or /etc/subgid.
    fn read_subid(path: &str, id: u32) -> BockResult<Option<(u32, u32)>> {
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => return Ok(None),
        };

        let id_str = id.to_string();
        let username = Self::get_username(id)?;

        for line in content.lines() {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() >= 3 {
                // Match by UID or username
                if parts[0] == id_str || parts[0] == username {
                    if let (Ok(start), Ok(count)) = (parts[1].parse(), parts[2].parse()) {
                        return Ok(Some((start, count)));
                    }
                }
            }
        }

        Ok(None)
    }

    /// Get username for a UID.
    fn get_username(uid: u32) -> BockResult<String> {
        let content = fs::read_to_string("/etc/passwd").unwrap_or_default();

        for line in content.lines() {
            let parts: Vec<&str> = line.split(':').collect();
            if parts.len() >= 3 {
                if let Ok(line_uid) = parts[2].parse::<u32>() {
                    if line_uid == uid {
                        return Ok(parts[0].to_string());
                    }
                }
            }
        }

        Ok(uid.to_string())
    }

    /// Apply UID mappings to a process.
    pub fn apply_uid_map(&self, pid: u32) -> BockResult<()> {
        let path = format!("/proc/{}/uid_map", pid);
        let content: String = self
            .uid_mappings
            .iter()
            .map(|m| m.to_proc_format())
            .collect::<Vec<_>>()
            .join("\n");

        fs::write(&path, content).map_err(|e| bock_common::BockError::Internal {
            message: format!("Failed to write uid_map: {}", e),
        })?;

        tracing::debug!(pid, "UID mappings applied");
        Ok(())
    }

    /// Apply GID mappings to a process.
    pub fn apply_gid_map(&self, pid: u32) -> BockResult<()> {
        // First, disable setgroups
        let setgroups_path = format!("/proc/{}/setgroups", pid);
        fs::write(&setgroups_path, "deny").ok();

        let path = format!("/proc/{}/gid_map", pid);
        let content: String = self
            .gid_mappings
            .iter()
            .map(|m| m.to_proc_format())
            .collect::<Vec<_>>()
            .join("\n");

        fs::write(&path, content).map_err(|e| bock_common::BockError::Internal {
            message: format!("Failed to write gid_map: {}", e),
        })?;

        tracing::debug!(pid, "GID mappings applied");
        Ok(())
    }
}

/// Check if user namespaces are available.
pub fn user_ns_available() -> bool {
    Path::new("/proc/self/ns/user").exists()
}

/// Check if running as root.
pub fn is_root() -> bool {
    unsafe { libc::getuid() == 0 }
}

/// Check if rootless mode is possible.
pub fn rootless_available() -> bool {
    user_ns_available() && !is_root()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_id_map_format() {
        let map = IdMap::new(0, 1000, 1);
        assert_eq!(map.to_proc_format(), "0 1000 1");
    }

    #[test]
    fn test_user_ns_config_root() {
        let config = UserNamespaceConfig::root();
        assert_eq!(config.uid_mappings.len(), 1);
        assert_eq!(config.gid_mappings.len(), 1);
    }

    #[test]
    fn test_user_ns_available() {
        // Just check the function works
        let _ = user_ns_available();
    }
}
