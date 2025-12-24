//! AppArmor profile management.
//!
//! This module provides utilities for loading and applying AppArmor profiles
//! to containers for mandatory access control.

use std::fs;
use std::path::Path;

use bock_common::BockResult;

/// AppArmor profile for containers.
#[derive(Debug, Clone)]
pub struct AppArmorProfile {
    /// Profile name.
    pub name: String,
    /// Profile content (if custom).
    pub content: Option<String>,
}

impl AppArmorProfile {
    /// Create a new AppArmor profile reference.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            content: None,
        }
    }

    /// Create a custom profile with content.
    pub fn custom(name: &str, content: &str) -> Self {
        Self {
            name: name.to_string(),
            content: Some(content.to_string()),
        }
    }

    /// Default container profile.
    pub fn container_default() -> Self {
        Self::new("bock-default")
    }

    /// Unconfined profile (no restrictions).
    pub fn unconfined() -> Self {
        Self::new("unconfined")
    }

    /// Check if AppArmor is enabled on the system.
    #[cfg(target_os = "linux")]
    pub fn is_enabled() -> bool {
        Path::new("/sys/module/apparmor").exists()
            && Path::new("/sys/kernel/security/apparmor").exists()
    }

    #[cfg(not(target_os = "linux"))]
    pub fn is_enabled() -> bool {
        false
    }

    /// Load a custom profile.
    #[cfg(target_os = "linux")]
    pub fn load(&self) -> BockResult<()> {
        if !Self::is_enabled() {
            tracing::warn!("AppArmor not enabled, skipping profile load");
            return Ok(());
        }

        if let Some(content) = &self.content {
            // Write profile to temp file and load with apparmor_parser
            let temp_path = format!("/tmp/apparmor-{}.profile", self.name);
            fs::write(&temp_path, content)?;

            let output = std::process::Command::new("apparmor_parser")
                .args(["-r", "-W", &temp_path])
                .output()
                .map_err(|e| bock_common::BockError::Internal {
                    message: format!("Failed to run apparmor_parser: {}", e),
                })?;

            fs::remove_file(&temp_path).ok();

            if !output.status.success() {
                return Err(bock_common::BockError::Internal {
                    message: format!(
                        "Failed to load AppArmor profile: {}",
                        String::from_utf8_lossy(&output.stderr)
                    ),
                });
            }

            tracing::info!(name = %self.name, "AppArmor profile loaded");
        }

        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    pub fn load(&self) -> BockResult<()> {
        Err(bock_common::BockError::Unsupported {
            feature: "AppArmor".to_string(),
        })
    }

    /// Apply the profile to the current process.
    #[cfg(target_os = "linux")]
    pub fn apply(&self) -> BockResult<()> {
        if !Self::is_enabled() {
            tracing::warn!("AppArmor not enabled, skipping profile apply");
            return Ok(());
        }

        if self.name == "unconfined" {
            tracing::debug!("Skipping unconfined AppArmor profile");
            return Ok(());
        }

        // Write to /proc/self/attr/apparmor/exec or /proc/self/attr/exec
        let exec_path = if Path::new("/proc/self/attr/apparmor/exec").exists() {
            "/proc/self/attr/apparmor/exec"
        } else {
            "/proc/self/attr/exec"
        };

        let profile_str = format!("exec {}", self.name);

        fs::write(exec_path, &profile_str).map_err(|e| {
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                bock_common::BockError::PermissionDenied {
                    operation: "apply AppArmor profile".to_string(),
                }
            } else {
                bock_common::BockError::Internal {
                    message: format!("Failed to apply AppArmor profile: {}", e),
                }
            }
        })?;

        tracing::info!(name = %self.name, "AppArmor profile applied");
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    pub fn apply(&self) -> BockResult<()> {
        Err(bock_common::BockError::Unsupported {
            feature: "AppArmor".to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_creation() {
        let profile = AppArmorProfile::new("test-profile");
        assert_eq!(profile.name, "test-profile");
        assert!(profile.content.is_none());
    }

    #[test]
    fn test_unconfined_profile() {
        let profile = AppArmorProfile::unconfined();
        assert_eq!(profile.name, "unconfined");
    }

    #[test]
    fn test_container_default_profile() {
        let profile = AppArmorProfile::container_default();
        assert_eq!(profile.name, "bock-default");
    }
}
