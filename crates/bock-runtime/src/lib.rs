//! # bock-runtime
//!
//! Spec-driven container image builder for Bock.
//!
//! Bock Runtime provides a modern alternative to Dockerfiles with:
//! - YAML/TOML/JSON-based Bockfile specification (v2)
//! - Multi-stage builds with parallelism
//! - Smart layer caching
//! - Per-stage security configuration
//! - Dynamic tag templates
//! - Registry integration

#![warn(missing_docs)]

pub mod bockfile;
/// Bockfile v2 - Modern cleaner format.
pub mod bockfile_v2;
pub mod build;
pub mod cache;
pub mod cli;
pub mod registry;

pub use bockfile::Bockfile;
pub use bockfile_v2::Bockfile as BockfileV2;
pub use build::{BuildOptions, Builder, BuiltImage};
pub use cache::{CacheInfo, CacheManager};
pub use registry::{ImageInfo, ImageManifest, Registry, RegistryAuth};
