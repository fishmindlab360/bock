//! Container type and operations.

use std::path::PathBuf;
use std::sync::Arc;

use bock_common::{BockResult, ContainerId};
use bock_oci::state::ContainerStatus;
use bock_oci::{ContainerState, Spec};
use parking_lot::RwLock;
use tokio::sync::Mutex;

use crate::cgroup::CgroupManager;
use crate::namespace::NamespaceManager;

use super::config::RuntimeConfig;
use super::state::StateManager;

/// A container instance.
#[derive(Debug)]
pub struct Container {
    /// Container ID.
    id: ContainerId,
    /// OCI specification.
    spec: Spec,
    /// Runtime configuration.
    config: RuntimeConfig,
    /// Container state.
    state: Arc<RwLock<ContainerState>>,
    /// Cgroup manager.
    cgroup: Option<CgroupManager>,
    /// Namespace manager.
    namespace: Option<NamespaceManager>,
    /// Process ID of the container init process.
    pid: Arc<Mutex<Option<u32>>>,
    /// Bundle path.
    bundle: PathBuf,
}

impl Container {
    /// Create a new container.
    ///
    /// This sets up the container environment but does not start the process.
    pub async fn create(
        id: impl Into<String>,
        bundle: impl Into<PathBuf>,
        spec: &Spec,
    ) -> BockResult<Self> {
        let id = ContainerId::new(id)?;
        let bundle = bundle.into();
        let config = RuntimeConfig::default();

        let state = ContainerState::new(id.as_str(), &bundle);

        tracing::info!(
            container_id = %id,
            bundle = %bundle.display(),
            "Creating container"
        );

        // TODO: Setup namespaces
        // TODO: Setup cgroups
        // TODO: Setup rootfs
        // TODO: Setup networking

        Ok(Self {
            id,
            spec: spec.clone(),
            config,
            state: Arc::new(RwLock::new(state)),
            cgroup: None,
            namespace: None,
            pid: Arc::new(Mutex::new(None)),
            bundle,
        })
    }

    /// Get the container ID.
    #[must_use]
    pub fn id(&self) -> &ContainerId {
        &self.id
    }

    /// Get the container state.
    #[must_use]
    pub fn state(&self) -> ContainerState {
        self.state.read().clone()
    }

    /// Get the container status.
    #[must_use]
    pub fn status(&self) -> ContainerStatus {
        self.state.read().status
    }

    /// Get the container PID.
    pub async fn pid(&self) -> Option<u32> {
        *self.pid.lock().await
    }

    /// Start the container.
    ///
    /// This starts the container process. The container must be in the "created" state.
    pub async fn start(&self) -> BockResult<()> {
        let mut state = self.state.write();

        if !state.status.can_start() {
            return Err(bock_common::BockError::Config {
                message: format!(
                    "Container {} cannot be started (status: {})",
                    self.id, state.status
                ),
            });
        }

        tracing::info!(container_id = %self.id, "Starting container");

        // TODO: Execute the container process
        // TODO: Wait for process to initialize

        state.set_running();

        Ok(())
    }

    /// Kill the container.
    ///
    /// Sends a signal to the container process.
    pub async fn kill(&self, signal: i32) -> BockResult<()> {
        let state = self.state.read();

        if !state.status.can_kill() {
            return Err(bock_common::BockError::Config {
                message: format!(
                    "Container {} cannot be killed (status: {})",
                    self.id, state.status
                ),
            });
        }

        tracing::info!(
            container_id = %self.id,
            signal = signal,
            "Killing container"
        );

        // TODO: Send signal to container process

        Ok(())
    }

    /// Delete the container.
    ///
    /// Removes all container resources. The container must be stopped.
    pub async fn delete(&self) -> BockResult<()> {
        let state = self.state.read();

        if !state.status.can_delete() {
            return Err(bock_common::BockError::Config {
                message: format!(
                    "Container {} cannot be deleted (status: {})",
                    self.id, state.status
                ),
            });
        }

        tracing::info!(container_id = %self.id, "Deleting container");

        // TODO: Remove cgroups
        // TODO: Remove rootfs
        // TODO: Remove state files

        Ok(())
    }

    /// Pause the container.
    ///
    /// Freezes all processes in the container.
    pub async fn pause(&self) -> BockResult<()> {
        let mut state = self.state.write();

        if !state.status.can_pause() {
            return Err(bock_common::BockError::Config {
                message: format!(
                    "Container {} cannot be paused (status: {})",
                    self.id, state.status
                ),
            });
        }

        tracing::info!(container_id = %self.id, "Pausing container");

        // TODO: Freeze cgroup

        state.set_paused();

        Ok(())
    }

    /// Resume the container.
    ///
    /// Unfreezes all processes in the container.
    pub async fn resume(&self) -> BockResult<()> {
        let mut state = self.state.write();

        if !state.status.can_resume() {
            return Err(bock_common::BockError::Config {
                message: format!(
                    "Container {} cannot be resumed (status: {})",
                    self.id, state.status
                ),
            });
        }

        tracing::info!(container_id = %self.id, "Resuming container");

        // TODO: Thaw cgroup

        state.set_running();

        Ok(())
    }

    /// Wait for the container to exit.
    ///
    /// Returns the exit code of the container process.
    pub async fn wait(&self) -> BockResult<i32> {
        tracing::debug!(container_id = %self.id, "Waiting for container to exit");

        // TODO: Wait for process exit

        Ok(0)
    }

    /// Execute a command in the running container.
    pub async fn exec(&self, args: Vec<String>, env: Vec<String>) -> BockResult<i32> {
        let state = self.state.read();

        if !state.status.is_running() {
            return Err(bock_common::BockError::Config {
                message: format!(
                    "Container {} is not running (status: {})",
                    self.id, state.status
                ),
            });
        }

        tracing::info!(
            container_id = %self.id,
            args = ?args,
            "Executing in container"
        );

        // TODO: Enter container namespaces and exec

        Ok(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn create_container() {
        let spec = Spec::default();
        let container = Container::create("test-container", "/tmp/bundle", &spec)
            .await
            .unwrap();

        assert_eq!(container.id().as_str(), "test-container");
        assert_eq!(container.status(), ContainerStatus::Creating);
    }
}
