//! Container lifecycle management.

use async_trait::async_trait;
use bock_common::BockResult;

/// Container lifecycle trait.
///
/// Defines the operations for managing container lifecycle.
#[async_trait]
pub trait ContainerLifecycle {
    /// Create the container.
    async fn create(&mut self) -> BockResult<()>;

    /// Start the container.
    async fn start(&mut self) -> BockResult<()>;

    /// Stop the container.
    async fn stop(&mut self, timeout: Option<u64>) -> BockResult<()>;

    /// Kill the container with a signal.
    async fn kill(&mut self, signal: i32) -> BockResult<()>;

    /// Delete the container.
    async fn delete(&mut self) -> BockResult<()>;

    /// Pause the container.
    async fn pause(&mut self) -> BockResult<()>;

    /// Resume the container.
    async fn resume(&mut self) -> BockResult<()>;
}

/// Container lifecycle phases.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecyclePhase {
    /// Setting up namespaces.
    Namespaces,
    /// Setting up cgroups.
    Cgroups,
    /// Setting up root filesystem.
    Rootfs,
    /// Setting up mounts.
    Mounts,
    /// Setting up networking.
    Network,
    /// Setting up security.
    Security,
    /// Executing process.
    Process,
}

impl std::fmt::Display for LifecyclePhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Namespaces => write!(f, "namespaces"),
            Self::Cgroups => write!(f, "cgroups"),
            Self::Rootfs => write!(f, "rootfs"),
            Self::Mounts => write!(f, "mounts"),
            Self::Network => write!(f, "network"),
            Self::Security => write!(f, "security"),
            Self::Process => write!(f, "process"),
        }
    }
}
