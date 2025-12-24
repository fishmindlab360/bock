//! # bock-network
//!
//! Networking primitives for Bock containers.
//!
//! This crate provides shared networking utilities used by both
//! the container runtime and orchestrator.

#![warn(missing_docs)]

pub mod bridge;
pub mod netns;
pub mod portmap;
pub mod veth;

pub use bridge::BridgeManager;
pub use netns::{
    create_netns, delete_netns, enter_netns, enter_netns_by_pid, list_netns, netns_exists,
};
pub use portmap::{PortMapper, PortMapping, Protocol, enable_ip_forwarding, setup_forward_rules};
pub use veth::VethPair;
