//! Container networking.

pub mod bridge;
pub mod veth;

/// Network configuration for a container.
#[derive(Debug, Clone)]
pub struct NetworkConfig {
    /// Network mode.
    pub mode: NetworkMode,
    /// Port mappings (host:container).
    pub ports: Vec<PortMapping>,
    /// DNS servers.
    pub dns: Vec<String>,
    /// Hostname.
    pub hostname: Option<String>,
}

/// Network mode.
#[derive(Debug, Clone)]
pub enum NetworkMode {
    /// Bridge network (default).
    Bridge,
    /// Host network (share host network namespace).
    Host,
    /// None (isolated, no networking).
    None,
    /// Share with another container.
    Container(String),
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            mode: NetworkMode::Bridge,
            ports: Vec::new(),
            dns: vec!["8.8.8.8".to_string()],
            hostname: None,
        }
    }
}

/// Port mapping configuration.
#[derive(Debug, Clone)]
pub struct PortMapping {
    /// Host port.
    pub host_port: u16,
    /// Container port.
    pub container_port: u16,
    /// Protocol (tcp/udp).
    pub protocol: String,
    /// Host IP to bind to.
    pub host_ip: Option<String>,
}
