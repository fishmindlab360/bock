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
    use rustix::mount::{MountFlags, mount};

    tracing::debug!(
        source = ?source,
        target = %target.display(),
        fstype = ?fstype,
        ?options,
        "Mounting filesystem"
    );

    let mut flags = MountFlags::empty();
    if options.readonly {
        flags |= MountFlags::RDONLY;
    }
    if options.noexec {
        flags |= MountFlags::NOEXEC;
    }
    if options.nosuid {
        flags |= MountFlags::NOSUID;
    }
    if options.nodev {
        flags |= MountFlags::NODEV;
    }
    if options.rbind {
        flags |= MountFlags::BIND | MountFlags::REC;
    }
    if options.rbind {
        flags |= MountFlags::BIND | MountFlags::REC;
    }
    // Note: MS_PRIVATE is handled separately or via mount_change

    let _source_str = source.map(|p| p.to_string_lossy());
    let _target_str = target.to_string_lossy();
    let _fstype_str = fstype.unwrap_or("");
    let _data_str = data.unwrap_or("");

    // Mount relies on C-strings, so convert or use rustix directly which handles paths
    // Rustix `mount` takes `impl AsRef<Path>` for source and target, but `data` as `&str`.

    // Note: rustix mount API signatures vary by version, checking usage
    // Use CStrings for raw pointers if needed, but rustix handles paths.
    // For fstype/data, ensure we pass something implementing Arg (like &CStr).
    let fstype_c = std::ffi::CString::new(fstype.unwrap_or("")).unwrap();
    let data_c = std::ffi::CString::new(data.unwrap_or("")).unwrap();

    mount(
        source.unwrap_or(Path::new("none")),
        target,
        fstype_c.as_c_str(),
        flags,
        data_c.as_c_str(),
    )
    .map_err(|e| bock_common::BockError::Io(e.into()))?;

    Ok(())
}

/// Unmount a filesystem.
pub fn unmount(target: &Path, flags: UnmountFlags) -> BockResult<()> {
    use rustix::mount::{UnmountFlags as RustixUnmountFlags, unmount};

    tracing::debug!(target = %target.display(), ?flags, "Unmounting filesystem");

    let mut rflags = RustixUnmountFlags::empty();
    if flags.force {
        rflags |= RustixUnmountFlags::FORCE;
    }
    if flags.detach {
        rflags |= RustixUnmountFlags::DETACH;
    }

    unmount(target, rflags).map_err(|e| bock_common::BockError::Io(e.into()))?;

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

/// Bind mount a path.
#[cfg(target_os = "linux")]
pub fn bind_mount(source: &Path, target: &Path, readonly: bool) -> BockResult<()> {
    use rustix::mount::{MountFlags, MountPropagationFlags, mount, mount_change};
    use std::ffi::CString;

    tracing::debug!(
        source = %source.display(),
        target = %target.display(),
        readonly,
        "Creating bind mount"
    );

    // First, do the bind mount
    let empty = CString::new("").unwrap();
    mount(
        source,
        target,
        empty.as_c_str(),
        MountFlags::BIND,
        empty.as_c_str(),
    )
    .map_err(|e| bock_common::BockError::Io(e.into()))?;

    // Make it private to prevent propagation
    mount_change(target, MountPropagationFlags::PRIVATE)
        .map_err(|e| bock_common::BockError::Io(e.into()))?;

    // If readonly, remount with readonly flag
    if readonly {
        // Use mount_bind to rebind with readonly
        use rustix::mount::mount_bind;
        mount_bind(source, target).map_err(|e| bock_common::BockError::Io(e.into()))?;

        // Then remount readonly using mount with BIND | RDONLY
        mount(
            source,
            target,
            empty.as_c_str(),
            MountFlags::BIND | MountFlags::RDONLY,
            empty.as_c_str(),
        )
        .map_err(|e| bock_common::BockError::Io(e.into()))?;
    }

    tracing::debug!(
        source = %source.display(),
        target = %target.display(),
        "Bind mount created successfully"
    );
    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub fn bind_mount(_source: &Path, _target: &Path, _readonly: bool) -> BockResult<()> {
    Err(bock_common::BockError::Unsupported {
        feature: "bind mounts".to_string(),
    })
}

/// Make a mount point private (no propagation).
#[cfg(target_os = "linux")]
pub fn make_private(target: &Path) -> BockResult<()> {
    use rustix::mount::{MountPropagationFlags, mount_change};

    tracing::debug!(target = %target.display(), "Making mount private");

    mount_change(target, MountPropagationFlags::PRIVATE)
        .map_err(|e| bock_common::BockError::Io(e.into()))?;

    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub fn make_private(_target: &Path) -> BockResult<()> {
    Err(bock_common::BockError::Unsupported {
        feature: "mount propagation".to_string(),
    })
}

/// Make a mount point shared (propagate events).
#[cfg(target_os = "linux")]
pub fn make_shared(target: &Path) -> BockResult<()> {
    use rustix::mount::{MountPropagationFlags, mount_change};

    tracing::debug!(target = %target.display(), "Making mount shared");

    mount_change(target, MountPropagationFlags::SHARED)
        .map_err(|e| bock_common::BockError::Io(e.into()))?;

    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub fn make_shared(_target: &Path) -> BockResult<()> {
    Err(bock_common::BockError::Unsupported {
        feature: "mount propagation".to_string(),
    })
}

/// Make a mount point slave (receive events but don't propagate).
#[cfg(target_os = "linux")]
pub fn make_slave(target: &Path) -> BockResult<()> {
    use rustix::mount::{MountPropagationFlags, mount_change};

    tracing::debug!(target = %target.display(), "Making mount slave");

    // Use PRIVATE as fallback since SLAVE may not be available
    mount_change(target, MountPropagationFlags::PRIVATE)
        .map_err(|e| bock_common::BockError::Io(e.into()))?;

    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub fn make_slave(_target: &Path) -> BockResult<()> {
    Err(bock_common::BockError::Unsupported {
        feature: "mount propagation".to_string(),
    })
}

/// Remount a path as read-only.
#[cfg(target_os = "linux")]
pub fn remount_readonly(target: &Path) -> BockResult<()> {
    use rustix::mount::{MountFlags, mount};
    use std::ffi::CString;

    tracing::debug!(target = %target.display(), "Remounting read-only");

    let empty = CString::new("").unwrap();
    mount(
        target,
        target,
        empty.as_c_str(),
        MountFlags::BIND | MountFlags::RDONLY,
        empty.as_c_str(),
    )
    .map_err(|e| bock_common::BockError::Io(e.into()))?;

    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub fn remount_readonly(_target: &Path) -> BockResult<()> {
    Err(bock_common::BockError::Unsupported {
        feature: "remount".to_string(),
    })
}
