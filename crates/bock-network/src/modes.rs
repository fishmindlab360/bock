//! Network modes: Macvlan and IPvlan.
//!
//! This module provides support for advanced network modes that allow
//! containers to directly connect to the physical network.

use std::process::Command;

use bock_common::BockResult;

/// Network driver type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkDriver {
    /// Bridge networking (default).
    Bridge,
    /// Host networking (no isolation).
    Host,
    /// None (no networking).
    None,
    /// Macvlan (MAC-based VLANs).
    Macvlan,
    /// IPvlan (IP-based VLANs).
    Ipvlan,
}

impl Default for NetworkDriver {
    fn default() -> Self {
        Self::Bridge
    }
}

/// Macvlan mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MacvlanMode {
    /// Bridge mode (containers can communicate).
    Bridge,
    /// Private mode (isolated from each other).
    Private,
    /// VEPA mode (requires external switch).
    Vepa,
    /// Passthrough mode (single container per interface).
    Passthru,
}

impl MacvlanMode {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Bridge => "bridge",
            Self::Private => "private",
            Self::Vepa => "vepa",
            Self::Passthru => "passthru",
        }
    }
}

impl Default for MacvlanMode {
    fn default() -> Self {
        Self::Bridge
    }
}

/// IPvlan mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpvlanMode {
    /// L2 mode (operates at layer 2).
    L2,
    /// L3 mode (operates at layer 3).
    L3,
    /// L3s mode (L3 with source filtering).
    L3S,
}

impl IpvlanMode {
    fn as_str(&self) -> &'static str {
        match self {
            Self::L2 => "l2",
            Self::L3 => "l3",
            Self::L3S => "l3s",
        }
    }
}

impl Default for IpvlanMode {
    fn default() -> Self {
        Self::L2
    }
}

/// Create a macvlan interface.
#[cfg(target_os = "linux")]
pub fn create_macvlan(parent: &str, name: &str, mode: MacvlanMode) -> BockResult<()> {
    let output = Command::new("ip")
        .args([
            "link",
            "add",
            name,
            "link",
            parent,
            "type",
            "macvlan",
            "mode",
            mode.as_str(),
        ])
        .output()
        .map_err(|e| bock_common::BockError::Internal {
            message: format!("Failed to run ip command: {}", e),
        })?;

    if !output.status.success() {
        return Err(bock_common::BockError::Internal {
            message: format!(
                "Failed to create macvlan: {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        });
    }

    tracing::info!(parent, name, mode = ?mode, "Macvlan interface created");
    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub fn create_macvlan(_parent: &str, _name: &str, _mode: MacvlanMode) -> BockResult<()> {
    Err(bock_common::BockError::Unsupported {
        feature: "macvlan".to_string(),
    })
}

/// Create an ipvlan interface.
#[cfg(target_os = "linux")]
pub fn create_ipvlan(parent: &str, name: &str, mode: IpvlanMode) -> BockResult<()> {
    let output = Command::new("ip")
        .args([
            "link",
            "add",
            name,
            "link",
            parent,
            "type",
            "ipvlan",
            "mode",
            mode.as_str(),
        ])
        .output()
        .map_err(|e| bock_common::BockError::Internal {
            message: format!("Failed to run ip command: {}", e),
        })?;

    if !output.status.success() {
        return Err(bock_common::BockError::Internal {
            message: format!(
                "Failed to create ipvlan: {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        });
    }

    tracing::info!(parent, name, mode = ?mode, "IPvlan interface created");
    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub fn create_ipvlan(_parent: &str, _name: &str, _mode: IpvlanMode) -> BockResult<()> {
    Err(bock_common::BockError::Unsupported {
        feature: "ipvlan".to_string(),
    })
}

/// Move interface to network namespace.
#[cfg(target_os = "linux")]
pub fn move_to_netns(interface: &str, netns: &str) -> BockResult<()> {
    let output = Command::new("ip")
        .args(["link", "set", interface, "netns", netns])
        .output()
        .map_err(|e| bock_common::BockError::Internal {
            message: format!("Failed to run ip command: {}", e),
        })?;

    if !output.status.success() {
        return Err(bock_common::BockError::Internal {
            message: format!(
                "Failed to move interface to netns: {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        });
    }

    tracing::debug!(interface, netns, "Interface moved to netns");
    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub fn move_to_netns(_interface: &str, _netns: &str) -> BockResult<()> {
    Err(bock_common::BockError::Unsupported {
        feature: "network namespaces".to_string(),
    })
}

/// Delete a network interface.
#[cfg(target_os = "linux")]
pub fn delete_interface(name: &str) -> BockResult<()> {
    let output = Command::new("ip")
        .args(["link", "delete", name])
        .output()
        .map_err(|e| bock_common::BockError::Internal {
            message: format!("Failed to run ip command: {}", e),
        })?;

    if !output.status.success() && !String::from_utf8_lossy(&output.stderr).contains("not exist") {
        return Err(bock_common::BockError::Internal {
            message: format!(
                "Failed to delete interface: {}",
                String::from_utf8_lossy(&output.stderr)
            ),
        });
    }

    tracing::debug!(name, "Interface deleted");
    Ok(())
}

#[cfg(not(target_os = "linux"))]
pub fn delete_interface(_name: &str) -> BockResult<()> {
    Err(bock_common::BockError::Unsupported {
        feature: "network interfaces".to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_macvlan_mode() {
        assert_eq!(MacvlanMode::Bridge.as_str(), "bridge");
        assert_eq!(MacvlanMode::Private.as_str(), "private");
    }

    #[test]
    fn test_ipvlan_mode() {
        assert_eq!(IpvlanMode::L2.as_str(), "l2");
        assert_eq!(IpvlanMode::L3.as_str(), "l3");
    }

    #[test]
    fn test_network_driver_default() {
        assert_eq!(NetworkDriver::default(), NetworkDriver::Bridge);
    }
}
