//! # bock-oci
//!
//! OCI (Open Container Initiative) specification types for Bock.
//!
//! This crate provides Rust types for:
//! - OCI Runtime Specification (config.json)
//! - OCI Image Specification (manifests, configs)
//! - Container state management

#![warn(missing_docs)]

pub mod image;
pub mod runtime;
pub mod state;

pub use runtime::Spec;
pub use state::ContainerState;
