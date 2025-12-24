//! Network namespace operations.
//!
//! This module provides utilities for creating, managing, and entering
//! Linux network namespaces.
#![allow(unsafe_code)]

use std::fs::File;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::process::Command;

use bock_common::BockResult;

/// Path to network namespace directory.
const NETNS_DIR: &str = "/var/run/netns";

/// Create a named network namespace.
pub fn create_netns(name: &str) -> BockResult<()> {
    tracing::debug!(name, "Creating network namespace");

    // Ensure netns directory exists
    if !Path::new(NETNS_DIR).exists() {
        std::fs::create_dir_all(NETNS_DIR)?;
    }

    let status = Command::new("ip")
        .args(["netns", "add", name])
        .status()
        .map_err(|e| bock_common::BockError::Internal {
            message: format!("Failed to execute ip netns add: {}", e),
        })?;

    if !status.success() {
        return Err(bock_common::BockError::Internal {
            message: format!("Failed to create network namespace '{}'", name),
        });
    }

    tracing::info!(name, "Network namespace created");
    Ok(())
}

/// Delete a named network namespace.
pub fn delete_netns(name: &str) -> BockResult<()> {
    tracing::debug!(name, "Deleting network namespace");

    let status = Command::new("ip")
        .args(["netns", "delete", name])
        .status()
        .map_err(|e| bock_common::BockError::Internal {
            message: format!("Failed to execute ip netns delete: {}", e),
        })?;

    if !status.success() {
        tracing::warn!(name, "Failed to delete network namespace (may not exist)");
    }

    Ok(())
}

/// Enter a named network namespace.
///
/// This function uses `setns` to enter the specified network namespace.
/// Note: This affects the calling thread/process.
#[cfg(target_os = "linux")]
pub fn enter_netns(name: &str) -> BockResult<()> {
    tracing::debug!(name, "Entering network namespace");

    let ns_path: PathBuf = [NETNS_DIR, name].iter().collect();

    if !ns_path.exists() {
        return Err(bock_common::BockError::Config {
            message: format!("Network namespace '{}' does not exist", name),
        });
    }

    let file = File::open(&ns_path)?;
    let fd = file.as_raw_fd();

    // SAFETY: fd is a valid file descriptor obtained from File::open() which succeeded.
    // setns() with CLONE_NEWNET is safe when the fd points to a valid network namespace.
    // The file is kept open for the duration of this call.
    let result = unsafe { libc::setns(fd, libc::CLONE_NEWNET) };

    if result != 0 {
        return Err(bock_common::BockError::Internal {
            message: format!("setns failed: {}", std::io::Error::last_os_error()),
        });
    }

    tracing::info!(name, "Entered network namespace");
    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub fn enter_netns(_name: &str) -> BockResult<()> {
    Err(bock_common::BockError::Unsupported {
        feature: "network namespaces".to_string(),
    })
}

/// Enter a network namespace by PID.
///
/// This enters the network namespace of a running process.
#[cfg(target_os = "linux")]
pub fn enter_netns_by_pid(pid: u32) -> BockResult<()> {
    tracing::debug!(pid, "Entering network namespace by PID");

    let ns_path = format!("/proc/{}/ns/net", pid);

    if !Path::new(&ns_path).exists() {
        return Err(bock_common::BockError::Config {
            message: format!("Process {} does not exist or has no network namespace", pid),
        });
    }

    let file = File::open(&ns_path)?;
    let fd = file.as_raw_fd();

    // SAFETY: fd is a valid file descriptor from File::open() on /proc/<pid>/ns/net.
    // setns() is safe when the fd refers to a valid namespace file.
    let result = unsafe { libc::setns(fd, libc::CLONE_NEWNET) };

    if result != 0 {
        return Err(bock_common::BockError::Internal {
            message: format!("setns failed: {}", std::io::Error::last_os_error()),
        });
    }

    tracing::info!(pid, "Entered network namespace");
    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub fn enter_netns_by_pid(_pid: u32) -> BockResult<()> {
    Err(bock_common::BockError::Unsupported {
        feature: "network namespaces".to_string(),
    })
}

/// Check if a named network namespace exists.
pub fn netns_exists(name: &str) -> bool {
    let ns_path: PathBuf = [NETNS_DIR, name].iter().collect();
    ns_path.exists()
}

/// List all named network namespaces.
pub fn list_netns() -> BockResult<Vec<String>> {
    let netns_dir = Path::new(NETNS_DIR);

    if !netns_dir.exists() {
        return Ok(Vec::new());
    }

    let entries = std::fs::read_dir(netns_dir)?;
    let namespaces: Vec<String> = entries
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();

    Ok(namespaces)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_netns_dir_constant() {
        assert_eq!(NETNS_DIR, "/var/run/netns");
    }

    #[test]
    fn test_netns_exists_nonexistent() {
        assert!(!netns_exists("nonexistent_ns_12345"));
    }
}
