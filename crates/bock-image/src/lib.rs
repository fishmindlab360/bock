//! # bock-image
//!
//! Container image management for Bock.
//!
//! This crate provides:
//! - Image pulling from registries
//! - Layer caching and deduplication
//! - Image storage and retrieval
//! - Manifest and config handling

#![warn(missing_docs)]

pub mod layer;
pub mod reference;
pub mod registry;
pub mod store;

pub use reference::ImageReference;
pub use store::ImageStore;
