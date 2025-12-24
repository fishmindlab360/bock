//! DNS server for container name resolution.
//!
//! This module provides a simple DNS server that resolves container names
//! to their IP addresses for inter-container communication.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, RwLock};

use bock_common::BockResult;

/// DNS record for a container.
#[derive(Debug, Clone)]
pub struct DnsRecord {
    /// Container name.
    pub name: String,
    /// IPv4 address.
    pub ipv4: Option<Ipv4Addr>,
    /// IPv6 address.
    pub ipv6: Option<std::net::Ipv6Addr>,
    /// TTL in seconds.
    pub ttl: u32,
}

/// DNS resolver for containers.
pub struct ContainerDns {
    /// DNS records by name.
    records: Arc<RwLock<HashMap<String, DnsRecord>>>,
    /// Listen address.
    listen_addr: SocketAddr,
}

impl ContainerDns {
    /// Create a new DNS resolver.
    pub fn new(listen_addr: SocketAddr) -> Self {
        Self {
            records: Arc::new(RwLock::new(HashMap::new())),
            listen_addr,
        }
    }

    /// Default DNS resolver (listens on 127.0.0.1:53).
    pub fn default() -> Self {
        Self::new(SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 53))
    }

    /// Add a DNS record.
    pub fn add_record(&self, record: DnsRecord) -> BockResult<()> {
        let mut records = self
            .records
            .write()
            .map_err(|_| bock_common::BockError::Internal {
                message: "Failed to acquire write lock".to_string(),
            })?;

        tracing::debug!(
            name = %record.name,
            ipv4 = ?record.ipv4,
            "DNS record added"
        );

        records.insert(record.name.clone(), record);
        Ok(())
    }

    /// Remove a DNS record.
    pub fn remove_record(&self, name: &str) -> BockResult<()> {
        let mut records = self
            .records
            .write()
            .map_err(|_| bock_common::BockError::Internal {
                message: "Failed to acquire write lock".to_string(),
            })?;

        records.remove(name);
        tracing::debug!(name, "DNS record removed");
        Ok(())
    }

    /// Resolve a name to an IP address.
    pub fn resolve(&self, name: &str) -> Option<IpAddr> {
        let records = self.records.read().ok()?;

        records
            .get(name)
            .and_then(|r| r.ipv4.map(IpAddr::V4).or_else(|| r.ipv6.map(IpAddr::V6)))
    }

    /// Resolve IPv4 address.
    pub fn resolve_ipv4(&self, name: &str) -> Option<Ipv4Addr> {
        let records = self.records.read().ok()?;
        records.get(name).and_then(|r| r.ipv4)
    }

    /// Get the listen address.
    pub fn listen_addr(&self) -> SocketAddr {
        self.listen_addr
    }

    /// Get all records.
    pub fn get_records(&self) -> Vec<DnsRecord> {
        self.records
            .read()
            .ok()
            .map(|r| r.values().cloned().collect())
            .unwrap_or_default()
    }

    /// Generate /etc/hosts content for a container.
    pub fn generate_hosts(&self, container_name: &str, container_ip: Ipv4Addr) -> String {
        let mut hosts = String::new();

        // Standard entries
        hosts.push_str("127.0.0.1 localhost\n");
        hosts.push_str("::1 localhost ip6-localhost ip6-loopback\n");

        // Container's own entry
        hosts.push_str(&format!("{} {}\n", container_ip, container_name));

        // Other containers
        if let Ok(records) = self.records.read() {
            for record in records.values() {
                if record.name != container_name {
                    if let Some(ip) = record.ipv4 {
                        hosts.push_str(&format!("{} {}\n", ip, record.name));
                    }
                }
            }
        }

        hosts
    }

    /// Generate resolv.conf content.
    pub fn generate_resolv_conf(&self, upstream_dns: &[Ipv4Addr]) -> String {
        let mut content = String::new();

        // Add local DNS first
        content.push_str(&format!("nameserver {}\n", self.listen_addr.ip()));

        // Add upstream DNS servers
        for dns in upstream_dns {
            content.push_str(&format!("nameserver {}\n", dns));
        }

        content
    }
}

impl Default for ContainerDns {
    fn default() -> Self {
        Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dns_record_add_remove() {
        let dns = ContainerDns::default();

        let record = DnsRecord {
            name: "web".to_string(),
            ipv4: Some(Ipv4Addr::new(172, 17, 0, 2)),
            ipv6: None,
            ttl: 300,
        };

        dns.add_record(record).unwrap();
        assert!(dns.resolve("web").is_some());

        dns.remove_record("web").unwrap();
        assert!(dns.resolve("web").is_none());
    }

    #[test]
    fn test_generate_hosts() {
        let dns = ContainerDns::default();

        let record = DnsRecord {
            name: "db".to_string(),
            ipv4: Some(Ipv4Addr::new(172, 17, 0, 3)),
            ipv6: None,
            ttl: 300,
        };
        dns.add_record(record).unwrap();

        let hosts = dns.generate_hosts("web", Ipv4Addr::new(172, 17, 0, 2));
        assert!(hosts.contains("172.17.0.2 web"));
        assert!(hosts.contains("172.17.0.3 db"));
    }
}
