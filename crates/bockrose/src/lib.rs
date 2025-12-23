//! # bockrose
//!
//! Multi-container orchestration for Bock - a Docker Compose alternative.
//!
//! bockrose provides:
//! - Declarative multi-container applications
//! - Service dependency management
//! - Health checks and auto-restart
//! - Network and volume orchestration

#![warn(missing_docs)]

pub mod cli;
pub mod health;
pub mod network;
pub mod orchestrator;
pub mod spec;
pub mod volume;

pub use orchestrator::Orchestrator;
pub use spec::BockoseSpec;
