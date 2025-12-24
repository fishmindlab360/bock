//! Container security features.
//!
//! This module provides security hardening for containers:
//! - Seccomp syscall filtering
//! - Linux capabilities
//! - AppArmor profiles
//! - SELinux labels

mod apparmor;
mod capabilities;
mod seccomp;
mod selinux;

pub use apparmor::AppArmorProfile;
pub use capabilities::CapabilitySet;
pub use seccomp::SeccompFilter;
pub use selinux::SELinuxContext;

/// Security configuration for a container.
#[derive(Debug, Clone, Default)]
pub struct SecurityConfig {
    /// Seccomp filter.
    pub seccomp: Option<SeccompFilter>,
    /// Capabilities to keep.
    pub capabilities: CapabilitySet,
    /// no_new_privs flag.
    pub no_new_privileges: bool,
    /// AppArmor profile.
    pub apparmor_profile: Option<String>,
    /// SELinux label.
    pub selinux_label: Option<String>,
    /// Read-only root filesystem.
    pub readonly_rootfs: bool,
}

impl SecurityConfig {
    /// Create a minimal security configuration.
    #[must_use]
    pub fn minimal() -> Self {
        Self {
            seccomp: None,
            capabilities: CapabilitySet::default(),
            no_new_privileges: true,
            apparmor_profile: None,
            selinux_label: None,
            readonly_rootfs: false,
        }
    }

    /// Create a hardened security configuration.
    #[must_use]
    pub fn hardened() -> Self {
        Self {
            seccomp: Some(SeccompFilter::default_deny()),
            capabilities: CapabilitySet::minimal(),
            no_new_privileges: true,
            apparmor_profile: Some("bock-default".to_string()),
            selinux_label: None,
            readonly_rootfs: true,
        }
    }
}
