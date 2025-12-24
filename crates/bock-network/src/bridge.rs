//! Linux bridge management.
//!
//! This module provides utilities for creating and managing Linux network bridges.

use std::process::Command;

use bock_common::BockResult;

/// Bridge manager for container networking.
pub struct BridgeManager {
    /// Bridge name.
    name: String,
}

impl BridgeManager {
    /// Create a new network bridge.
    pub async fn create(name: &str) -> BockResult<Self> {
        tracing::debug!(name, "Creating bridge");

        // Create bridge: ip link add name <bridge> type bridge
        let status = Command::new("ip")
            .args(["link", "add", "name", name, "type", "bridge"])
            .status()
            .map_err(|e| bock_common::BockError::Internal {
                message: format!("Failed to execute ip link add (bridge): {}", e),
            })?;

        if !status.success() {
            // Bridge might already exist, check before failing
            if !Self::exists(name) {
                return Err(bock_common::BockError::Internal {
                    message: format!("Failed to create bridge '{}': ip command failed", name),
                });
            }
        }

        // Bring the bridge up
        let bridge = Self {
            name: name.to_string(),
        };
        bridge.up().await?;

        tracing::info!(name, "Bridge created successfully");
        Ok(bridge)
    }

    /// Get an existing bridge.
    pub fn get(name: &str) -> BockResult<Self> {
        if !Self::exists(name) {
            return Err(bock_common::BockError::Config {
                message: format!("Bridge '{}' does not exist", name),
            });
        }
        Ok(Self {
            name: name.to_string(),
        })
    }

    /// Check if a bridge exists.
    pub fn exists(name: &str) -> bool {
        Command::new("ip")
            .args(["link", "show", name])
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Get the bridge name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Bring the bridge interface up.
    pub async fn up(&self) -> BockResult<()> {
        tracing::debug!(name = %self.name, "Bringing bridge up");

        let status = Command::new("ip")
            .args(["link", "set", &self.name, "up"])
            .status()
            .map_err(|e| bock_common::BockError::Internal {
                message: format!("Failed to execute ip link set up: {}", e),
            })?;

        if !status.success() {
            return Err(bock_common::BockError::Internal {
                message: format!("Failed to bring up bridge '{}'", self.name),
            });
        }

        Ok(())
    }

    /// Add an interface to the bridge.
    pub async fn add_interface(&self, interface: &str) -> BockResult<()> {
        tracing::debug!(bridge = %self.name, interface, "Adding interface to bridge");

        let status = Command::new("ip")
            .args(["link", "set", interface, "master", &self.name])
            .status()
            .map_err(|e| bock_common::BockError::Internal {
                message: format!("Failed to execute ip link set master: {}", e),
            })?;

        if !status.success() {
            return Err(bock_common::BockError::Internal {
                message: format!(
                    "Failed to add interface '{}' to bridge '{}'",
                    interface, self.name
                ),
            });
        }

        tracing::debug!(bridge = %self.name, interface, "Interface added successfully");
        Ok(())
    }

    /// Set IP address on the bridge.
    pub async fn set_ip(&self, ip_cidr: &str) -> BockResult<()> {
        tracing::debug!(bridge = %self.name, ip = ip_cidr, "Setting IP address");

        let status = Command::new("ip")
            .args(["addr", "add", ip_cidr, "dev", &self.name])
            .status()
            .map_err(|e| bock_common::BockError::Internal {
                message: format!("Failed to execute ip addr add: {}", e),
            })?;

        if !status.success() {
            // IP might already be assigned
            tracing::warn!(bridge = %self.name, ip = ip_cidr, "Failed to set IP (may already exist)");
        }

        Ok(())
    }

    /// Delete the bridge.
    pub async fn delete(&self) -> BockResult<()> {
        tracing::debug!(name = %self.name, "Deleting bridge");

        let status = Command::new("ip")
            .args(["link", "delete", &self.name])
            .status()
            .map_err(|e| bock_common::BockError::Internal {
                message: format!("Failed to execute ip link delete: {}", e),
            })?;

        if !status.success() {
            tracing::warn!(name = %self.name, "Failed to delete bridge (may not exist)");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bridge_exists_nonexistent() {
        // A non-existent bridge should return false
        assert!(!BridgeManager::exists("nonexistent_bridge_12345"));
    }
}
