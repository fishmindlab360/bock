//! NAT (Network Address Translation) management for container networking.
//!
//! Provides iptables-based NAT setup for container internet access.

use bock_common::BockResult;
use std::process::Command;

/// Default bridge name for container networking.
pub const DEFAULT_BRIDGE: &str = "bock0";

/// Default subnet for container networking.
pub const DEFAULT_SUBNET: &str = "172.20.0.0/16";

/// Default gateway IP for the bridge.
pub const DEFAULT_GATEWAY: &str = "172.20.0.1/16";

/// NAT manager for container networking.
pub struct NatManager;

impl NatManager {
    /// Enable IP forwarding in the kernel.
    pub fn enable_ip_forward() -> BockResult<()> {
        tracing::debug!("Enabling IP forwarding");

        let status = Command::new("sysctl")
            .args(["-w", "net.ipv4.ip_forward=1"])
            .status()
            .map_err(|e| bock_common::BockError::Internal {
                message: format!("Failed to execute sysctl: {}", e),
            })?;

        if !status.success() {
            return Err(bock_common::BockError::Internal {
                message: "Failed to enable IP forwarding".to_string(),
            });
        }

        Ok(())
    }

    /// Setup MASQUERADE NAT rule for a subnet.
    ///
    /// This allows containers to access the internet through the host's network.
    pub fn setup_masquerade(subnet: &str) -> BockResult<()> {
        tracing::debug!(subnet, "Setting up MASQUERADE");

        // Check if rule already exists
        let check = Command::new("iptables")
            .args([
                "-t",
                "nat",
                "-C",
                "POSTROUTING",
                "-s",
                subnet,
                "-j",
                "MASQUERADE",
            ])
            .output()
            .map_err(|e| bock_common::BockError::Internal {
                message: format!("Failed to execute iptables: {}", e),
            })?;

        if check.status.success() {
            tracing::debug!(subnet, "MASQUERADE rule already exists");
            return Ok(());
        }

        // Add the rule
        let status = Command::new("iptables")
            .args([
                "-t",
                "nat",
                "-A",
                "POSTROUTING",
                "-s",
                subnet,
                "!",
                "-o",
                DEFAULT_BRIDGE,
                "-j",
                "MASQUERADE",
            ])
            .status()
            .map_err(|e| bock_common::BockError::Internal {
                message: format!("Failed to execute iptables: {}", e),
            })?;

        if !status.success() {
            return Err(bock_common::BockError::Internal {
                message: format!("Failed to add MASQUERADE rule for {}", subnet),
            });
        }

        tracing::info!(subnet, "MASQUERADE rule added");
        Ok(())
    }

    /// Setup FORWARD rules to allow traffic to/from containers.
    pub fn setup_forward_rules(bridge: &str) -> BockResult<()> {
        tracing::debug!(bridge, "Setting up FORWARD rules");

        // Allow traffic from bridge
        let _ = Command::new("iptables")
            .args(["-I", "FORWARD", "-i", bridge, "-j", "ACCEPT"])
            .status();

        // Allow traffic to bridge (established connections)
        let _ = Command::new("iptables")
            .args([
                "-I",
                "FORWARD",
                "-o",
                bridge,
                "-m",
                "conntrack",
                "--ctstate",
                "RELATED,ESTABLISHED",
                "-j",
                "ACCEPT",
            ])
            .status();

        // Allow traffic to bridge (new connections from bridge subnet)
        let _ = Command::new("iptables")
            .args(["-I", "FORWARD", "-o", bridge, "-j", "ACCEPT"])
            .status();

        Ok(())
    }

    /// Cleanup NAT rules for a subnet.
    pub fn cleanup(subnet: &str) -> BockResult<()> {
        tracing::debug!(subnet, "Cleaning up NAT rules");

        // Remove MASQUERADE rule (ignore errors if it doesn't exist)
        let _ = Command::new("iptables")
            .args([
                "-t",
                "nat",
                "-D",
                "POSTROUTING",
                "-s",
                subnet,
                "!",
                "-o",
                DEFAULT_BRIDGE,
                "-j",
                "MASQUERADE",
            ])
            .status();

        Ok(())
    }

    /// Full network setup: bridge + NAT.
    pub async fn setup_network() -> BockResult<()> {
        use crate::BridgeManager;

        tracing::info!("Setting up container network");

        // 1. Enable IP forwarding
        Self::enable_ip_forward()?;

        // 2. Create or get bridge
        let bridge = if BridgeManager::exists(DEFAULT_BRIDGE) {
            BridgeManager::get(DEFAULT_BRIDGE)?
        } else {
            BridgeManager::create(DEFAULT_BRIDGE).await?
        };

        // 3. Set bridge IP (gateway for containers)
        bridge.set_ip(DEFAULT_GATEWAY).await?;

        // 4. Setup NAT
        Self::setup_masquerade(DEFAULT_SUBNET)?;
        Self::setup_forward_rules(DEFAULT_BRIDGE)?;

        tracing::info!(
            bridge = DEFAULT_BRIDGE,
            subnet = DEFAULT_SUBNET,
            "Container network ready"
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constants() {
        assert_eq!(DEFAULT_BRIDGE, "bock0");
        assert!(DEFAULT_SUBNET.contains("172.17"));
    }
}
