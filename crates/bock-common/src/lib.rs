//! # bock-common
//!
//! Shared utilities and types for the Bock container ecosystem.
//!
//! This crate provides common functionality used across all Bock crates:
//! - Container and image ID generation
//! - Standard filesystem paths
//! - Resource quantity parsing
//! - Common error types

#![warn(missing_docs)]

pub mod error;
pub mod id;
pub mod paths;
pub mod resource;

pub use error::{BockError, BockResult};
pub use id::ContainerId;
pub use paths::BockPaths;
pub use resource::ResourceQuantity;
