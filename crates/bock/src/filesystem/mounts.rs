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
