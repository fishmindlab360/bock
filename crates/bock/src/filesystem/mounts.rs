//! Mount operations.

use std::path::Path;

use bock_common::BockResult;

/// Mount options.
#[derive(Debug, Clone, Default)]
pub struct MountOptions {
    /// Read-only mount.
    pub readonly: bool,
    /// No exec.
    pub noexec: bool,
    /// No suid.
    pub nosuid: bool,
    /// No dev.
    pub nodev: bool,
    /// Recursive bind mount.
    pub rbind: bool,
    /// Private mount propagation.
    pub private: bool,
}

impl MountOptions {
    /// Create options for a typical container mount.
    #[must_use]
    pub fn container_default() -> Self {
        Self {
            readonly: false,
            noexec: false,
            nosuid: true,
            nodev: true,
            rbind: false,
            private: false,
        }
    }

    /// Create options for /proc mount.
    #[must_use]
    pub fn proc() -> Self {
        Self {
            readonly: false,
            noexec: true,
            nosuid: true,
            nodev: true,
            rbind: false,
            private: false,
        }
    }

    /// Create options for /sys mount.
    #[must_use]
    pub fn sysfs() -> Self {
        Self {
            readonly: true,
            noexec: true,
            nosuid: true,
            nodev: true,
            rbind: false,
            private: false,
        }
    }
}

/// Mount a filesystem.
pub fn mount(
    source: Option<&Path>,
    target: &Path,
    fstype: Option<&str>,
    options: &MountOptions,
    data: Option<&str>,
) -> BockResult<()> {
    tracing::debug!(
        source = ?source,
        target = %target.display(),
        fstype = ?fstype,
        "Mounting filesystem"
    );

    // TODO: Implement using rustix::mount

    Ok(())
}

/// Unmount a filesystem.
pub fn unmount(target: &Path, flags: UnmountFlags) -> BockResult<()> {
    tracing::debug!(target = %target.display(), ?flags, "Unmounting filesystem");

    // TODO: Implement using rustix::mount

    Ok(())
}

/// Unmount flags.
#[derive(Debug, Clone, Copy, Default)]
pub struct UnmountFlags {
    /// Force unmount.
    pub force: bool,
    /// Lazy unmount (detach).
    pub detach: bool,
}
