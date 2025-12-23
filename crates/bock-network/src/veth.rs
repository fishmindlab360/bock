//! Virtual ethernet pair management.

use bock_common::BockResult;
use std::process::Command;

/// Virtual ethernet pair.
pub struct VethPair {
    /// Host-side interface name.
    pub host: String,
    /// Container-side interface name.
    pub container: String,
}

impl VethPair {
    /// Create a new veth pair.
    pub async fn create(host_name: &str, container_name: &str) -> BockResult<Self> {
        tracing::debug!(host_name, container_name, "Creating veth pair");

        let status = Command::new("ip")
            .args([
                "link",
                "add",
                host_name,
                "type",
                "veth",
                "peer",
                "name",
                container_name,
            ])
            .status()
            .map_err(|e| bock_common::BockError::Internal {
                message: format!("Failed to execute ip link add: {}", e),
            })?;

        if !status.success() {
            return Err(bock_common::BockError::Internal {
                message: format!("ip link add failed with status: {}", status),
            });
        }

        // Bring host up
        let status = Command::new("ip")
            .args(["link", "set", host_name, "up"])
            .status()
            .map_err(|e| bock_common::BockError::Internal {
                message: format!("Failed to execute ip link set up: {}", e),
            })?;

        if !status.success() {
            return Err(bock_common::BockError::Internal {
                message: format!("ip link set up failed with status: {}", status),
            });
        }

        Ok(Self {
            host: host_name.to_string(),
            container: container_name.to_string(),
        })
    }

    /// Move the container side to a network namespace.
    pub async fn move_to_netns(&self, pid: u32) -> BockResult<()> {
        tracing::debug!(interface = %self.container, pid, "Moving to netns");

        let status = Command::new("ip")
            .args(["link", "set", &self.container, "netns", &pid.to_string()])
            .status()
            .map_err(|e| bock_common::BockError::Internal {
                message: format!("Failed to execute ip link set netns: {}", e),
            })?;

        if !status.success() {
            return Err(bock_common::BockError::Internal {
                message: format!(
                    "Failed to move interface to netns. ip command failed: {}",
                    status
                ),
            });
        }

        Ok(())
    }

    /// Delete the veth pair.
    pub async fn delete(&self) -> BockResult<()> {
        tracing::debug!(host = %self.host, "Deleting veth pair");

        let status = Command::new("ip")
            .args(["link", "delete", &self.host])
            .status()
            .map_err(|e| bock_common::BockError::Internal {
                message: format!("Failed to execute ip link delete: {}", e),
            })?;

        if !status.success() {
            tracing::warn!("ip link delete failed: {}", status);
        }

        Ok(())
    }
}
