//! OCI lifecycle hooks.
//!
//! This module implements execution of OCI runtime lifecycle hooks.
//! Hooks are executed at specific points in the container lifecycle:
//! - `prestart`: Before the container process starts
//! - `poststart`: After the container process starts
//! - `poststop`: After the container process exits

use std::io::Write;
use std::process::{Command, Stdio};
use std::time::Duration;

use bock_common::BockResult;
use bock_oci::ContainerState;
use bock_oci::runtime::Hook;

/// Execute a single hook.
///
/// According to the OCI spec, the container state is passed to the hook's
/// stdin as JSON. The hook must exit with a zero status code.
pub fn run_hook(hook: &Hook, state: &ContainerState) -> BockResult<()> {
    let path = &hook.path;

    if !path.exists() {
        return Err(bock_common::BockError::Config {
            message: format!("Hook path does not exist: {}", path.display()),
        });
    }

    tracing::debug!(
        path = %path.display(),
        args = ?hook.args,
        timeout = ?hook.timeout,
        "Running hook"
    );

    // Build the command
    let mut cmd = Command::new(path);

    // Add arguments (OCI spec: args[0] should be the path itself if present)
    if hook.args.len() > 1 {
        cmd.args(&hook.args[1..]);
    }

    // Set environment variables
    for env_str in &hook.env {
        if let Some((key, value)) = env_str.split_once('=') {
            cmd.env(key, value);
        }
    }

    // Set up stdin for passing container state
    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::null());
    cmd.stderr(Stdio::inherit());

    // Spawn the process
    let mut child = cmd.spawn().map_err(|e| bock_common::BockError::Internal {
        message: format!("Failed to spawn hook {}: {}", path.display(), e),
    })?;

    // Write container state to stdin
    let state_json =
        serde_json::to_string(state).map_err(|e| bock_common::BockError::Internal {
            message: format!("Failed to serialize container state: {}", e),
        })?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(state_json.as_bytes())
            .map_err(|e| bock_common::BockError::Internal {
                message: format!("Failed to write to hook stdin: {}", e),
            })?;
    }

    // Wait for hook with optional timeout
    let timeout_dur = hook.timeout.map(|t| Duration::from_secs(t as u64));

    let status = if let Some(timeout) = timeout_dur {
        // Wait with timeout
        wait_with_timeout(&mut child, timeout)?
    } else {
        // Wait indefinitely
        child.wait().map_err(|e| bock_common::BockError::Internal {
            message: format!("Failed to wait for hook: {}", e),
        })?
    };

    if !status.success() {
        return Err(bock_common::BockError::Internal {
            message: format!(
                "Hook {} exited with non-zero status: {:?}",
                path.display(),
                status.code()
            ),
        });
    }

    tracing::debug!(path = %path.display(), "Hook completed successfully");
    Ok(())
}

/// Wait for a process with a timeout.
fn wait_with_timeout(
    child: &mut std::process::Child,
    timeout: Duration,
) -> BockResult<std::process::ExitStatus> {
    use std::time::Instant;

    let start = Instant::now();
    let poll_interval = Duration::from_millis(100);

    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) => {
                if start.elapsed() >= timeout {
                    // Timeout reached, kill the process
                    let _ = child.kill();
                    let _ = child.wait(); // Reap the zombie
                    return Err(bock_common::BockError::Internal {
                        message: "Hook timed out".to_string(),
                    });
                }
                std::thread::sleep(poll_interval);
            }
            Err(e) => {
                return Err(bock_common::BockError::Internal {
                    message: format!("Error waiting for hook: {}", e),
                });
            }
        }
    }
}

/// Run a list of hooks.
pub fn run_hooks(hooks: &[Hook], state: &ContainerState) -> BockResult<()> {
    for hook in hooks {
        run_hook(hook, state)?;
    }
    Ok(())
}

/// Run prestart hooks.
pub fn run_prestart_hooks(hooks: &[Hook], state: &ContainerState) -> BockResult<()> {
    tracing::debug!(count = hooks.len(), "Running prestart hooks");
    run_hooks(hooks, state)
}

/// Run poststart hooks.
pub fn run_poststart_hooks(hooks: &[Hook], state: &ContainerState) -> BockResult<()> {
    tracing::debug!(count = hooks.len(), "Running poststart hooks");
    run_hooks(hooks, state)
}

/// Run poststop hooks.
pub fn run_poststop_hooks(hooks: &[Hook], state: &ContainerState) -> BockResult<()> {
    tracing::debug!(count = hooks.len(), "Running poststop hooks");
    run_hooks(hooks, state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_run_hooks_empty() {
        let state = ContainerState::new("test", "/bundle");
        let result = run_hooks(&[], &state);
        assert!(result.is_ok());
    }

    #[test]
    fn test_hook_missing_path() {
        let hook = Hook {
            path: PathBuf::from("/nonexistent/hook"),
            args: vec![],
            env: vec![],
            timeout: None,
        };
        let state = ContainerState::new("test", "/bundle");
        let result = run_hook(&hook, &state);
        assert!(result.is_err());
    }

    #[test]
    #[cfg(unix)]
    fn test_run_true_hook() {
        // /bin/true should exist on most Unix systems
        let true_path = PathBuf::from("/bin/true");
        if !true_path.exists() {
            return; // Skip if not available
        }

        let hook = Hook {
            path: true_path,
            args: vec![],
            env: vec![],
            timeout: Some(5),
        };
        let state = ContainerState::new("test", "/bundle");
        let result = run_hook(&hook, &state);
        assert!(result.is_ok());
    }
}
