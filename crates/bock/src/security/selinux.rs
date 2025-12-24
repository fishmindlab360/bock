//! SELinux label management.
//!
//! This module provides utilities for setting SELinux security contexts
//! on container processes and files.

use std::fs;
use std::path::Path;

use bock_common::BockResult;

/// SELinux context for containers.
#[derive(Debug, Clone)]
pub struct SELinuxContext {
    /// User component.
    pub user: String,
    /// Role component.
    pub role: String,
    /// Type component.
    pub type_: String,
    /// Level component (optional).
    pub level: Option<String>,
}

impl SELinuxContext {
    /// Create a new SELinux context.
    pub fn new(user: &str, role: &str, type_: &str, level: Option<&str>) -> Self {
        Self {
            user: user.to_string(),
            role: role.to_string(),
            type_: type_.to_string(),
            level: level.map(String::from),
        }
    }

    /// Parse a context string.
    pub fn parse(context: &str) -> BockResult<Self> {
        let parts: Vec<&str> = context.split(':').collect();

        if parts.len() < 3 {
            return Err(bock_common::BockError::Config {
                message: format!("Invalid SELinux context: {}", context),
            });
        }

        Ok(Self {
            user: parts[0].to_string(),
            role: parts[1].to_string(),
            type_: parts[2].to_string(),
            level: parts.get(3).map(|s| s.to_string()),
        })
    }

    /// Format as a context string.
    pub fn to_string(&self) -> String {
        match &self.level {
            Some(level) => format!("{}:{}:{}:{}", self.user, self.role, self.type_, level),
            None => format!("{}:{}:{}", self.user, self.role, self.type_),
        }
    }

    /// Default container context.
    pub fn container_default() -> Self {
        Self::new("system_u", "system_r", "container_t", Some("s0"))
    }

    /// Unconfined context.
    pub fn unconfined() -> Self {
        Self::new("unconfined_u", "unconfined_r", "unconfined_t", Some("s0"))
    }

    /// Check if SELinux is enabled.
    #[cfg(target_os = "linux")]
    pub fn is_enabled() -> bool {
        Path::new("/sys/fs/selinux").exists()
    }

    #[cfg(not(target_os = "linux"))]
    pub fn is_enabled() -> bool {
        false
    }

    /// Get current SELinux enforcement mode.
    #[cfg(target_os = "linux")]
    pub fn enforcement_mode() -> Option<String> {
        fs::read_to_string("/sys/fs/selinux/enforce").ok().map(|s| {
            if s.trim() == "1" {
                "enforcing".to_string()
            } else {
                "permissive".to_string()
            }
        })
    }

    #[cfg(not(target_os = "linux"))]
    pub fn enforcement_mode() -> Option<String> {
        None
    }

    /// Apply the context to the current process.
    #[cfg(target_os = "linux")]
    pub fn apply(&self) -> BockResult<()> {
        if !Self::is_enabled() {
            tracing::warn!("SELinux not enabled, skipping context apply");
            return Ok(());
        }

        let context_str = self.to_string();

        // Write to /proc/self/attr/exec for next exec
        fs::write("/proc/self/attr/exec", &context_str).map_err(|e| {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                bock_common::BockError::PermissionDenied {
                    operation: "apply SELinux context".to_string(),
                }
            } else {
                bock_common::BockError::Internal {
                    message: format!("Failed to set SELinux context: {}", e),
                }
            }
        })?;

        tracing::info!(context = %context_str, "SELinux context applied");
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    pub fn apply(&self) -> BockResult<()> {
        Err(bock_common::BockError::Unsupported {
            feature: "SELinux".to_string(),
        })
    }

    /// Set file context.
    #[cfg(target_os = "linux")]
    pub fn set_file_context(path: &Path, context: &str) -> BockResult<()> {
        use std::process::Command;

        if !Self::is_enabled() {
            return Ok(());
        }

        let output = Command::new("chcon")
            .args([context, path.to_str().unwrap_or("")])
            .output()
            .map_err(|e| bock_common::BockError::Internal {
                message: format!("Failed to run chcon: {}", e),
            })?;

        if !output.status.success() {
            return Err(bock_common::BockError::Internal {
                message: format!("chcon failed: {}", String::from_utf8_lossy(&output.stderr)),
            });
        }

        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    pub fn set_file_context(_path: &Path, _context: &str) -> BockResult<()> {
        Err(bock_common::BockError::Unsupported {
            feature: "SELinux".to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_creation() {
        let ctx = SELinuxContext::new("user_u", "user_r", "user_t", Some("s0"));
        assert_eq!(ctx.user, "user_u");
        assert_eq!(ctx.role, "user_r");
        assert_eq!(ctx.type_, "user_t");
        assert_eq!(ctx.level, Some("s0".to_string()));
    }

    #[test]
    fn test_context_parse() {
        let ctx = SELinuxContext::parse("system_u:system_r:container_t:s0").unwrap();
        assert_eq!(ctx.user, "system_u");
        assert_eq!(ctx.role, "system_r");
        assert_eq!(ctx.type_, "container_t");
        assert_eq!(ctx.level, Some("s0".to_string()));
    }

    #[test]
    fn test_context_to_string() {
        let ctx = SELinuxContext::container_default();
        assert_eq!(ctx.to_string(), "system_u:system_r:container_t:s0");
    }
}
