//! Container state management.
//!
//! Based on the OCI Runtime Specification state format:
//! <https://github.com/opencontainers/runtime-spec/blob/main/runtime.md#state>

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Container runtime state.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContainerState {
    /// OCI version.
    pub oci_version: String,
    /// Container ID.
    pub id: String,
    /// Container status.
    pub status: ContainerStatus,
    /// Process ID of the container init process.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    /// Path to the OCI bundle.
    pub bundle: PathBuf,
    /// Annotations.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub annotations: HashMap<String, String>,
}

/// Container status values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContainerStatus {
    /// Container is being created.
    Creating,
    /// Container has been created but not started.
    Created,
    /// Container is running.
    Running,
    /// Container has exited.
    Stopped,
    /// Container is paused.
    Paused,
}

impl ContainerStatus {
    /// Returns true if the container can be started.
    #[must_use]
    pub const fn can_start(&self) -> bool {
        matches!(self, Self::Created)
    }

    /// Returns true if the container can be killed.
    #[must_use]
    pub const fn can_kill(&self) -> bool {
        matches!(self, Self::Running | Self::Paused)
    }

    /// Returns true if the container can be deleted.
    #[must_use]
    pub const fn can_delete(&self) -> bool {
        matches!(self, Self::Stopped | Self::Created)
    }

    /// Returns true if the container can be paused.
    #[must_use]
    pub const fn can_pause(&self) -> bool {
        matches!(self, Self::Running)
    }

    /// Returns true if the container can be resumed.
    #[must_use]
    pub const fn can_resume(&self) -> bool {
        matches!(self, Self::Paused)
    }

    /// Returns true if the container is in a running state.
    #[must_use]
    pub const fn is_running(&self) -> bool {
        matches!(self, Self::Running)
    }

    /// Returns true if the container has exited.
    #[must_use]
    pub const fn is_stopped(&self) -> bool {
        matches!(self, Self::Stopped)
    }
}

impl std::fmt::Display for ContainerStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Creating => write!(f, "creating"),
            Self::Created => write!(f, "created"),
            Self::Running => write!(f, "running"),
            Self::Stopped => write!(f, "stopped"),
            Self::Paused => write!(f, "paused"),
        }
    }
}

impl ContainerState {
    /// Create a new container state in the "creating" status.
    #[must_use]
    pub fn new(id: impl Into<String>, bundle: impl Into<PathBuf>) -> Self {
        Self {
            oci_version: "1.2.0".to_string(),
            id: id.into(),
            status: ContainerStatus::Creating,
            pid: None,
            bundle: bundle.into(),
            annotations: HashMap::new(),
        }
    }

    /// Transition to the "created" status.
    pub fn set_created(&mut self, pid: u32) {
        self.status = ContainerStatus::Created;
        self.pid = Some(pid);
    }

    /// Transition to the "running" status.
    pub fn set_running(&mut self) {
        self.status = ContainerStatus::Running;
    }

    /// Transition to the "stopped" status.
    pub fn set_stopped(&mut self) {
        self.status = ContainerStatus::Stopped;
        self.pid = None;
    }

    /// Transition to the "paused" status.
    pub fn set_paused(&mut self) {
        self.status = ContainerStatus::Paused;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_transitions() {
        let mut state = ContainerState::new("test-container", "/bundles/test");
        assert_eq!(state.status, ContainerStatus::Creating);

        state.set_created(12345);
        assert_eq!(state.status, ContainerStatus::Created);
        assert_eq!(state.pid, Some(12345));
        assert!(state.status.can_start());

        state.set_running();
        assert_eq!(state.status, ContainerStatus::Running);
        assert!(state.status.can_kill());
        assert!(state.status.can_pause());

        state.set_stopped();
        assert_eq!(state.status, ContainerStatus::Stopped);
        assert!(state.status.can_delete());
    }

    #[test]
    fn state_serialization() {
        let state = ContainerState {
            oci_version: "1.2.0".to_string(),
            id: "test-container".to_string(),
            status: ContainerStatus::Running,
            pid: Some(12345),
            bundle: "/bundles/test".into(),
            annotations: HashMap::new(),
        };

        let json = serde_json::to_string(&state).unwrap();
        assert!(json.contains("\"status\":\"running\""));
        assert!(json.contains("\"pid\":12345"));
    }

    #[test]
    fn status_display() {
        assert_eq!(ContainerStatus::Creating.to_string(), "creating");
        assert_eq!(ContainerStatus::Running.to_string(), "running");
        assert_eq!(ContainerStatus::Stopped.to_string(), "stopped");
    }
}
