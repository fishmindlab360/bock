#![allow(unsafe_code)]
//! Container type and operations.

use std::path::PathBuf;
use std::sync::Arc;

use bock_common::{BockResult, ContainerId};
use bock_oci::state::ContainerStatus;
use bock_oci::{ContainerState, Spec};
use parking_lot::RwLock;
use tokio::sync::Mutex;

use crate::cgroup::CgroupManager;
use crate::namespace::NamespaceManager;
use bock_network::VethPair;

use super::config::RuntimeConfig;
use super::state::StateManager;

/// A container instance.
#[derive(Debug)]
pub struct Container {
    /// Container ID.
    id: ContainerId,
    /// OCI specification.
    spec: Spec,
    /// Runtime configuration.
    config: RuntimeConfig,
    /// Container state.
    state: Arc<RwLock<ContainerState>>,
    /// Cgroup manager.
    cgroup: Option<CgroupManager>,
    /// Namespace manager.
    namespace: Option<NamespaceManager>,
    /// Process ID of the container init process.
    pid: Arc<Mutex<Option<u32>>>,
    /// Bundle path.
    bundle: PathBuf,
}

impl Container {
    /// Create a new container.
    ///
    /// This sets up the container environment but does not start the process.
    pub async fn create(
        id: impl Into<String>,
        bundle: impl Into<PathBuf>,
        spec: &Spec,
        config: RuntimeConfig,
    ) -> BockResult<Self> {
        let id = ContainerId::new(id)?;
        let bundle = bundle.into();

        // Ensure container directory exists
        let container_dir = config.paths.container(id.as_str());
        if !container_dir.exists() {
            std::fs::create_dir_all(&container_dir)?;
        }

        let state = ContainerState::new(id.as_str(), &bundle);

        // Save initial state to disk so it can be loaded later
        let state_manager = StateManager::new(config.paths.containers());
        state_manager.save(&state)?;

        tracing::info!(
            container_id = %id,
            bundle = %bundle.display(),
            "Creating container"
        );

        let rootfs = bundle.join("rootfs");
        if !rootfs.exists() {
            return Err(bock_common::BockError::Config {
                message: format!("Rootfs not found at {}", rootfs.display()),
            });
        }

        // Setup rootfs
        crate::filesystem::setup_rootfs(&rootfs)?;

        // Cgroups
        let cgroup = match CgroupManager::new(id.as_str()) {
            Ok(c) => Some(c),
            Err(bock_common::BockError::PermissionDenied { .. }) => {
                tracing::warn!(
                    "Failed to create cgroup (permission denied), continuing without cgroups"
                );
                None
            }
            Err(e) => return Err(e),
        };

        Ok(Self {
            id,
            spec: spec.clone(),
            config,
            state: Arc::new(RwLock::new(state)),
            cgroup,
            namespace: Some(NamespaceManager::new(
                crate::namespace::NamespaceConfig::from_spec(spec),
            )),
            pid: Arc::new(Mutex::new(None)),
            bundle,
        })
    }

    /// Load a container from state.
    pub async fn load(id: &str, config: RuntimeConfig) -> BockResult<Self> {
        let state_manager = StateManager::new(config.paths.containers());
        let state = state_manager.load(id)?;

        // Load bundle spec
        let bundle = PathBuf::from(&state.bundle);
        let config_path = bundle.join("config.json");
        if !config_path.exists() {
            return Err(bock_common::BockError::Config {
                message: format!("Config not found at {}", config_path.display()),
            });
        }

        let spec_json = std::fs::read_to_string(&config_path)?;
        let spec: Spec = serde_json::from_str(&spec_json)?;

        let id = ContainerId::new(state.id.clone())?;

        Ok(Self {
            id,
            spec: spec.clone(),
            config,
            state: Arc::new(RwLock::new(state)),
            cgroup: None,
            namespace: Some(NamespaceManager::new(
                crate::namespace::NamespaceConfig::from_spec(&spec),
            )),
            pid: Arc::new(Mutex::new(None)),
            bundle,
        })
    }

    /// Save container state.
    fn save_state(&self) -> BockResult<()> {
        let state = self.state.read();
        let state_manager = StateManager::new(self.config.paths.containers());
        state_manager.save(&state)
    }

    /// ID accessor.
    #[must_use]
    pub fn id(&self) -> &ContainerId {
        &self.id
    }

    /// Get the container state.
    #[must_use]
    pub fn state(&self) -> ContainerState {
        self.state.read().clone()
    }

    /// Get the container status.
    #[must_use]
    pub fn status(&self) -> ContainerStatus {
        self.state.read().status
    }

    /// Get the container PID.
    pub async fn pid(&self) -> Option<u32> {
        // Reload PID from somewhere? currently just memory.
        *self.pid.lock().await
    }

    /// Start the container.
    pub async fn start(&self) -> BockResult<()> {
        let mut state = self.state.write();

        if !state.status.can_start() {
            return Err(bock_common::BockError::Config {
                message: format!(
                    "Container {} cannot be started (status: {})",
                    self.id, state.status
                ),
            });
        }

        tracing::info!(container_id = %self.id, "Starting container");

        let process = self
            .spec
            .process
            .as_ref()
            .ok_or_else(|| bock_common::BockError::Config {
                message: "No process config in spec".to_string(),
            })?;

        let args = process.args.clone();
        let env: Vec<(String, String)> = process
            .env
            .iter()
            .filter_map(|e| {
                let mut parts = e.splitn(2, '=');
                Some((parts.next()?.to_string(), parts.next()?.to_string()))
            })
            .collect();

        let rootfs = self.bundle.join("rootfs");

        // Create synchronization pipes
        // Using rustix::pipe::pipe() which returns Result<(OwnedFd, OwnedFd), Errno>
        let (parent_read, child_write) =
            rustix::pipe::pipe().map_err(|e| bock_common::BockError::Internal {
                message: e.to_string(),
            })?;
        let (child_read, parent_write) =
            rustix::pipe::pipe().map_err(|e| bock_common::BockError::Internal {
                message: e.to_string(),
            })?;

        let rootfs_clone = rootfs.clone();
        let ns_manager = self.namespace.clone();

        // Convert to RawFd for closure capture (OwnedFd is not Copy)
        use rustix::fd::AsRawFd;
        let c_read_fd = child_read.as_raw_fd();
        let c_write_fd = child_write.as_raw_fd();

        // Spawn process with setup hook
        let pid = crate::exec::process::spawn_process(&args, &env, move || {
            use std::io::{Read, Write};
            use std::os::unix::io::FromRawFd;

            // Reconstruct pipes from raw fds (using RawFd i32)
            let mut c_read = unsafe { std::fs::File::from_raw_fd(c_read_fd) };
            let mut c_write = unsafe { std::fs::File::from_raw_fd(c_write_fd) };

            // 1. Unshare namespaces
            if let Some(ns) = &ns_manager {
                ns.unshare()
                    .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
            }

            // 2. Signal parent "Unshared"
            c_write.write_all(b"UNSHARED")?;

            // 3. Wait for parent "Mappings Written"
            let mut buf = [0u8; 4];
            c_read.read_exact(&mut buf)?;

            // 4. Pivot root
            let old_root = rootfs_clone.join(".pivot_root");
            if !old_root.exists() {
                std::fs::create_dir(&old_root)?;
            }

            crate::filesystem::pivot_root(&rootfs_clone, &old_root)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;

            Ok(())
        })?;

        // Parent logic
        use rustix::fd::IntoRawFd;
        use std::io::{Read, Write};
        use std::os::unix::io::FromRawFd;
        let mut p_read = unsafe { std::fs::File::from_raw_fd(parent_read.into_raw_fd()) };
        let mut p_write = unsafe { std::fs::File::from_raw_fd(parent_write.into_raw_fd()) };

        // Wait for child unshare
        let mut buf = [0u8; 8];
        p_read
            .read_exact(&mut buf)
            .map_err(|e| bock_common::BockError::Internal {
                message: format!("Child failed to sync (read unshared): {}", e),
            })?;

        // Write ID mappings
        if let Some(ns) = &self.namespace {
            ns.write_uid_map(pid)?;
            ns.write_gid_map(pid)?;
        }

        // Network set up
        let host_if = format!(
            "veth{}",
            &self.id.as_str()[..std::cmp::min(6, self.id.as_str().len())]
        );
        let guest_if = format!(
            "ceth{}",
            &self.id.as_str()[..std::cmp::min(6, self.id.as_str().len())]
        );
        let veth = VethPair::create(&host_if, &guest_if).await?;
        veth.move_to_netns(pid).await?;

        // Signal child to proceed
        p_write
            .write_all(b"DONE")
            .map_err(|e| bock_common::BockError::Internal {
                message: format!("Failed to signal child: {}", e),
            })?;

        tracing::debug!(pid, "Container process spawned and synchronized");
        *self.pid.lock().await = Some(pid);

        // Save PID to file for persistence
        let container_dir = self.config.paths.container(self.id.as_str());
        let pid_path = container_dir.join("pid");
        if let Err(e) = std::fs::write(&pid_path, pid.to_string()) {
            return Err(bock_common::BockError::Internal {
                message: format!("Failed to write PID file: {}", e),
            });
        }

        state.set_running();
        drop(state); // Drop lock before saving
        self.save_state()?;

        Ok(())
    }

    /// Kill the container process.
    pub async fn kill(&self, signal: i32) -> BockResult<()> {
        let state = self.state.read();

        if state.status == ContainerStatus::Stopped {
            return Err(bock_common::BockError::Config {
                message: "Container is already stopped".to_string(),
            });
        }
        drop(state);

        // Try to get PID from memory first, then file
        let pid = match *self.pid.lock().await {
            Some(pid) => pid,
            None => {
                let container_dir = self.config.paths.container(self.id.as_str());
                let pid_path = container_dir.join("pid");
                if pid_path.exists() {
                    let pid_str = std::fs::read_to_string(&pid_path).map_err(|e| {
                        bock_common::BockError::Internal {
                            message: format!("Failed to read PID file: {}", e),
                        }
                    })?;
                    let pid = pid_str.trim().parse::<u32>().map_err(|_| {
                        bock_common::BockError::Internal {
                            message: "Invalid PID in PID file".to_string(),
                        }
                    })?;
                    *self.pid.lock().await = Some(pid);
                    pid
                } else {
                    return Err(bock_common::BockError::Config {
                        message: "Container not running (no PID found)".to_string(),
                    });
                }
            }
        };

        unsafe {
            if libc::kill(pid as i32, signal) != 0 {
                return Err(bock_common::BockError::Internal {
                    message: format!(
                        "Failed to send signal {}: {}",
                        signal,
                        std::io::Error::last_os_error()
                    ),
                });
            }
        }

        Ok(())
    }

    /// Delete the container.
    pub async fn delete(&self) -> BockResult<()> {
        let state = self.state.read();

        if state.status == ContainerStatus::Running {
            return Err(bock_common::BockError::Config {
                message: "Cannot delete running container. Stop it first.".to_string(),
            });
        }

        // Remove cgroup
        if let Some(cgroup) = &self.cgroup {
            let _ = cgroup.delete();
        }

        // Remove container directory
        let container_dir = self.config.paths.container(self.id.as_str());
        if container_dir.exists() {
            std::fs::remove_dir_all(&container_dir)?;
        }

        // Cleanup network
        let host_if = format!(
            "veth{}",
            &self.id.as_str()[..std::cmp::min(6, self.id.as_str().len())]
        );
        let guest_if = format!(
            "ceth{}",
            &self.id.as_str()[..std::cmp::min(6, self.id.as_str().len())]
        );
        let veth = VethPair {
            host: host_if,
            container: guest_if,
        };
        // Ignore errors during deletion (might not exist)
        let _ = veth.delete().await;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn create_container() {
        let spec = Spec::default();
        let container = Container::create(
            "test-container",
            "/tmp/bundle",
            &spec,
            RuntimeConfig::default(),
        )
        .await
        .unwrap();

        assert_eq!(container.id().as_str(), "test-container");
        assert_eq!(container.status(), ContainerStatus::Creating);
    }
}
