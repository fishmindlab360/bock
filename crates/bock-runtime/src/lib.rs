//! # bock-runtime
//!
//! Spec-driven container image builder for Bock.
//!
//! Bock Runtime provides a modern alternative to Dockerfiles with:
//! - YAML-based Bockfile specification
//! - Multi-stage builds with parallelism
//! - Smart layer caching
//! - Security defaults

#![warn(missing_docs)]

pub mod bockfile;
pub mod build;
pub mod cache;
pub mod cli;
pub mod registry;

pub use bockfile::Bockfile;
pub use build::{BuildOptions, Builder, BuiltImage};
pub use cache::{CacheInfo, CacheManager};
pub use registry::{ImageInfo, ImageManifest, Registry, RegistryAuth};
