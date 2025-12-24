//! # bock-network
//!
//! Networking primitives for Bock containers.
//!
//! This crate provides shared networking utilities used by both
//! the container runtime and orchestrator.

#![warn(missing_docs)]

pub mod bridge;
pub mod dns;
pub mod ipv6;
pub mod modes;
pub mod netns;
pub mod policy;
pub mod portmap;
pub mod veth;

pub use bridge::BridgeManager;
pub use dns::{ContainerDns, DnsRecord};
pub use ipv6::{Ipv6Config, configure_interface_ipv6, enable_ipv6_forwarding};
pub use modes::{IpvlanMode, MacvlanMode, NetworkDriver, create_ipvlan, create_macvlan};
pub use netns::{
    create_netns, delete_netns, enter_netns, enter_netns_by_pid, list_netns, netns_exists,
};
pub use policy::{NetworkPolicy, PolicyAction, PolicyRule};
pub use portmap::{PortMapper, PortMapping, Protocol, enable_ip_forwarding, setup_forward_rules};
pub use veth::VethPair;
