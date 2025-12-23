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

    Ok(())
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
