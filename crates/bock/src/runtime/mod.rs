//! Container runtime core.
//!
//! This module provides the main Container type and lifecycle management.

mod config;
mod container;
mod lifecycle;
mod state;

pub use config::RuntimeConfig;
pub use container::Container;
pub use lifecycle::ContainerLifecycle;
pub use state::StateManager;
