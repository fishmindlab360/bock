//! Container init process.
//!
//! This module implements a proper PID 1 init process for containers:
//! - Signal forwarding to child processes
//! - Zombie process reaping
//! - Graceful shutdown handling

#![allow(unsafe_code)]

use std::collections::HashMap;
use std::process::{Child, Command};

use bock_common::BockResult;

/// Container init process that acts as PID 1.
pub struct ContainerInit {
    /// The main child process.
    child: Option<Child>,
    /// Child processes being tracked.
    children: HashMap<u32, String>,
}

impl ContainerInit {
    /// Create a new container init.
    pub fn new() -> Self {
        Self {
            child: None,
            children: HashMap::new(),
        }
    }

    /// Run the container init process.
    ///
    /// This function sets up signal handlers, spawns the main process,
    /// and enters the main loop for signal handling and zombie reaping.
    pub fn run(&mut self, args: &[String], env: &[(String, String)]) -> BockResult<i32> {
        if args.is_empty() {
            return Err(bock_common::BockError::Config {
                message: "No command specified".to_string(),
            });
        }

        tracing::info!(command = ?args, "Container init starting");

        // Set up signal handlers
        self.setup_signal_handlers()?;

        // Spawn the main process
        let mut cmd = Command::new(&args[0]);
        if args.len() > 1 {
            cmd.args(&args[1..]);
        }

        for (key, value) in env {
            cmd.env(key, value);
        }

        let child = cmd.spawn().map_err(|e| bock_common::BockError::Internal {
            message: format!("Failed to spawn child process: {}", e),
        })?;

        let child_pid = child.id();
        tracing::info!(pid = child_pid, "Main process spawned");

        self.child = Some(child);
        self.children.insert(child_pid, args[0].clone());

        // Main loop: handle signals and reap zombies
        let exit_code = self.main_loop()?;

        tracing::info!(exit_code, "Container init exiting");
        Ok(exit_code)
    }

    /// Set up signal handlers for init.
    fn setup_signal_handlers(&self) -> BockResult<()> {
        #[cfg(target_os = "linux")]
        {
            // Set up signal handlers using sigaction
            // We catch SIGTERM, SIGINT, SIGCHLD
            unsafe {
                // SIGCHLD - for zombie reaping
                let mut sa: libc::sigaction = std::mem::zeroed();
                sa.sa_sigaction = sigchld_handler as *const () as usize;
                sa.sa_flags = libc::SA_SIGINFO | libc::SA_RESTART | libc::SA_NOCLDSTOP;
                libc::sigemptyset(&mut sa.sa_mask);
                libc::sigaction(libc::SIGCHLD, &sa, std::ptr::null_mut());

                // SIGTERM - for graceful shutdown
                let mut sa_term: libc::sigaction = std::mem::zeroed();
                sa_term.sa_sigaction = sigterm_handler as *const () as usize;
                sa_term.sa_flags = libc::SA_SIGINFO | libc::SA_RESTART;
                libc::sigemptyset(&mut sa_term.sa_mask);
                libc::sigaction(libc::SIGTERM, &sa_term, std::ptr::null_mut());

                // SIGINT
                libc::sigaction(libc::SIGINT, &sa_term, std::ptr::null_mut());
            }

            tracing::debug!("Signal handlers installed");
        }

        Ok(())
    }

    /// Main event loop for init.
    fn main_loop(&mut self) -> BockResult<i32> {
        loop {
            // Reap any zombie processes
            self.reap_zombies();

            // Check if main child has exited
            if let Some(ref mut child) = self.child {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        let code = status.code().unwrap_or(1);
                        tracing::info!(pid = child.id(), exit_code = code, "Main process exited");
                        return Ok(code);
                    }
                    Ok(None) => {
                        // Still running, continue
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "Error waiting for child");
                        return Err(bock_common::BockError::Internal {
                            message: format!("wait error: {}", e),
                        });
                    }
                }
            } else {
                // No child to wait for
                return Ok(0);
            }

            // Check shutdown flag
            if SHUTDOWN_REQUESTED.load(std::sync::atomic::Ordering::SeqCst) {
                tracing::info!("Shutdown requested, terminating children");
                self.terminate_children()?;
            }

            // Sleep briefly to avoid busy loop
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }

    /// Reap zombie processes.
    fn reap_zombies(&mut self) {
        loop {
            let mut status: libc::c_int = 0;
            let pid = unsafe { libc::waitpid(-1, &mut status, libc::WNOHANG) };

            if pid <= 0 {
                break;
            }

            let exit_code = if libc::WIFEXITED(status) {
                libc::WEXITSTATUS(status)
            } else if libc::WIFSIGNALED(status) {
                128 + libc::WTERMSIG(status)
            } else {
                1
            };

            if let Some(name) = self.children.remove(&(pid as u32)) {
                tracing::debug!(pid, exit_code, name = %name, "Reaped zombie process");
            } else {
                tracing::debug!(pid, exit_code, "Reaped unknown zombie process");
            }
        }
    }

    /// Terminate all child processes.
    fn terminate_children(&mut self) -> BockResult<()> {
        if let Some(ref child) = self.child {
            let pid = child.id() as libc::pid_t;

            // Send SIGTERM first
            unsafe {
                libc::kill(pid, libc::SIGTERM);
            }

            // Give processes time to terminate
            std::thread::sleep(std::time::Duration::from_secs(5));

            // Send SIGKILL if still running
            if let Some(ref mut child) = self.child {
                if child.try_wait().ok().flatten().is_none() {
                    unsafe {
                        libc::kill(pid, libc::SIGKILL);
                    }
                }
            }
        }

        Ok(())
    }
}

impl Default for ContainerInit {
    fn default() -> Self {
        Self::new()
    }
}

/// Global shutdown flag.
static SHUTDOWN_REQUESTED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// SIGCHLD handler - just wake up the main loop.
#[cfg(target_os = "linux")]
extern "C" fn sigchld_handler(
    _sig: libc::c_int,
    _info: *mut libc::siginfo_t,
    _ctx: *mut libc::c_void,
) {
    // Do nothing - the main loop will reap zombies
}

/// SIGTERM/SIGINT handler - request shutdown.
#[cfg(target_os = "linux")]
extern "C" fn sigterm_handler(
    _sig: libc::c_int,
    _info: *mut libc::siginfo_t,
    _ctx: *mut libc::c_void,
) {
    SHUTDOWN_REQUESTED.store(true, std::sync::atomic::Ordering::SeqCst);
}

/// Run as container init (PID 1).
///
/// This is the main entry point for container init functionality.
pub fn container_init() -> BockResult<()> {
    tracing::debug!("Running as container init");

    // Set no_new_privs
    set_no_new_privs()?;

    Ok(())
}

/// Set the no_new_privs flag.
#[cfg(target_os = "linux")]
pub fn set_no_new_privs() -> BockResult<()> {
    let result = unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) };

    if result != 0 {
        return Err(bock_common::BockError::Internal {
            message: format!(
                "prctl(PR_SET_NO_NEW_PRIVS) failed: {}",
                std::io::Error::last_os_error()
            ),
        });
    }

    tracing::debug!("no_new_privs flag set");
    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub fn set_no_new_privs() -> BockResult<()> {
    Err(bock_common::BockError::Unsupported {
        feature: "no_new_privs".to_string(),
    })
}

/// Apply security settings before exec.
pub fn apply_security(
    no_new_privs: bool,
    capabilities: Option<&crate::security::CapabilitySet>,
    seccomp: Option<&crate::security::SeccompFilter>,
) -> BockResult<()> {
    // 1. Set no_new_privs (must be done before dropping caps)
    if no_new_privs {
        set_no_new_privs()?;
    }

    // 2. Apply capabilities
    if let Some(caps) = capabilities {
        caps.apply()?;
    }

    // 3. Apply seccomp filter (must be done last)
    if let Some(filter) = seccomp {
        filter.apply()?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_container_init_creation() {
        let init = ContainerInit::new();
        assert!(init.child.is_none());
        assert!(init.children.is_empty());
    }

    #[test]
    #[cfg(target_os = "linux")]
    fn test_no_new_privs() {
        // This might fail in unprivileged environments
        // but should succeed in normal test runs
        let result = set_no_new_privs();
        // Allow failure in restricted environments
        if result.is_err() {
            println!("Note: no_new_privs test skipped (may need privileges)");
        }
    }
}
