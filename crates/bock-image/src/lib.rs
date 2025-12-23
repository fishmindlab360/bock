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
/// Image registry client.
pub mod registry;
/// Local image store.
pub mod store;

pub use reference::ImageReference;
pub use store::ImageStore;
