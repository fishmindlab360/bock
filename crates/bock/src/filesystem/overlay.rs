//! OverlayFS setup for container rootfs.

use std::path::{Path, PathBuf};

use bock_common::BockResult;

/// OverlayFS configuration.
#[derive(Debug, Clone)]
pub struct OverlayFs {
    /// Lower directories (read-only layers).
    pub lower_dirs: Vec<PathBuf>,
    /// Upper directory (writable layer).
    pub upper_dir: PathBuf,
    /// Work directory (required by overlayfs).
    pub work_dir: PathBuf,
    /// Merged mount point.
    pub merged_dir: PathBuf,
}

impl OverlayFs {
    /// Create a new OverlayFS configuration.
    pub fn new(
        lower_dirs: Vec<PathBuf>,
        upper_dir: PathBuf,
        work_dir: PathBuf,
        merged_dir: PathBuf,
    ) -> Self {
        Self {
            lower_dirs,
            upper_dir,
            work_dir,
            merged_dir,
        }
    }

    /// Setup for a container.
    pub fn for_container(container_dir: &Path, layer_dirs: Vec<PathBuf>) -> Self {
        Self {
            lower_dirs: layer_dirs,
            upper_dir: container_dir.join("upper"),
            work_dir: container_dir.join("work"),
            merged_dir: container_dir.join("rootfs"),
        }
    }

    /// Create necessary directories.
    pub fn create_dirs(&self) -> BockResult<()> {
        std::fs::create_dir_all(&self.upper_dir)?;
        std::fs::create_dir_all(&self.work_dir)?;
        std::fs::create_dir_all(&self.merged_dir)?;
        Ok(())
    }

    /// Mount the overlay filesystem.
    #[cfg(target_os = "linux")]
    pub fn mount(&self) -> BockResult<()> {
        use rustix::mount::{MountFlags, mount};
        use std::ffi::CString;

        self.create_dirs()?;

        let lower = self
            .lower_dirs
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(":");

        let options = format!(
            "lowerdir={},upperdir={},workdir={}",
            lower,
            self.upper_dir.display(),
            self.work_dir.display()
        );

        tracing::debug!(
            merged = %self.merged_dir.display(),
            options = %options,
            "Mounting overlayfs"
        );

        let fstype = CString::new("overlay").unwrap();
        let options_c =
            CString::new(options.as_str()).map_err(|_| bock_common::BockError::Config {
                message: "Invalid overlay options (contains null byte)".to_string(),
            })?;

        // Mount using rustix
        mount(
            "overlay",            // source
            &self.merged_dir,     // target
            fstype.as_c_str(),    // filesystem type
            MountFlags::empty(),  // flags
            options_c.as_c_str(), // data/options
        )
        .map_err(|e| bock_common::BockError::Io(e.into()))?;

        tracing::info!(merged = %self.merged_dir.display(), "OverlayFS mounted successfully");
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    pub fn mount(&self) -> BockResult<()> {
        Err(bock_common::BockError::Unsupported {
            feature: "overlayfs".to_string(),
        })
    }

    /// Unmount the overlay filesystem.
    #[cfg(target_os = "linux")]
    pub fn unmount(&self) -> BockResult<()> {
        use rustix::mount::{UnmountFlags, unmount};

        tracing::debug!(merged = %self.merged_dir.display(), "Unmounting overlayfs");

        unmount(&self.merged_dir, UnmountFlags::DETACH)
            .map_err(|e| bock_common::BockError::Io(e.into()))?;

        tracing::info!(merged = %self.merged_dir.display(), "OverlayFS unmounted successfully");
        Ok(())
    }

    #[cfg(not(target_os = "linux"))]
    pub fn unmount(&self) -> BockResult<()> {
        Err(bock_common::BockError::Unsupported {
            feature: "overlayfs".to_string(),
        })
    }

    /// Get the mount options string.
    #[must_use]
    pub fn mount_options(&self) -> String {
        let lower = self
            .lower_dirs
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>()
            .join(":");

        format!(
            "lowerdir={},upperdir={},workdir={}",
            lower,
            self.upper_dir.display(),
            self.work_dir.display()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn overlay_mount_options() {
        let overlay = OverlayFs::new(
            vec![PathBuf::from("/layer1"), PathBuf::from("/layer2")],
            PathBuf::from("/upper"),
            PathBuf::from("/work"),
            PathBuf::from("/merged"),
        );

        let options = overlay.mount_options();
        assert!(options.contains("/layer1:/layer2"));
        assert!(options.contains("upperdir=/upper"));
        assert!(options.contains("workdir=/work"));
    }
}
