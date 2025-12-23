//! Linux capabilities management.

use bock_common::{BockError, BockResult};

/// Linux capability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Capability {
    /// CAP_CHOWN
    Chown,
    /// CAP_DAC_OVERRIDE
    DacOverride,
    /// CAP_DAC_READ_SEARCH
    DacReadSearch,
    /// CAP_FOWNER
    Fowner,
    /// CAP_FSETID
    Fsetid,
    /// CAP_KILL
    Kill,
    /// CAP_SETGID
    Setgid,
    /// CAP_SETUID
    Setuid,
    /// CAP_SETPCAP
    Setpcap,
    /// CAP_LINUX_IMMUTABLE
    LinuxImmutable,
    /// CAP_NET_BIND_SERVICE
    NetBindService,
    /// CAP_NET_BROADCAST
    NetBroadcast,
    /// CAP_NET_ADMIN
    NetAdmin,
    /// CAP_NET_RAW
    NetRaw,
    /// CAP_IPC_LOCK
    IpcLock,
    /// CAP_IPC_OWNER
    IpcOwner,
    /// CAP_SYS_MODULE
    SysModule,
    /// CAP_SYS_RAWIO
    SysRawio,
    /// CAP_SYS_CHROOT
    SysChroot,
    /// CAP_SYS_PTRACE
    SysPtrace,
    /// CAP_SYS_PACCT
    SysPacct,
    /// CAP_SYS_ADMIN
    SysAdmin,
    /// CAP_SYS_BOOT
    SysBoot,
    /// CAP_SYS_NICE
    SysNice,
    /// CAP_SYS_RESOURCE
    SysResource,
    /// CAP_SYS_TIME
    SysTime,
    /// CAP_SYS_TTY_CONFIG
    SysTtyConfig,
    /// CAP_MKNOD
    Mknod,
    /// CAP_LEASE
    Lease,
    /// CAP_AUDIT_WRITE
    AuditWrite,
    /// CAP_AUDIT_CONTROL
    AuditControl,
    /// CAP_SETFCAP
    Setfcap,
}

impl Capability {
    /// Get the capability name as a string.
    #[must_use]
    pub fn name(&self) -> &'static str {
        match self {
            Self::Chown => "CAP_CHOWN",
            Self::DacOverride => "CAP_DAC_OVERRIDE",
            Self::DacReadSearch => "CAP_DAC_READ_SEARCH",
            Self::Fowner => "CAP_FOWNER",
            Self::Fsetid => "CAP_FSETID",
            Self::Kill => "CAP_KILL",
            Self::Setgid => "CAP_SETGID",
            Self::Setuid => "CAP_SETUID",
            Self::Setpcap => "CAP_SETPCAP",
            Self::LinuxImmutable => "CAP_LINUX_IMMUTABLE",
            Self::NetBindService => "CAP_NET_BIND_SERVICE",
            Self::NetBroadcast => "CAP_NET_BROADCAST",
            Self::NetAdmin => "CAP_NET_ADMIN",
            Self::NetRaw => "CAP_NET_RAW",
            Self::IpcLock => "CAP_IPC_LOCK",
            Self::IpcOwner => "CAP_IPC_OWNER",
            Self::SysModule => "CAP_SYS_MODULE",
            Self::SysRawio => "CAP_SYS_RAWIO",
            Self::SysChroot => "CAP_SYS_CHROOT",
            Self::SysPtrace => "CAP_SYS_PTRACE",
            Self::SysPacct => "CAP_SYS_PACCT",
            Self::SysAdmin => "CAP_SYS_ADMIN",
            Self::SysBoot => "CAP_SYS_BOOT",
            Self::SysNice => "CAP_SYS_NICE",
            Self::SysResource => "CAP_SYS_RESOURCE",
            Self::SysTime => "CAP_SYS_TIME",
            Self::SysTtyConfig => "CAP_SYS_TTY_CONFIG",
            Self::Mknod => "CAP_MKNOD",
            Self::Lease => "CAP_LEASE",
            Self::AuditWrite => "CAP_AUDIT_WRITE",
            Self::AuditControl => "CAP_AUDIT_CONTROL",
            Self::Setfcap => "CAP_SETFCAP",
        }
    }
}

/// Set of capabilities.
#[derive(Debug, Clone, Default)]
pub struct CapabilitySet {
    /// Capabilities to add.
    pub add: Vec<Capability>,
    /// Capabilities to drop.
    pub drop: Vec<Capability>,
}

impl CapabilitySet {
    /// Create a minimal capability set (container defaults).
    #[must_use]
    pub fn minimal() -> Self {
        Self {
            add: vec![
                Capability::Chown,
                Capability::DacOverride,
                Capability::Fsetid,
                Capability::Fowner,
                Capability::Mknod,
                Capability::NetRaw,
                Capability::Setgid,
                Capability::Setuid,
                Capability::Setfcap,
                Capability::Setpcap,
                Capability::NetBindService,
                Capability::SysChroot,
                Capability::Kill,
                Capability::AuditWrite,
            ],
            drop: Vec::new(),
        }
    }

    /// Create an empty capability set (drop all).
    #[must_use]
    pub fn empty() -> Self {
        Self {
            add: Vec::new(),
            drop: Vec::new(),
        }
    }

    /// Apply the capability set.
    pub fn apply(&self) -> BockResult<()> {
        tracing::debug!("Applying capability set");
        
        // Convert internal Capability to caps::Capability
        let caps_to_add: caps::CapsHashSet = self
            .add
            .iter()
            .map(|c| c.to_caps_capability())
            .collect();

        // Check if we need to drop any specific logic, but typically we set the exact set allowable
        
        // Drop all capabilities from bounding set (requires privilege)
        // Then add specifically allowed ones?
        // Actually, caps::set sets the whole set.

        // Bounding set (limit for future execs)
        // Only works if we have CAP_SETPCAP
        if let Err(e) = caps::set(None, caps::CapSet::Bounding, &caps_to_add) {
             tracing::warn!("Failed to set bounding caps (might lack permission): {}", e);
        }

        // Inheritable
        caps::set(None, caps::CapSet::Inheritable, &caps_to_add).map_err(|e| BockError::Internal {
            message: format!("Failed to set inheritable caps: {}", e),
        })?;

        // Effective
        caps::set(None, caps::CapSet::Effective, &caps_to_add).map_err(|e| BockError::Internal {
            message: format!("Failed to set effective caps: {}", e),
        })?;

        // Permitted
        caps::set(None, caps::CapSet::Permitted, &caps_to_add).map_err(|e| BockError::Internal {
            message: format!("Failed to set permitted caps: {}", e),
        })?;

        // Ambient (if supported)
        if let Err(e) = caps::set(None, caps::CapSet::Ambient, &caps_to_add) {
            tracing::warn!("Failed to set ambient caps: {}", e);
        }

        Ok(())
    }
}

impl Capability {
    fn to_caps_capability(&self) -> caps::Capability {
        match self {
            Self::Chown => caps::Capability::CAP_CHOWN,
            Self::DacOverride => caps::Capability::CAP_DAC_OVERRIDE,
            Self::DacReadSearch => caps::Capability::CAP_DAC_READ_SEARCH,
            Self::Fowner => caps::Capability::CAP_FOWNER,
            Self::Fsetid => caps::Capability::CAP_FSETID,
            Self::Kill => caps::Capability::CAP_KILL,
            Self::Setgid => caps::Capability::CAP_SETGID,
            Self::Setuid => caps::Capability::CAP_SETUID,
            Self::Setpcap => caps::Capability::CAP_SETPCAP,
            Self::LinuxImmutable => caps::Capability::CAP_LINUX_IMMUTABLE,
            Self::NetBindService => caps::Capability::CAP_NET_BIND_SERVICE,
            Self::NetBroadcast => caps::Capability::CAP_NET_BROADCAST,
            Self::NetAdmin => caps::Capability::CAP_NET_ADMIN,
            Self::NetRaw => caps::Capability::CAP_NET_RAW,
            Self::IpcLock => caps::Capability::CAP_IPC_LOCK,
            Self::IpcOwner => caps::Capability::CAP_IPC_OWNER,
            Self::SysModule => caps::Capability::CAP_SYS_MODULE,
            Self::SysRawio => caps::Capability::CAP_SYS_RAWIO,
            Self::SysChroot => caps::Capability::CAP_SYS_CHROOT,
            Self::SysPtrace => caps::Capability::CAP_SYS_PTRACE,
            Self::SysPacct => caps::Capability::CAP_SYS_PACCT,
            Self::SysAdmin => caps::Capability::CAP_SYS_ADMIN,
            Self::SysBoot => caps::Capability::CAP_SYS_BOOT,
            Self::SysNice => caps::Capability::CAP_SYS_NICE,
            Self::SysResource => caps::Capability::CAP_SYS_RESOURCE,
            Self::SysTime => caps::Capability::CAP_SYS_TIME,
            Self::SysTtyConfig => caps::Capability::CAP_SYS_TTY_CONFIG,
            Self::Mknod => caps::Capability::CAP_MKNOD,
            Self::Lease => caps::Capability::CAP_LEASE,
            Self::AuditWrite => caps::Capability::CAP_AUDIT_WRITE,
            Self::AuditControl => caps::Capability::CAP_AUDIT_CONTROL,
            Self::Setfcap => caps::Capability::CAP_SETFCAP,
        }
    }
}