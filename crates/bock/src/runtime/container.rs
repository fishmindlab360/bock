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

use std::collections::hash_map::DefaultHasher;
use std::ffi::CString;
use std::hash::{Hash, Hasher};

/// Generate unique veth interface names based on container ID hash.
/// Returns (host_interface, container_interface) tuple.
fn generate_veth_names(id: &str) -> (String, String) {
    let mut hasher = DefaultHasher::new();
    id.hash(&mut hasher);
    let hash = hasher.finish();
    // Use 7 hex chars for uniqueness (veth + 7 = 11 chars, under 15 char limit)
    let suffix = format!("{:07x}", hash & 0x0FFFFFFF);
    (format!("veth{}", suffix), format!("ceth{}", suffix))
}

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
    /// Port mappings (host_port:container_port).
    #[serde(default)]
    pub ports: Vec<String>,
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

        // Transition to Created status (allows starting)
        {
            let mut state = container.state.write();
            state.status = bock_oci::state::ContainerStatus::Created;
        }
        container.save_state()?;

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

        // Convert child pipe ends to raw FDs for closure capture
        // Use into_raw_fd() to transfer ownership - child process will manage these FDs
        use rustix::fd::IntoRawFd;
        let c_read_fd = child_read.into_raw_fd();
        let c_write_fd = child_write.into_raw_fd();

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

        // Clone data needed for background handshake task
        let namespace_for_task = self.namespace.clone();
        let network_config_for_task = self.network_config.clone();
        let container_id_for_task = self.id.clone();

        // Convert parent pipe ends to raw FDs for the handshake task
        let p_read_fd = parent_read.into_raw_fd();
        let p_write_fd = parent_write.into_raw_fd();

        // Start parent handshake in background BEFORE spawn_process
        // This breaks the deadlock: parent handshake runs concurrently with child setup
        let handshake_handle = tokio::task::spawn_blocking(move || -> BockResult<u32> {
            use std::io::{Read, Write};
            use std::os::unix::io::FromRawFd;

            let mut p_read = unsafe { std::fs::File::from_raw_fd(p_read_fd) };
            let mut p_write = unsafe { std::fs::File::from_raw_fd(p_write_fd) };

            tracing::info!("Parent handshake: waiting for child PID...");

            // 1. Read PID from child (child sends its PID as 4-byte u32 LE)
            let mut pid_buf = [0u8; 4];
            p_read
                .read_exact(&mut pid_buf)
                .map_err(|e| bock_common::BockError::Internal {
                    message: format!("Failed to read child PID: {}", e),
                })?;
            let pid = u32::from_le_bytes(pid_buf);

            tracing::info!(
                "Parent handshake: got child PID {}. Writing ID maps...",
                pid
            );

            // 2. Write ID mappings
            if let Some(ns) = &namespace_for_task {
                ns.write_uid_map(pid)?;
                ns.write_gid_map(pid)?;
            }

            tracing::info!("Parent handshake: ID maps written. Setting up network...");

            // 3. Network setup (async operations - use Handle::block_on)
            let rt_handle = tokio::runtime::Handle::current();
            let (host_if, guest_if) = generate_veth_names(container_id_for_task.as_str());

            rt_handle.block_on(async {
                // Setup bridge and NAT (idempotent)
                bock_network::NatManager::setup_network().await?;

                // Clean up any existing interface with same name (from failed previous run)
                let cleanup_veth = bock_network::VethPair {
                    host: host_if.clone(),
                    container: guest_if.clone(),
                };
                let _ = cleanup_veth.delete().await; // Ignore errors

                tracing::info!("Creating veth pair {}/{}...", host_if, guest_if);
                let veth = bock_network::VethPair::create(&host_if, &guest_if).await?;

                // Attach host-side to bridge
                tracing::info!(
                    "Attaching {} to bridge {}...",
                    host_if,
                    bock_network::DEFAULT_BRIDGE
                );
                let bridge = bock_network::BridgeManager::get(bock_network::DEFAULT_BRIDGE)?;
                bridge.add_interface(&host_if).await?;

                // Now bring host interface up
                veth.bring_host_up().await?;

                // Move container-side to netns
                tracing::info!("Moving {} to netns {}...", guest_if, pid);
                veth.move_to_netns(pid).await?;

                Ok::<(), bock_common::BockError>(())
            })?;

            // 4. Configure network interfaces
            if let Some(net_config) = &network_config_for_task {
                tracing::info!("Configuring network interfaces via nsenter...");
                let pid_str = pid.to_string();

                let run_in_netns = |args: &[&str]| -> BockResult<()> {
                    let status = std::process::Command::new("nsenter")
                        .arg("-t")
                        .arg(&pid_str)
                        .arg("-n")
                        .args(args)
                        .status()
                        .map_err(|e| bock_common::BockError::Internal {
                            message: format!("nsenter failed: {}", e),
                        })?;
                    if !status.success() {
                        return Err(bock_common::BockError::Internal {
                            message: format!("netns command failed: {:?}", args),
                        });
                    }
                    Ok(())
                };

                run_in_netns(&["ip", "link", "set", "lo", "up"])?;
                run_in_netns(&["ip", "link", "set", &guest_if, "up"])?;
                run_in_netns(&["ip", "addr", "add", &net_config.ip, "dev", &guest_if])?;
                run_in_netns(&["ip", "route", "add", "default", "via", &net_config.gateway])?;
            }

            tracing::info!("Parent handshake: network configured. Signaling child DONE...");

            // 5. Signal child to proceed
            p_write
                .write_all(b"DONE")
                .map_err(|e| bock_common::BockError::Internal {
                    message: format!("Failed to signal child: {}", e),
                })?;

            tracing::info!("Parent handshake complete.");
            Ok(pid)
        });

        // Spawn child process - this will now unblock because parent handshake runs concurrently
        tracing::info!("Spawning container process...");
        let _spawn_pid = crate::exec::process::spawn_process(
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

                // 2. Send our PID to parent (instead of just "UNSHARED")
                let self_pid = std::process::id();
                c_write.write_all(&self_pid.to_le_bytes())?;

                // 3. Wait for parent "DONE"
                let mut buf = [0u8; 4];
                c_read.read_exact(&mut buf)?;

                // 4. Pivot root setup
                // First, make the entire mount tree private to prevent propagation
                // This is essential after CLONE_NEWNS to isolate container mounts
                use std::ffi::CString;
                let root = CString::new("/").unwrap();
                unsafe {
                    // MS_REC | MS_PRIVATE = make all mounts private recursively
                    if libc::mount(
                        std::ptr::null(),
                        root.as_ptr(),
                        std::ptr::null(),
                        libc::MS_REC | libc::MS_PRIVATE,
                        std::ptr::null(),
                    ) != 0
                    {
                        let err = std::io::Error::last_os_error();
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            format!("Failed to make root private: {}", err),
                        ));
                    }
                }

                // Bind mount rootfs to itself (required for pivot_root)
                let rootfs_c = CString::new(rootfs_clone.to_string_lossy().as_bytes()).unwrap();
                unsafe {
                    if libc::mount(
                        rootfs_c.as_ptr(),
                        rootfs_c.as_ptr(),
                        std::ptr::null(),
                        libc::MS_BIND | libc::MS_REC,
                        std::ptr::null(),
                    ) != 0
                    {
                        let err = std::io::Error::last_os_error();
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            format!("Failed to bind mount rootfs: {}", err),
                        ));
                    }
                }

                // Create put_old directory
                let old_root = rootfs_clone.join(".pivot_root");
                if !old_root.exists() {
                    std::fs::create_dir(&old_root)?;
                }

                // Perform pivot_root
                crate::filesystem::pivot_root(&rootfs_clone, &old_root).map_err(|e| {
                    std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("pivot_root failed: {}", e),
                    )
                })?;

                // Unmount old root and clean up
                let old_root_c = CString::new("/.pivot_root").unwrap();
                unsafe {
                    libc::umount2(old_root_c.as_ptr(), libc::MNT_DETACH);
                }
                let _ = std::fs::remove_dir("/.pivot_root");

                // Mount essential filesystems for container functionality
                // Mount /proc (required for networking and most system tools)
                let proc_target = CString::new("/proc").unwrap();
                let proc_type = CString::new("proc").unwrap();
                let proc_source = CString::new("proc").unwrap();
                unsafe {
                    if libc::mount(
                        proc_source.as_ptr(),
                        proc_target.as_ptr(),
                        proc_type.as_ptr(),
                        0,
                        std::ptr::null(),
                    ) != 0
                    {
                        let err = std::io::Error::last_os_error();
                        // Log but don't fail - /proc might already be mounted
                        eprintln!("Warning: failed to mount /proc: {}", err);
                    }
                }

                // Mount /sys (required for device information)
                let sys_target = CString::new("/sys").unwrap();
                let sys_type = CString::new("sysfs").unwrap();
                let sys_source = CString::new("sysfs").unwrap();
                unsafe {
                    if libc::mount(
                        sys_source.as_ptr(),
                        sys_target.as_ptr(),
                        sys_type.as_ptr(),
                        libc::MS_RDONLY,
                        std::ptr::null(),
                    ) != 0
                    {
                        let err = std::io::Error::last_os_error();
                        eprintln!("Warning: failed to mount /sys: {}", err);
                    }
                }

                // Mount /dev as tmpfs
                let dev_target = CString::new("/dev").unwrap();
                let dev_type = CString::new("tmpfs").unwrap();
                let dev_source = CString::new("tmpfs").unwrap();
                let dev_opts = CString::new("mode=755,size=65536k").unwrap();
                unsafe {
                    if libc::mount(
                        dev_source.as_ptr(),
                        dev_target.as_ptr(),
                        dev_type.as_ptr(),
                        libc::MS_NOSUID | libc::MS_STRICTATIME,
                        dev_opts.as_ptr() as *const libc::c_void,
                    ) != 0
                    {
                        let err = std::io::Error::last_os_error();
                        eprintln!("Warning: failed to mount /dev: {}", err);
                    }
                }

                // Create essential device symlinks
                let _ = std::os::unix::fs::symlink("/proc/self/fd", "/dev/fd");
                let _ = std::os::unix::fs::symlink("/proc/self/fd/0", "/dev/stdin");
                let _ = std::os::unix::fs::symlink("/proc/self/fd/1", "/dev/stdout");
                let _ = std::os::unix::fs::symlink("/proc/self/fd/2", "/dev/stderr");

                Ok(())
            },
        )?;

        // Wait for handshake to complete and get the PID
        let pid = handshake_handle
            .await
            .map_err(|e| bock_common::BockError::Internal {
                message: format!("Handshake task panicked: {}", e),
            })??;

        // Note: child pipe ends were consumed by into_raw_fd() above,
        // so no explicit drop needed - ownership transferred to child process

        tracing::info!("Container start handshake complete. PID: {}", pid);

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
        let (host_if, guest_if) = generate_veth_names(self.id.as_str());
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
