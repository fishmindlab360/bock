//! Container runtime core.
//!
//! This module provides the main Container type and lifecycle management.

mod config;
mod container;
pub mod events;
mod lifecycle;
mod state;

pub use config::RuntimeConfig;
pub use container::{Container, ContainerStats, NetworkConfig};
pub use events::{EventBus, RuntimeEvent};
pub use lifecycle::ContainerLifecycle;
pub use state::StateManager;
