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
use crate::runtime::RuntimeEvent;

use std::ffi::CString;

/// Namespace types to enter when executing in a container.
const NAMESPACE_TYPES: &[(&str, libc::c_int)] = &[
    ("mnt", libc::CLONE_NEWNS),
    ("uts", libc::CLONE_NEWUTS),
    ("ipc", libc::CLONE_NEWIPC),
    ("net", libc::CLONE_NEWNET),
    ("pid", libc::CLONE_NEWPID),
    ("cgroup", libc::CLONE_NEWCGROUP),
];

/// Execute a command inside a container's namespaces.
///
/// This function forks, enters the container's namespaces via /proc/{pid}/ns/*,
/// and executes the given command.
fn exec_in_container(
    container_pid: u32,
    args: &[String],
    env: &[(String, String)],
    cwd: Option<&str>,
) -> BockResult<i32> {
    use std::os::unix::io::AsRawFd;

    // Open namespace file descriptors before forking
    let ns_fds: Vec<(libc::c_int, std::fs::File)> = NAMESPACE_TYPES
        .iter()
        .filter_map(|(ns_name, ns_flag)| {
            let ns_path = format!("/proc/{}/ns/{}", container_pid, ns_name);
            match std::fs::File::open(&ns_path) {
                Ok(file) => Some((*ns_flag, file)),
                Err(_) => None, // Namespace might not exist
            }
        })
        .collect();

    // Fork a child process
    let pid = unsafe { libc::fork() };

    if pid < 0 {
        return Err(bock_common::BockError::Internal {
            message: format!("fork failed: {}", std::io::Error::last_os_error()),
        });
    }

    if pid == 0 {
        // Child process: enter namespaces and exec

        // Enter each namespace
        for (_flag, file) in &ns_fds {
            let fd = file.as_raw_fd();
            if unsafe { libc::setns(fd, 0) } != 0 {
                let err = std::io::Error::last_os_error();
                eprintln!("setns failed: {}", err);
                unsafe { libc::_exit(1) };
            }
        }

        // Change working directory if specified
        if let Some(dir) = cwd {
            if let Ok(cdir) = CString::new(dir) {
                unsafe { libc::chdir(cdir.as_ptr()) };
            }
        }

        // Set environment variables
        // SAFETY: We are in a forked child process, no other threads exist
        for (key, value) in env {
            unsafe { std::env::set_var(key, value) };
        }

        // Prepare arguments for execvp
        let c_args: Vec<CString> = args
            .iter()
            .filter_map(|s| CString::new(s.as_bytes()).ok())
            .collect();

        let c_arg_ptrs: Vec<*const libc::c_char> = c_args
            .iter()
            .map(|s| s.as_ptr())
            .chain(std::iter::once(std::ptr::null()))
            .collect();

        // Execute the command
        unsafe {
            libc::execvp(c_arg_ptrs[0], c_arg_ptrs.as_ptr());
        }

        // If execvp returns, it failed
        unsafe { libc::_exit(127) };
    }

    // Parent process: wait for child
    let mut status: libc::c_int = 0;
    loop {
        let result = unsafe { libc::waitpid(pid, &mut status, 0) };
        if result == -1 {
            let err = std::io::Error::last_os_error();
            if err.kind() == std::io::ErrorKind::Interrupted {
                continue;
            }
            return Err(bock_common::BockError::Internal {
                message: format!("waitpid failed: {}", err),
            });
        }
        break;
    }

    if libc::WIFEXITED(status) {
        Ok(libc::WEXITSTATUS(status))
    } else if libc::WIFSIGNALED(status) {
        Ok(128 + libc::WTERMSIG(status))
    } else {
        Ok(1)
    }
}

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
    /// Bundle path.
    bundle: PathBuf,
    /// Network configuration.
    network_config: Option<NetworkConfig>,
}

/// Network configuration for the container.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct NetworkConfig {
    /// IP address (CIDR format, e.g., "172.16.0.2/24").
    pub ip: String,
    /// Gateway address (e.g., "172.16.0.1").
    pub gateway: String,
}

/// Container statistics.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ContainerStats {
    /// CPU usage in microseconds.
    pub cpu_usage_usec: u64,
    /// Memory usage in bytes.
    pub memory_usage_bytes: u64,
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

        let container = Self {
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
            network_config: None,
        };

        container
            .config
            .event_bus
            .publish(RuntimeEvent::ContainerCreated {
                id: container.id.to_string(),
                timestamp: chrono::Utc::now().timestamp(),
            });

        Ok(container)
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

        // Load network config if present
        let network_config = {
            let container_dir = config.paths.container(state.id.as_str());
            let path = container_dir.join("network.json");
            if path.exists() {
                let json = std::fs::read_to_string(path)?;
                Some(serde_json::from_str(&json)?)
            } else {
                None
            }
        };

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
            network_config,
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

    /// Get container statistics.
    pub fn stats(&self) -> BockResult<ContainerStats> {
        if let Some(cgroup) = &self.cgroup {
            let cpu = cgroup.cpu_stats()?;
            let memory = cgroup.memory_usage()?;
            Ok(ContainerStats {
                cpu_usage_usec: cpu.usage_usec,
                memory_usage_bytes: memory,
            })
        } else {
            Err(bock_common::BockError::Config {
                message: "No cgroup manager available".to_string(),
            })
        }
    }

    /// Get the container PID.
    pub async fn pid(&self) -> Option<u32> {
        // Reload PID from somewhere? currently just memory.
        *self.pid.lock().await
    }

    /// Start the container.
    pub async fn start(&self) -> BockResult<()> {
        // Check status first with scoped lock
        {
            let state = self.state.read();
            if !state.status.can_start() {
                return Err(bock_common::BockError::Config {
                    message: format!(
                        "Container {} cannot be started (status: {})",
                        self.id, state.status
                    ),
                });
            }
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

        // Convert to RawFd for closure capture
        use rustix::fd::AsRawFd;
        let c_read_fd = child_read.as_raw_fd();
        let c_write_fd = child_write.as_raw_fd();

        // Prepare log files
        let container_dir = self.config.paths.container(self.id.as_str());
        let stdout_path = container_dir.join("stdout.log");
        let stderr_path = container_dir.join("stderr.log");

        let stdout_file =
            std::fs::File::create(&stdout_path).map_err(|e| bock_common::BockError::Io(e))?;
        let stderr_file =
            std::fs::File::create(&stderr_path).map_err(|e| bock_common::BockError::Io(e))?;

        let stdout = std::process::Stdio::from(stdout_file);
        let stderr = std::process::Stdio::from(stderr_file);

        // Spawn process with setup hook
        let pid = crate::exec::process::spawn_process(
            &args,
            &env,
            Some(stdout),
            Some(stderr),
            move || {
                use std::io::{Read, Write};
                use std::os::unix::io::FromRawFd;

                let mut c_read = unsafe { std::fs::File::from_raw_fd(c_read_fd) };
                let mut c_write = unsafe { std::fs::File::from_raw_fd(c_write_fd) };

                // 1. Unshare namespaces
                if let Some(ns) = &ns_manager {
                    ns.unshare().map_err(|e| {
                        std::io::Error::new(std::io::ErrorKind::Other, e.to_string())
                    })?;
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
            },
        )?;

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

        // Network set up (no locks held during await)
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

        // Configure network if specified
        if let Some(net_config) = &self.network_config {
            let pid_str = pid.to_string();
            tracing::debug!(pid = %pid, ip = %net_config.ip, gateway = %net_config.gateway, "Configuring container network");

            // Helper to run command in container namespace via nsenter
            let run_in_netns = |args: &[&str]| -> BockResult<()> {
                let status = std::process::Command::new("nsenter")
                    .arg("-t")
                    .arg(&pid_str)
                    .arg("-n")
                    .args(args)
                    .status()
                    .map_err(|e| bock_common::BockError::Internal {
                        message: format!("Failed to execute nsenter: {}", e),
                    })?;

                if !status.success() {
                    return Err(bock_common::BockError::Internal {
                        message: format!(
                            "Command in netns failed: {:?} (status: {})",
                            args, status
                        ),
                    });
                }
                Ok(())
            };

            // 1. Bring up loopback
            run_in_netns(&["ip", "link", "set", "lo", "up"])?;

            // 2. Bring up guest interface
            run_in_netns(&["ip", "link", "set", &guest_if, "up"])?;

            // 3. Assign IP address
            run_in_netns(&["ip", "addr", "add", &net_config.ip, "dev", &guest_if])?;

            // 4. Set default gateway
            run_in_netns(&["ip", "route", "add", "default", "via", &net_config.gateway])?;
        }

        // Signal child to proceed
        p_write
            .write_all(b"DONE")
            .map_err(|e| bock_common::BockError::Internal {
                message: format!("Failed to signal child: {}", e),
            })?;

        tracing::debug!(pid, "Container process spawned and synchronized");
        *self.pid.lock().await = Some(pid);

        // Save PID to file for persistence
        // container_dir is already defined above
        let pid_path = container_dir.join("pid");
        if let Err(e) = std::fs::write(&pid_path, pid.to_string()) {
            return Err(bock_common::BockError::Internal {
                message: format!("Failed to write PID file: {}", e),
            });
        }

        // Update state with scoped lock
        {
            let mut state = self.state.write();
            state.set_running();
        }
        self.save_state()?;

        self.config
            .event_bus
            .publish(RuntimeEvent::ContainerStarted {
                id: self.id.to_string(),
                timestamp: chrono::Utc::now().timestamp(),
            });

        Ok(())
    }

    /// Kill the container process.
    pub async fn kill(&self, signal: i32) -> BockResult<()> {
        // Check status with scoped lock
        {
            let state = self.state.read();
            if state.status == ContainerStatus::Stopped {
                return Err(bock_common::BockError::Config {
                    message: "Container is already stopped".to_string(),
                });
            }
        }

        let pid = self.get_or_load_pid().await?;

        tracing::debug!(container_id = %self.id, pid, signal, "Sending signal to container");

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

    /// Wait for the container process to exit and return the exit code.
    pub async fn wait(&self) -> BockResult<i32> {
        // Check status with scoped lock
        {
            let state = self.state.read();
            if state.status == ContainerStatus::Stopped {
                return Ok(0); // Already stopped
            }
            if state.status != ContainerStatus::Running && state.status != ContainerStatus::Paused {
                return Err(bock_common::BockError::Config {
                    message: format!(
                        "Container {} is not running (status: {})",
                        self.id, state.status
                    ),
                });
            }
        }

        let pid = self.get_or_load_pid().await?;

        tracing::info!(container_id = %self.id, pid, "Waiting for container to exit");

        // Wait for the process using waitpid
        let exit_code = tokio::task::spawn_blocking(move || {
            let mut status: libc::c_int = 0;
            loop {
                let result = unsafe { libc::waitpid(pid as i32, &mut status, 0) };
                if result == -1 {
                    let err = std::io::Error::last_os_error();
                    if err.kind() == std::io::ErrorKind::Interrupted {
                        continue; // EINTR, retry
                    }
                    // If ECHILD, process might have already been reaped
                    if err.raw_os_error() == Some(libc::ECHILD) {
                        return Ok(0);
                    }
                    return Err(bock_common::BockError::Internal {
                        message: format!("waitpid failed: {}", err),
                    });
                }
                break;
            }

            // Extract exit code
            if libc::WIFEXITED(status) {
                Ok(libc::WEXITSTATUS(status))
            } else if libc::WIFSIGNALED(status) {
                Ok(128 + libc::WTERMSIG(status))
            } else {
                Ok(1)
            }
        })
        .await
        .map_err(|e| bock_common::BockError::Internal {
            message: format!("Task join error: {}", e),
        })??;

        // Update state with scoped lock
        {
            let mut state = self.state.write();
            state.set_stopped();
        }
        self.save_state()?;

        self.config
            .event_bus
            .publish(RuntimeEvent::ContainerStopped {
                id: self.id.to_string(),
                timestamp: chrono::Utc::now().timestamp(),
            });

        // Clean up PID file
        let container_dir = self.config.paths.container(self.id.as_str());
        let _ = std::fs::remove_file(container_dir.join("pid"));

        tracing::info!(container_id = %self.id, exit_code, "Container exited");
        Ok(exit_code)
    }

    /// Pause the container using cgroup freeze.
    pub async fn pause(&self) -> BockResult<()> {
        let state = self.state.read();
        if !state.status.can_pause() {
            return Err(bock_common::BockError::Config {
                message: format!(
                    "Container {} cannot be paused (status: {})",
                    self.id, state.status
                ),
            });
        }
        drop(state);

        let cgroup = self
            .cgroup
            .as_ref()
            .ok_or_else(|| bock_common::BockError::Config {
                message: "Cannot pause container: no cgroup manager available".to_string(),
            })?;

        cgroup.freeze()?;

        let mut state = self.state.write();
        state.set_paused();
        drop(state);
        self.save_state()?;

        tracing::info!(container_id = %self.id, "Container paused");
        Ok(())
    }

    /// Resume a paused container.
    pub async fn resume(&self) -> BockResult<()> {
        let state = self.state.read();
        if !state.status.can_resume() {
            return Err(bock_common::BockError::Config {
                message: format!(
                    "Container {} cannot be resumed (status: {})",
                    self.id, state.status
                ),
            });
        }
        drop(state);

        let cgroup = self
            .cgroup
            .as_ref()
            .ok_or_else(|| bock_common::BockError::Config {
                message: "Cannot resume container: no cgroup manager available".to_string(),
            })?;

        cgroup.unfreeze()?;

        let mut state = self.state.write();
        state.set_running();
        drop(state);
        self.save_state()?;

        tracing::info!(container_id = %self.id, "Container resumed");
        Ok(())
    }

    /// Execute a command in a running container.
    ///
    /// This joins the container's namespaces and executes the specified command.
    pub async fn exec(
        &self,
        args: &[String],
        env: &[(String, String)],
        cwd: Option<&str>,
    ) -> BockResult<i32> {
        let state = self.state.read();
        if state.status != ContainerStatus::Running {
            return Err(bock_common::BockError::Config {
                message: format!(
                    "Container {} is not running (status: {})",
                    self.id, state.status
                ),
            });
        }
        drop(state);

        if args.is_empty() {
            return Err(bock_common::BockError::Config {
                message: "No command specified for exec".to_string(),
            });
        }

        let pid = self.get_or_load_pid().await?;

        tracing::info!(
            container_id = %self.id,
            pid,
            command = ?args,
            "Executing command in container"
        );

        // Clone data for the spawned task
        let args = args.to_vec();
        let env: Vec<(String, String)> = env.to_vec();
        let cwd = cwd.map(|s| s.to_string());

        // Execute in a blocking task since we need to fork and enter namespaces
        let exit_code = tokio::task::spawn_blocking(move || {
            exec_in_container(pid, &args, &env, cwd.as_deref())
        })
        .await
        .map_err(|e| bock_common::BockError::Internal {
            message: format!("Task join error: {}", e),
        })??;

        Ok(exit_code)
    }

    /// Get the container PID from memory or load from file.
    async fn get_or_load_pid(&self) -> BockResult<u32> {
        let mut pid_guard = self.pid.lock().await;
        if let Some(pid) = *pid_guard {
            return Ok(pid);
        }

        let container_dir = self.config.paths.container(self.id.as_str());
        let pid_path = container_dir.join("pid");
        if pid_path.exists() {
            let pid_str = std::fs::read_to_string(&pid_path).map_err(|e| {
                bock_common::BockError::Internal {
                    message: format!("Failed to read PID file: {}", e),
                }
            })?;
            let pid =
                pid_str
                    .trim()
                    .parse::<u32>()
                    .map_err(|_| bock_common::BockError::Internal {
                        message: "Invalid PID in PID file".to_string(),
                    })?;
            *pid_guard = Some(pid);
            return Ok(pid);
        }

        Err(bock_common::BockError::Config {
            message: "Container not running (no PID found)".to_string(),
        })
    }

    /// Delete the container.
    pub async fn delete(&self) -> BockResult<()> {
        // Check status with scoped lock
        {
            let state = self.state.read();
            if state.status == ContainerStatus::Running {
                return Err(bock_common::BockError::Config {
                    message: "Cannot delete running container. Stop it first.".to_string(),
                });
            }
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

        // Cleanup network (no locks held during await)
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

    /// Execute a command_inside the container (via nsenter).
    pub async fn exec_command(&self, cmd: &[String]) -> BockResult<i32> {
        let pid = self.get_or_load_pid().await?;
        let pid_str = pid.to_string();

        let mut args = vec!["-t", &pid_str, "-a", "--"];
        let cmd_refs: Vec<&str> = cmd.iter().map(|s| s.as_str()).collect();
        args.extend(cmd_refs);

        tracing::debug!(pid, command = ?cmd, "Executing command in container");

        let status = std::process::Command::new("nsenter")
            .args(&args)
            .status()
            .map_err(|e| bock_common::BockError::Internal {
                message: format!("Failed to execute nsenter: {}", e),
            })?;

        Ok(status.code().unwrap_or(-1))
    }

    /// Set network configuration.
    pub fn set_network_config(&mut self, config: NetworkConfig) -> BockResult<()> {
        self.network_config = Some(config);
        self.save_network_config()
    }

    /// Get network configuration.
    pub fn network_config(&self) -> Option<&NetworkConfig> {
        self.network_config.as_ref()
    }

    /// Save network configuration.
    fn save_network_config(&self) -> BockResult<()> {
        if let Some(config) = &self.network_config {
            let container_dir = self.config.paths.container(self.id.as_str());
            let path = container_dir.join("network.json");
            let json = serde_json::to_string_pretty(config).map_err(|e| {
                bock_common::BockError::Internal {
                    message: format!("Failed to serialize network config: {}", e),
                }
            })?;
            std::fs::write(&path, json).map_err(|e| bock_common::BockError::Internal {
                message: format!(
                    "Failed to write network config to {}: {}",
                    path.display(),
                    e
                ),
            })?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn create_container() {
        let temp = tempfile::tempdir().expect("Failed to create temp dir");
        let bundle_path = temp.path().join("bundle");
        let rootfs = bundle_path.join("rootfs");
        std::fs::create_dir_all(&rootfs).unwrap();

        let spec = Spec::default();
        std::fs::write(
            bundle_path.join("config.json"),
            serde_json::to_string(&spec).unwrap(),
        )
        .unwrap();

        let root = temp.path().join("root");
        let config = RuntimeConfig::default().with_root(root);

        let container = Container::create(
            "test-container",
            bundle_path.to_str().unwrap(),
            &spec,
            config,
        )
        .await
        .unwrap();

        assert_eq!(container.id().as_str(), "test-container");
        assert_eq!(container.status(), ContainerStatus::Creating);
    }
}
