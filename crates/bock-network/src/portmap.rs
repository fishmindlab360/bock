//! Port mapping and forwarding for containers.
//!
//! This module provides utilities for setting up port forwarding between
//! the host and container using iptables NAT rules.

use std::process::Command;

use bock_common::BockResult;

/// Protocol for port mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    /// TCP protocol.
    Tcp,
    /// UDP protocol.
    Udp,
}

impl Protocol {
    /// Get the protocol string for iptables.
    fn as_str(&self) -> &'static str {
        match self {
            Protocol::Tcp => "tcp",
            Protocol::Udp => "udp",
        }
    }
}

impl std::fmt::Display for Protocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// A port mapping between host and container.
#[derive(Debug, Clone)]
pub struct PortMapping {
    /// Host port.
    pub host_port: u16,
    /// Container port.
    pub container_port: u16,
    /// Container IP address.
    pub container_ip: String,
    /// Protocol (TCP or UDP).
    pub protocol: Protocol,
    /// Host IP to bind to (optional, defaults to 0.0.0.0).
    pub host_ip: Option<String>,
}

impl PortMapping {
    /// Create a new TCP port mapping.
    pub fn tcp(host_port: u16, container_port: u16, container_ip: &str) -> Self {
        Self {
            host_port,
            container_port,
            container_ip: container_ip.to_string(),
            protocol: Protocol::Tcp,
            host_ip: None,
        }
    }

    /// Create a new UDP port mapping.
    pub fn udp(host_port: u16, container_port: u16, container_ip: &str) -> Self {
        Self {
            host_port,
            container_port,
            container_ip: container_ip.to_string(),
            protocol: Protocol::Udp,
            host_ip: None,
        }
    }

    /// Set the host IP to bind to.
    pub fn with_host_ip(mut self, ip: &str) -> Self {
        self.host_ip = Some(ip.to_string());
        self
    }
}

/// Port mapper for managing iptables NAT rules.
pub struct PortMapper {
    /// Container ID for rule comments.
    container_id: String,
    /// Active port mappings.
    mappings: Vec<PortMapping>,
}

impl PortMapper {
    /// Create a new port mapper for a container.
    pub fn new(container_id: &str) -> Self {
        Self {
            container_id: container_id.to_string(),
            mappings: Vec::new(),
        }
    }

    /// Add a port mapping.
    pub fn add_mapping(&mut self, mapping: PortMapping) -> BockResult<()> {
        tracing::debug!(
            host_port = mapping.host_port,
            container_port = mapping.container_port,
            container_ip = %mapping.container_ip,
            protocol = %mapping.protocol,
            "Adding port mapping"
        );

        // Pre-compute strings that we need references to
        let host_port_str = mapping.host_port.to_string();
        let container_port_str = mapping.container_port.to_string();
        let dest_str = format!("{}:{}", mapping.container_ip, mapping.container_port);
        let comment = format!("bock-{}", self.container_id);

        // PREROUTING DNAT rule for external traffic
        let mut args: Vec<&str> = vec![
            "-t",
            "nat",
            "-A",
            "PREROUTING",
            "-p",
            mapping.protocol.as_str(),
            "--dport",
            &host_port_str,
            "-j",
            "DNAT",
            "--to-destination",
            &dest_str,
            "-m",
            "comment",
            "--comment",
            &comment,
        ];

        if let Some(ref host_ip) = mapping.host_ip {
            args.insert(4, "-d");
            args.insert(5, host_ip);
        }

        run_iptables(&args)?;

        // OUTPUT DNAT rule for localhost traffic
        run_iptables(&[
            "-t",
            "nat",
            "-A",
            "OUTPUT",
            "-p",
            mapping.protocol.as_str(),
            "-d",
            "127.0.0.1",
            "--dport",
            &host_port_str,
            "-j",
            "DNAT",
            "--to-destination",
            &dest_str,
            "-m",
            "comment",
            "--comment",
            &comment,
        ])?;

        // MASQUERADE rule for return traffic
        run_iptables(&[
            "-t",
            "nat",
            "-A",
            "POSTROUTING",
            "-p",
            mapping.protocol.as_str(),
            "-d",
            &mapping.container_ip,
            "--dport",
            &container_port_str,
            "-j",
            "MASQUERADE",
            "-m",
            "comment",
            "--comment",
            &comment,
        ])?;

        self.mappings.push(mapping);
        Ok(())
    }

    /// Remove a specific port mapping.
    pub fn remove_mapping(&mut self, host_port: u16, protocol: Protocol) -> BockResult<()> {
        let idx = self
            .mappings
            .iter()
            .position(|m| m.host_port == host_port && m.protocol == protocol);

        if let Some(idx) = idx {
            let mapping = self.mappings.remove(idx);
            self.remove_iptables_rules(&mapping)?;
        }

        Ok(())
    }

    /// Remove all port mappings for this container.
    pub fn remove_all(&mut self) -> BockResult<()> {
        let mappings: Vec<_> = self.mappings.drain(..).collect();
        for mapping in mappings {
            if let Err(e) = self.remove_iptables_rules(&mapping) {
                tracing::warn!(error = %e, "Failed to remove port mapping");
            }
        }
        Ok(())
    }

    /// Remove iptables rules for a mapping.
    fn remove_iptables_rules(&self, mapping: &PortMapping) -> BockResult<()> {
        tracing::debug!(
            host_port = mapping.host_port,
            container_port = mapping.container_port,
            "Removing port mapping"
        );

        // Pre-compute strings
        let host_port_str = mapping.host_port.to_string();
        let container_port_str = mapping.container_port.to_string();
        let dest_str = format!("{}:{}", mapping.container_ip, mapping.container_port);
        let comment = format!("bock-{}", self.container_id);

        // Remove PREROUTING DNAT rule
        let _ = run_iptables(&[
            "-t",
            "nat",
            "-D",
            "PREROUTING",
            "-p",
            mapping.protocol.as_str(),
            "--dport",
            &host_port_str,
            "-j",
            "DNAT",
            "--to-destination",
            &dest_str,
            "-m",
            "comment",
            "--comment",
            &comment,
        ]);

        // Remove OUTPUT DNAT rule
        let _ = run_iptables(&[
            "-t",
            "nat",
            "-D",
            "OUTPUT",
            "-p",
            mapping.protocol.as_str(),
            "-d",
            "127.0.0.1",
            "--dport",
            &host_port_str,
            "-j",
            "DNAT",
            "--to-destination",
            &dest_str,
            "-m",
            "comment",
            "--comment",
            &comment,
        ]);

        // Remove MASQUERADE rule
        let _ = run_iptables(&[
            "-t",
            "nat",
            "-D",
            "POSTROUTING",
            "-p",
            mapping.protocol.as_str(),
            "-d",
            &mapping.container_ip,
            "--dport",
            &container_port_str,
            "-j",
            "MASQUERADE",
            "-m",
            "comment",
            "--comment",
            &comment,
        ]);

        Ok(())
    }

    /// Get active mappings.
    pub fn mappings(&self) -> &[PortMapping] {
        &self.mappings
    }
}

impl Drop for PortMapper {
    fn drop(&mut self) {
        if let Err(e) = self.remove_all() {
            tracing::warn!(error = %e, "Failed to cleanup port mappings on drop");
        }
    }
}

/// Run an iptables command.
fn run_iptables(args: &[&str]) -> BockResult<()> {
    let status = Command::new("iptables").args(args).status().map_err(|e| {
        bock_common::BockError::Internal {
            message: format!("Failed to execute iptables: {}", e),
        }
    })?;

    if !status.success() {
        return Err(bock_common::BockError::Internal {
            message: format!("iptables command failed: {:?}", args),
        });
    }

    Ok(())
}

/// Enable IP forwarding on the system.
pub fn enable_ip_forwarding() -> BockResult<()> {
    std::fs::write("/proc/sys/net/ipv4/ip_forward", "1").map_err(|e| {
        bock_common::BockError::Internal {
            message: format!("Failed to enable IP forwarding: {}", e),
        }
    })?;

    tracing::info!("IP forwarding enabled");
    Ok(())
}

/// Setup default FORWARD rules for container networking.
pub fn setup_forward_rules(bridge_interface: &str) -> BockResult<()> {
    run_iptables(&["-A", "FORWARD", "-i", bridge_interface, "-j", "ACCEPT"])?;
    run_iptables(&["-A", "FORWARD", "-o", bridge_interface, "-j", "ACCEPT"])?;

    tracing::info!(bridge = bridge_interface, "Forward rules configured");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_port_mapping_tcp() {
        let mapping = PortMapping::tcp(8080, 80, "172.17.0.2");
        assert_eq!(mapping.host_port, 8080);
        assert_eq!(mapping.container_port, 80);
        assert_eq!(mapping.container_ip, "172.17.0.2");
        assert_eq!(mapping.protocol, Protocol::Tcp);
    }

    #[test]
    fn test_port_mapping_udp() {
        let mapping = PortMapping::udp(5353, 53, "172.17.0.2");
        assert_eq!(mapping.protocol, Protocol::Udp);
    }

    #[test]
    fn test_port_mapping_with_host_ip() {
        let mapping = PortMapping::tcp(8080, 80, "172.17.0.2").with_host_ip("192.168.1.100");
        assert_eq!(mapping.host_ip, Some("192.168.1.100".to_string()));
    }

    #[test]
    fn test_protocol_display() {
        assert_eq!(format!("{}", Protocol::Tcp), "tcp");
        assert_eq!(format!("{}", Protocol::Udp), "udp");
    }
}
