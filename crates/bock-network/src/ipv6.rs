//! IPv6 support for container networking.
//!
//! This module provides IPv6 address management and configuration
//! for container networks.

use std::net::Ipv6Addr;
use std::process::Command;

use bock_common::BockResult;

/// IPv6 network configuration.
#[derive(Debug, Clone)]
pub struct Ipv6Config {
    /// Enable IPv6.
    pub enabled: bool,
    /// IPv6 subnet (CIDR).
    pub subnet: Option<String>,
    /// Gateway address.
    pub gateway: Option<Ipv6Addr>,
}

impl Default for Ipv6Config {
    fn default() -> Self {
        Self {
            enabled: false,
            subnet: None,
            gateway: None,
        }
    }
}

impl Ipv6Config {
    /// Create an enabled IPv6 config.
    pub fn enabled() -> Self {
        Self {
            enabled: true,
            subnet: Some("fd00::/64".to_string()),
            gateway: Some(Ipv6Addr::new(0xfd00, 0, 0, 0, 0, 0, 0, 1)),
        }
    }

    /// Create with custom subnet.
    pub fn with_subnet(subnet: &str, gateway: Ipv6Addr) -> Self {
        Self {
            enabled: true,
            subnet: Some(subnet.to_string()),
            gateway: Some(gateway),
        }
    }
}

/// Configure IPv6 for a network interface.
#[cfg(target_os = "linux")]
pub fn configure_interface_ipv6(interface: &str, address: &str) -> BockResult<()> {
    let output = Command::new("ip")
        .args(["-6", "addr", "add", address, "dev", interface])
        .output()
        .map_err(|e| bock_common::BockError::Internal {
            message: format!("Failed to run ip command: {}", e),
        })?;

    if !output.status.success() {
        return Err(bock_common::BockError::Internal {
            message: format!(
                "Failed to configure IPv6: {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        });
    }

    tracing::debug!(interface, address, "IPv6 address configured");
    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub fn configure_interface_ipv6(_interface: &str, _address: &str) -> BockResult<()> {
    Err(bock_common::BockError::Unsupported {
        feature: "IPv6".to_string(),
    })
}

/// Enable IPv6 forwarding.
#[cfg(target_os = "linux")]
pub fn enable_ipv6_forwarding() -> BockResult<()> {
    std::fs::write("/proc/sys/net/ipv6/conf/all/forwarding", "1").map_err(|e| {
        bock_common::BockError::Internal {
            message: format!("Failed to enable IPv6 forwarding: {}", e),
        }
    })?;

    tracing::debug!("IPv6 forwarding enabled");
    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub fn enable_ipv6_forwarding() -> BockResult<()> {
    Err(bock_common::BockError::Unsupported {
        feature: "IPv6 forwarding".to_string(),
    })
}

/// Add IPv6 route.
#[cfg(target_os = "linux")]
pub fn add_ipv6_route(destination: &str, gateway: &str) -> BockResult<()> {
    let output = Command::new("ip")
        .args(["-6", "route", "add", destination, "via", gateway])
        .output()
        .map_err(|e| bock_common::BockError::Internal {
            message: format!("Failed to run ip command: {}", e),
        })?;

    if !output.status.success() && !String::from_utf8_lossy(&output.stderr).contains("exists") {
        return Err(bock_common::BockError::Internal {
            message: format!(
                "Failed to add IPv6 route: {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        });
    }

    tracing::debug!(destination, gateway, "IPv6 route added");
    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub fn add_ipv6_route(_destination: &str, _gateway: &str) -> BockResult<()> {
    Err(bock_common::BockError::Unsupported {
        feature: "IPv6 routing".to_string(),
    })
}

/// Generate unique IPv6 address from container ID.
pub fn generate_container_ipv6(prefix: &str, container_id: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    container_id.hash(&mut hasher);
    let hash = hasher.finish();

    format!("{}::{:x}/64", prefix.trim_end_matches("/64"), hash & 0xffff)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ipv6_config_default() {
        let config = Ipv6Config::default();
        assert!(!config.enabled);
        assert!(config.subnet.is_none());
    }

    #[test]
    fn test_ipv6_config_enabled() {
        let config = Ipv6Config::enabled();
        assert!(config.enabled);
        assert!(config.subnet.is_some());
        assert!(config.gateway.is_some());
    }

    #[test]
    fn test_generate_container_ipv6() {
        let addr = generate_container_ipv6("fd00::", "test-container");
        assert!(addr.starts_with("fd00::"));
        assert!(addr.ends_with("/64"));
    }
}
