//! Multi-container orchestrator.

use bock_common::BockResult;
use dashmap::DashMap;

use crate::spec::BockoseSpec;

/// Service state.
#[derive(Debug, Clone)]
pub struct ServiceState {
    /// Service name.
    pub name: String,
    /// Container IDs.
    pub containers: Vec<String>,
    /// Current status.
    pub status: ServiceStatus,
}

/// Service status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ServiceStatus {
    /// Service is starting.
    Starting,
    /// Service is running.
    Running,
    /// Service is healthy.
    Healthy,
    /// Service is unhealthy.
    Unhealthy,
    /// Service is stopping.
    Stopping,
    /// Service is stopped.
    Stopped,
}

/// Multi-container orchestrator.
pub struct Orchestrator {
    /// Stack specification.
    spec: BockoseSpec,
    /// Service states.
    services: DashMap<String, ServiceState>,
}

impl Orchestrator {
    /// Create a new orchestrator.
    pub fn new(spec: BockoseSpec) -> Self {
        Self {
            spec,
            services: DashMap::new(),
        }
    }

    /// Start all services.
    pub async fn up(&self, _detach: bool) -> BockResult<()> {
        let stack_name = self.spec.stack_name();
        tracing::info!(stack = %stack_name, "Starting stack");

        // Build dependency graph
        let order = self.resolve_dependency_order()?;
        tracing::debug!(?order, "Resolved dependency order");

        // Create networks
        for (name, _network) in &self.spec.networks {
            tracing::info!(network = %name, "Creating network");
            // TODO: Create network
        }

        // Create volumes
        for (name, _volume) in &self.spec.volumes {
            tracing::info!(volume = %name, "Creating volume");
            // TODO: Create volume
        }

        // Start services in dependency order
        for service_name in order {
            self.start_service(&service_name).await?;
        }

        Ok(())
    }

    /// Stop all services.
    pub async fn down(&self, remove_volumes: bool) -> BockResult<()> {
        let stack_name = self.spec.stack_name();
        tracing::info!(stack = %stack_name, "Stopping stack");

        // Stop services in reverse order
        let order = self.resolve_dependency_order()?;
        for service_name in order.into_iter().rev() {
            self.stop_service(&service_name).await?;
        }

        // Remove networks
        for (name, _) in &self.spec.networks {
            tracing::info!(network = %name, "Removing network");
            // TODO: Remove network
        }

        // Remove volumes if requested
        if remove_volumes {
            for (name, _) in &self.spec.volumes {
                tracing::info!(volume = %name, "Removing volume");
                // TODO: Remove volume
            }
        }

        Ok(())
    }

    /// Start a single service.
    async fn start_service(&self, name: &str) -> BockResult<()> {
        tracing::info!(service = %name, "Starting service");

        let _service =
            self.spec
                .services
                .get(name)
                .ok_or_else(|| bock_common::BockError::Config {
                    message: format!("Service not found: {}", name),
                })?;

        // Initialize state
        self.services.insert(
            name.to_string(),
            ServiceState {
                name: name.to_string(),
                containers: Vec::new(),
                status: ServiceStatus::Starting,
            },
        );

        // TODO: Build if needed
        // TODO: Pull image if needed
        // TODO: Create container(s)
        // TODO: Start container(s)

        // Update state
        if let Some(mut state) = self.services.get_mut(name) {
            state.status = ServiceStatus::Running;
        }

        Ok(())
    }

    /// Stop a single service.
    async fn stop_service(&self, name: &str) -> BockResult<()> {
        tracing::info!(service = %name, "Stopping service");

        if let Some(mut state) = self.services.get_mut(name) {
            state.status = ServiceStatus::Stopping;
        }

        // TODO: Stop container(s)
        // TODO: Remove container(s)

        if let Some(mut state) = self.services.get_mut(name) {
            state.status = ServiceStatus::Stopped;
        }

        Ok(())
    }

    /// Resolve service dependency order (topological sort).
    fn resolve_dependency_order(&self) -> BockResult<Vec<String>> {
        // TODO: Implement proper topological sort
        // For now, just return services in arbitrary order
        Ok(self.spec.services.keys().cloned().collect())
    }

    /// List all services and their status.
    pub fn list_services(&self) -> Vec<ServiceState> {
        self.services
            .iter()
            .map(|entry| entry.value().clone())
            .collect()
    }

    /// Scale a service.
    pub async fn scale(&self, service: &str, replicas: u32) -> BockResult<()> {
        tracing::info!(service, replicas, "Scaling service");
        // TODO: Implement
        Ok(())
    }
}
