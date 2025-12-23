//! Standard filesystem paths for Bock.

use std::path::PathBuf;

use once_cell::sync::Lazy;

/// Default root directory for Bock data.
pub static BOCK_ROOT: Lazy<PathBuf> = Lazy::new(|| {
    std::env::var("BOCK_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/var/lib/bock"))
});

/// Default runtime directory for Bock.
pub static BOCK_RUNTIME_DIR: Lazy<PathBuf> = Lazy::new(|| {
    std::env::var("BOCK_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/run/bock"))
});

/// Standard paths used by the Bock runtime.
#[derive(Debug, Clone)]
pub struct BockPaths {
    /// Root data directory (default: /var/lib/bock).
    pub root: PathBuf,
    /// Runtime directory (default: /run/bock).
    pub runtime: PathBuf,
}

impl BockPaths {
    /// Create paths with default locations.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create paths with a custom root directory.
    #[must_use]
    pub fn with_root(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        let runtime = root.join("run");
        Self { root, runtime }
    }

    /// Directory for container data.
    #[must_use]
    pub fn containers(&self) -> PathBuf {
        self.root.join("containers")
    }

    /// Directory for a specific container.
    #[must_use]
    pub fn container(&self, id: &str) -> PathBuf {
        self.containers().join(id)
    }

    /// Container state file.
    #[must_use]
    pub fn container_state(&self, id: &str) -> PathBuf {
        self.container(id).join("state.json")
    }

    /// Container config file.
    #[must_use]
    pub fn container_config(&self, id: &str) -> PathBuf {
        self.container(id).join("config.json")
    }

    /// Container rootfs directory.
    #[must_use]
    pub fn container_rootfs(&self, id: &str) -> PathBuf {
        self.container(id).join("rootfs")
    }

    /// Container work directory (for overlay).
    #[must_use]
    pub fn container_work(&self, id: &str) -> PathBuf {
        self.container(id).join("work")
    }

    /// Container upper directory (for overlay).
    #[must_use]
    pub fn container_upper(&self, id: &str) -> PathBuf {
        self.container(id).join("upper")
    }

    /// Directory for images.
    #[must_use]
    pub fn images(&self) -> PathBuf {
        self.root.join("images")
    }

    /// Content-addressable storage for blobs.
    #[must_use]
    pub fn blobs(&self) -> PathBuf {
        self.root.join("blobs")
    }

    /// Blob file by digest.
    #[must_use]
    pub fn blob(&self, algorithm: &str, hash: &str) -> PathBuf {
        self.blobs().join(algorithm).join(hash)
    }

    /// Directory for layers (extracted).
    #[must_use]
    pub fn layers(&self) -> PathBuf {
        self.root.join("layers")
    }

    /// Layer directory by digest.
    #[must_use]
    pub fn layer(&self, digest: &str) -> PathBuf {
        // Replace : with / for filesystem compatibility
        let path = digest.replace(':', "/");
        self.layers().join(path)
    }

    /// Build cache directory.
    #[must_use]
    pub fn cache(&self) -> PathBuf {
        self.root.join("cache")
    }

    /// Networks directory.
    #[must_use]
    pub fn networks(&self) -> PathBuf {
        self.root.join("networks")
    }

    /// Volumes directory.
    #[must_use]
    pub fn volumes(&self) -> PathBuf {
        self.root.join("volumes")
    }

    /// PID file for a container.
    #[must_use]
    pub fn container_pid(&self, id: &str) -> PathBuf {
        self.runtime.join("containers").join(id).join("pid")
    }

    /// Socket file for container communication.
    #[must_use]
    pub fn container_socket(&self, id: &str) -> PathBuf {
        self.runtime.join("containers").join(id).join("shim.sock")
    }

    /// Create all necessary directories.
    ///
    /// # Errors
    ///
    /// Returns an error if directory creation fails.
    pub fn create_dirs(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.root)?;
        std::fs::create_dir_all(&self.runtime)?;
        std::fs::create_dir_all(self.containers())?;
        std::fs::create_dir_all(self.images())?;
        std::fs::create_dir_all(self.blobs())?;
        std::fs::create_dir_all(self.layers())?;
        std::fs::create_dir_all(self.cache())?;
        std::fs::create_dir_all(self.networks())?;
        std::fs::create_dir_all(self.volumes())?;
        Ok(())
    }
}

impl Default for BockPaths {
    fn default() -> Self {
        Self {
            root: BOCK_ROOT.clone(),
            runtime: BOCK_RUNTIME_DIR.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_paths() {
        let paths = BockPaths::new();
        assert_eq!(
            paths.containers(),
            PathBuf::from("/var/lib/bock/containers")
        );
        assert_eq!(
            paths.container("abc123"),
            PathBuf::from("/var/lib/bock/containers/abc123")
        );
    }

    #[test]
    fn custom_root() {
        let paths = BockPaths::with_root("/tmp/bock-test");
        assert_eq!(
            paths.containers(),
            PathBuf::from("/tmp/bock-test/containers")
        );
        assert_eq!(paths.runtime, PathBuf::from("/tmp/bock-test/run"));
    }

    #[test]
    fn blob_path() {
        let paths = BockPaths::new();
        assert_eq!(
            paths.blob("sha256", "abc123"),
            PathBuf::from("/var/lib/bock/blobs/sha256/abc123")
        );
    }

    #[test]
    fn layer_path() {
        let paths = BockPaths::new();
        assert_eq!(
            paths.layer("sha256:abc123"),
            PathBuf::from("/var/lib/bock/layers/sha256/abc123")
        );
    }
}
