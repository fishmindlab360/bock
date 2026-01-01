//! IP Address Management (IPAM) for container networking.
//!
//! Provides simple IP allocation from a subnet.

use bock_common::BockResult;
use std::collections::HashSet;
use std::net::Ipv4Addr;
use std::path::Path;

/// IP address allocator for a subnet.
#[derive(Debug)]
pub struct IpAllocator {
    /// Base network address (e.g., 172.17.0.0)
    base: u32,
    /// Network mask bits (e.g., 16 for /16)
    mask_bits: u8,
    /// Set of allocated IPs (as host part only)
    allocated: HashSet<u32>,
    /// Path to persist allocations
    state_path: Option<String>,
}

impl IpAllocator {
    /// Create a new IP allocator for a subnet.
    ///
    /// # Arguments
    /// * `subnet` - CIDR notation (e.g., "172.17.0.0/16")
    pub fn new(subnet: &str) -> BockResult<Self> {
        let parts: Vec<&str> = subnet.split('/').collect();
        if parts.len() != 2 {
            return Err(bock_common::BockError::Config {
                message: format!("Invalid subnet format: {}", subnet),
            });
        }

        let ip: Ipv4Addr = parts[0]
            .parse()
            .map_err(|_| bock_common::BockError::Config {
                message: format!("Invalid IP address: {}", parts[0]),
            })?;

        let mask_bits: u8 = parts[1]
            .parse()
            .map_err(|_| bock_common::BockError::Config {
                message: format!("Invalid mask bits: {}", parts[1]),
            })?;

        let base = u32::from(ip);

        Ok(Self {
            base,
            mask_bits,
            allocated: HashSet::new(),
            state_path: None,
        })
    }

    /// Set the state persistence path.
    pub fn with_state_path(mut self, path: impl Into<String>) -> Self {
        self.state_path = Some(path.into());
        self
    }

    /// Load state from disk if available.
    pub fn load_state(&mut self) -> BockResult<()> {
        if let Some(path) = &self.state_path {
            if Path::new(path).exists() {
                let content = std::fs::read_to_string(path)?;
                for line in content.lines() {
                    if let Ok(host_part) = line.parse::<u32>() {
                        self.allocated.insert(host_part);
                    }
                }
            }
        }
        Ok(())
    }

    /// Save state to disk.
    fn save_state(&self) -> BockResult<()> {
        if let Some(path) = &self.state_path {
            let content: String = self
                .allocated
                .iter()
                .map(|h| h.to_string())
                .collect::<Vec<_>>()
                .join("\n");
            std::fs::write(path, content)?;
        }
        Ok(())
    }

    /// Allocate an IP address from the pool.
    ///
    /// Returns the full IP in CIDR format (e.g., "172.17.0.2/16").
    pub fn allocate(&mut self) -> BockResult<String> {
        let mask = !((1u32 << (32 - self.mask_bits)) - 1);
        let network = self.base & mask;
        let max_host = (1u32 << (32 - self.mask_bits)) - 1;

        // Start from .2 (skip .0 network, .1 gateway)
        for host_part in 2..max_host {
            if !self.allocated.contains(&host_part) {
                self.allocated.insert(host_part);
                let _ = self.save_state();

                let ip = Ipv4Addr::from(network | host_part);
                return Ok(format!("{}/{}", ip, self.mask_bits));
            }
        }

        Err(bock_common::BockError::Internal {
            message: "No available IP addresses in pool".to_string(),
        })
    }

    /// Release an IP address back to the pool.
    pub fn release(&mut self, ip_cidr: &str) -> BockResult<()> {
        let ip_str = ip_cidr.split('/').next().unwrap_or(ip_cidr);
        let ip: Ipv4Addr = ip_str.parse().map_err(|_| bock_common::BockError::Config {
            message: format!("Invalid IP address: {}", ip_str),
        })?;

        let mask = !((1u32 << (32 - self.mask_bits)) - 1);
        let host_part = u32::from(ip) & !mask;

        self.allocated.remove(&host_part);
        let _ = self.save_state();

        Ok(())
    }

    /// Get the gateway IP (first usable address).
    pub fn gateway(&self) -> String {
        let mask = !((1u32 << (32 - self.mask_bits)) - 1);
        let network = self.base & mask;
        let gateway_ip = Ipv4Addr::from(network | 1);
        gateway_ip.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_allocate() {
        let mut alloc = IpAllocator::new("172.17.0.0/16").unwrap();

        let ip1 = alloc.allocate().unwrap();
        assert_eq!(ip1, "172.17.0.2/16");

        let ip2 = alloc.allocate().unwrap();
        assert_eq!(ip2, "172.17.0.3/16");
    }

    #[test]
    fn test_release() {
        let mut alloc = IpAllocator::new("172.17.0.0/16").unwrap();

        let ip1 = alloc.allocate().unwrap();
        alloc.release(&ip1).unwrap();

        let ip2 = alloc.allocate().unwrap();
        assert_eq!(ip1, ip2); // Should get same IP back
    }

    #[test]
    fn test_gateway() {
        let alloc = IpAllocator::new("172.17.0.0/16").unwrap();
        assert_eq!(alloc.gateway(), "172.17.0.1");
    }
}
