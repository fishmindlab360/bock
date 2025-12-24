//! Root filesystem setup.

use std::path::Path;

use bock_common::BockResult;

/// Default directories to create in the rootfs.
const DEFAULT_DIRS: &[&str] = &["dev", "proc", "sys", "tmp", "etc", "var", "run"];

/// Setup the container root filesystem.
pub fn setup_rootfs(rootfs: &Path) -> BockResult<()> {
    tracing::debug!(rootfs = %rootfs.display(), "Setting up root filesystem");

    // Create essential directories
    for dir in DEFAULT_DIRS {
        let path = rootfs.join(dir);
        if !path.exists() {
            std::fs::create_dir_all(&path)?;
        }
    }

    // Setup /dev
    setup_dev(rootfs)?;

    // Setup /etc/resolv.conf if not exists
    setup_etc(rootfs)?;

    Ok(())
}

/// Setup /dev with essential device nodes.
fn setup_dev(rootfs: &Path) -> BockResult<()> {
    let dev = rootfs.join("dev");

    // Create essential symlinks
    let links = [
        ("fd", "/proc/self/fd"),
        ("stdin", "/proc/self/fd/0"),
        ("stdout", "/proc/self/fd/1"),
        ("stderr", "/proc/self/fd/2"),
    ];

    for (name, target) in links {
        let path = dev.join(name);
        if !path.exists() {
            let _ = std::os::unix::fs::symlink(target, &path);
        }
    }

    // Create pts and shm directories
    let _ = std::fs::create_dir_all(dev.join("pts"));
    let _ = std::fs::create_dir_all(dev.join("shm"));

    // Create essential device nodes (requires root privileges)
    #[cfg(target_os = "linux")]
    create_device_nodes(&dev)?;

    Ok(())
}

/// Standard device nodes to create in containers.
/// Format: (name, type, major, minor, mode)
/// type: 'c' = character device, 'b' = block device
#[cfg(target_os = "linux")]
const DEVICE_NODES: &[(&str, char, u32, u32, u32)] = &[
    ("null", 'c', 1, 3, 0o666),
    ("zero", 'c', 1, 5, 0o666),
    ("full", 'c', 1, 7, 0o666),
    ("random", 'c', 1, 8, 0o666),
    ("urandom", 'c', 1, 9, 0o666),
    ("tty", 'c', 5, 0, 0o666),
    ("console", 'c', 5, 1, 0o620),
    ("ptmx", 'c', 5, 2, 0o666),
];

/// Create device nodes in /dev.
#[cfg(target_os = "linux")]
fn create_device_nodes(dev: &Path) -> BockResult<()> {
    use rustix::fs::{FileType, Mode, mknodat};

    for (name, dev_type, major, minor, mode) in DEVICE_NODES {
        let path = dev.join(name);
        if path.exists() {
            continue;
        }

        let file_type = match dev_type {
            'c' => FileType::CharacterDevice,
            'b' => FileType::BlockDevice,
            _ => continue,
        };

        let dev_num = rustix::fs::makedev(*major, *minor);
        let mode_bits = Mode::from_raw_mode(*mode);

        tracing::debug!(device = %name, major, minor, "Creating device node");

        // Create device node - requires CAP_MKNOD
        match mknodat(rustix::fs::CWD, &path, file_type, mode_bits, dev_num) {
            Ok(()) => {}
            Err(e) => {
                // Permission denied is expected in rootless mode
                if e.kind() != std::io::ErrorKind::PermissionDenied {
                    tracing::warn!(device = %name, error = %e, "Failed to create device node");
                }
            }
        }
    }

    Ok(())
}

/// Mount tmpfs on container directories.
#[cfg(target_os = "linux")]
pub fn mount_tmpfs(target: &Path, size: Option<&str>) -> BockResult<()> {
    use rustix::mount::{MountFlags, mount};
    use std::ffi::CString;

    let options = match size {
        Some(s) => format!("mode=1777,size={}", s),
        None => "mode=1777".to_string(),
    };

    let fstype = CString::new("tmpfs").unwrap();
    let options_c = CString::new(options.as_str()).unwrap();

    tracing::debug!(target = %target.display(), options = %options, "Mounting tmpfs");

    mount(
        "tmpfs",
        target,
        fstype.as_c_str(),
        MountFlags::NOSUID | MountFlags::NODEV,
        options_c.as_c_str(),
    )
    .map_err(|e| bock_common::BockError::Io(e.into()))?;

    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub fn mount_tmpfs(_target: &Path, _size: Option<&str>) -> BockResult<()> {
    Err(bock_common::BockError::Unsupported {
        feature: "tmpfs".to_string(),
    })
}

/// Setup /etc.
fn setup_etc(rootfs: &Path) -> BockResult<()> {
    let etc = rootfs.join("etc");
    std::fs::create_dir_all(&etc)?;

    // Create empty resolv.conf if not exists
    let resolv_conf = etc.join("resolv.conf");
    if !resolv_conf.exists() {
        std::fs::write(
            &resolv_conf,
            "# Container resolv.conf\nnameserver 8.8.8.8\n",
        )?;
    }

    // Create /etc/hostname
    let hostname = etc.join("hostname");
    if !hostname.exists() {
        std::fs::write(&hostname, "container\n")?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn setup_rootfs_creates_dirs() {
        let temp = tempdir().unwrap();
        setup_rootfs(temp.path()).unwrap();

        assert!(temp.path().join("dev").exists());
        assert!(temp.path().join("proc").exists());
        assert!(temp.path().join("sys").exists());
        assert!(temp.path().join("tmp").exists());
    }
}
