//! Linux namespace management.
//!
//! This module provides utilities for creating and managing Linux namespaces:
//! - User namespace (CLONE_NEWUSER)
//! - PID namespace (CLONE_NEWPID)
//! - Network namespace (CLONE_NEWNET)
//! - Mount namespace (CLONE_NEWNS)
//! - UTS namespace (CLONE_NEWUTS)
//! - IPC namespace (CLONE_NEWIPC)
//! - Cgroup namespace (CLONE_NEWCGROUP)

mod ipc;
mod manager;
mod mount;
mod net;
mod pid;
mod user;
mod uts;

pub use manager::NamespaceManager;

use bock_oci::runtime::NamespaceType;

/// Namespace configuration.
#[derive(Debug, Clone, Default)]
pub struct NamespaceConfig {
    /// User namespace settings.
    pub user: bool,
    /// PID namespace.
    pub pid: bool,
    /// Network namespace.
    pub net: bool,
    /// Mount namespace.
    pub mount: bool,
    /// UTS namespace.
    pub uts: bool,
    /// IPC namespace.
    pub ipc: bool,
    /// Cgroup namespace.
    pub cgroup: bool,
}

impl NamespaceConfig {
    /// Create a configuration with all namespaces enabled.
    #[must_use]
    pub fn all() -> Self {
        Self {
            user: true,
            pid: true,
            net: true,
            mount: true,
            uts: true,
            ipc: true,
            cgroup: true,
        }
    }

    /// Create a minimal configuration (just mount and pid).
    #[must_use]
    pub fn minimal() -> Self {
        Self {
            mount: true,
            pid: true,
            ..Default::default()
        }
    }

    /// Convert to rustix unshare flags.
    #[cfg(target_os = "linux")]
    pub fn to_unshare_flags(&self) -> rustix::thread::UnshareFlags {
        use rustix::thread::UnshareFlags;

        let mut flags = UnshareFlags::empty();

        if self.user {
            flags |= UnshareFlags::NEWUSER;
        }
        if self.pid {
            flags |= UnshareFlags::NEWPID;
        }
        if self.net {
            flags |= UnshareFlags::NEWNET;
        }
        if self.mount {
            flags |= UnshareFlags::NEWNS;
        }
        if self.uts {
            flags |= UnshareFlags::NEWUTS;
        }
        if self.ipc {
            flags |= UnshareFlags::NEWIPC;
        }
        if self.cgroup {
            flags |= UnshareFlags::NEWCGROUP;
        }

        flags
    }
    /// Create from OCI spec.
    pub fn from_spec(spec: &bock_oci::Spec) -> Self {
        let mut config = Self::default();

        if let Some(linux) = &spec.linux {
            for ns in &linux.namespaces {
                match ns.ns_type {
                    NamespaceType::User => config.user = true,
                    NamespaceType::Pid => config.pid = true,
                    NamespaceType::Network => config.net = true,
                    NamespaceType::Mount => config.mount = true,
                    NamespaceType::Uts => config.uts = true,
                    NamespaceType::Ipc => config.ipc = true,
                    NamespaceType::Cgroup => config.cgroup = true,
                    NamespaceType::Time => {} // Time namespace not yet supported in Bock config struct but defined in OCI
                }
            }
        }

        config
    }
}

/// UID/GID mapping for user namespaces.
#[derive(Debug, Clone)]
pub struct IdMapping {
    /// Container ID (start of range).
    pub container_id: u32,
    /// Host ID (start of range).
    pub host_id: u32,
    /// Size of the range.
    pub size: u32,
}

impl IdMapping {
    /// Create a simple 1:1 mapping for root.
    #[must_use]
    pub fn root_only() -> Self {
        Self {
            container_id: 0,
            host_id: 0,
            size: 1,
        }
    }

    /// Create a mapping for the current user.
    #[must_use]
    pub fn current_user() -> Self {
        #[cfg(target_os = "linux")]
        {
            use rustix::process::getuid;
            Self {
                container_id: 0,
                host_id: getuid().as_raw(),
                size: 1,
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            Self {
                container_id: 0,
                host_id: 1000,
                size: 1,
            }
        }
    }
}
