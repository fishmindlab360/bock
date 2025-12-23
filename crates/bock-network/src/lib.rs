//! # bock-network
//!
//! Networking primitives for Bock containers.
//!
//! This crate provides shared networking utilities used by both
//! the container runtime and orchestrator.

#![warn(missing_docs)]

pub mod bridge;
pub mod netns;
pub mod veth;

pub use bridge::BridgeManager;
pub use veth::VethPair;
