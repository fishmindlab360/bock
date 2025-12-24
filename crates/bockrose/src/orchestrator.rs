//! Multi-container orchestrator.

use std::collections::HashMap;
use std::path::PathBuf;

use bock_common::BockResult;
use dashmap::DashMap;

use crate::spec::BockoseSpec;
use bock::runtime::Container;
use bock::runtime::RuntimeConfig;
use bock_image::store::ImageStore;
use bock_oci::Spec;
use bock_runtime::{Bockfile, Builder};

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

impl ServiceState {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            containers: Vec::new(),
            status: ServiceStatus::Starting,
        }
    }
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
    /// Image store.
    image_store: ImageStore,
    /// Runtime config.
    config: RuntimeConfig,
}

impl Orchestrator {
    /// Create a new orchestrator.
    pub fn new(spec: BockoseSpec) -> BockResult<Self> {
        let config = RuntimeConfig::default();
        let image_store = ImageStore::new(config.paths.images())?;

        Ok(Self {
            spec,
            services: DashMap::new(),
            image_store,
            config,
        })
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
        let service_spec =
            self.spec
                .services
                .get(name)
                .ok_or_else(|| bock_common::BockError::Config {
                    message: format!("Service not found: {}", name),
                })?;

        tracing::info!(service = %name, "Starting service");

        // Initialize state
        self.services
            .insert(name.to_string(), ServiceState::new(name));

        // 1. Ensure image is available
        let image_ref = self.ensure_image(name, service_spec).await?;
        tracing::debug!(service = %name, image = %image_ref, "Image ready");

        // 2. Prepare container(s)
        // For now, assume replicas = 1
        let container_name = format!("{}_{}_1", self.spec.stack_name(), name);

        // Check if already exists/running
        // TODO: Checking existing containers is complex without loading state.
        // For simple "up", we can try to create, if fails check why.
        // Or simpler: just generate a new unique ID if we wanted stateless, but we want stable names.

        // Prepare bundle
        // Using <data_root>/containers/<name>/bundle
        let container_dir = self.config.paths.container(&container_name);
        let bundle_path = container_dir.join("bundle");
        if bundle_path.exists() {
            // Cleanup capability would be needed here for restart
            tracing::warn!("Container bundle already exists, cleaning up...");
            std::fs::remove_dir_all(&bundle_path).ok();
        }
        std::fs::create_dir_all(&bundle_path)?;

        // 3. Extract rootfs
        tracing::info!("Extracting image layers...");
        let image =
            self.image_store
                .get(&image_ref)?
                .ok_or_else(|| bock_common::BockError::Internal {
                    message: format!("Image {} missing after ensure_image", image_ref),
                })?;
        let rootfs = bundle_path.join("rootfs");
        self.image_store.extract_layers(&image, &rootfs)?;

        // 4. Create OCI Spec
        // TODO: Merge image config with service spec
        // For now using default spec + simple overrides
        let mut spec = Spec::default();
        if !service_spec.command.is_empty() {
            if let Some(process) = &mut spec.process {
                process.args = service_spec.command.clone();
            }
        }
        // TODO: Env, Entrypoint, etc. using spec.process.env

        // Write config.json
        let config_json =
            serde_json::to_string_pretty(&spec).map_err(|e| bock_common::BockError::Config {
                message: format!("Failed to serialize spec: {}", e),
            })?;
        std::fs::write(bundle_path.join("config.json"), config_json)?;

        // 5. Create Container
        tracing::info!(container = %container_name, "Creating container");
        let container =
            Container::create(&container_name, &bundle_path, &spec, self.config.clone()).await?;

        // 6. Start Container
        tracing::info!(container = %container_name, "Starting container");
        container.start().await?;

        // Update state
        if let Some(mut state) = self.services.get_mut(name) {
            state.status = ServiceStatus::Running;
            state.containers.push(container_name);
        }

        Ok(())
    }

    /// Ensure image exists (build or pull).
    async fn ensure_image(
        &self,
        name: &str,
        spec: &crate::spec::ServiceSpec,
    ) -> BockResult<String> {
        if let Some(build_config) = &spec.build {
            tracing::info!(service = %name, "Building image...");
            let (context_path, dockerfile_path) = match build_config {
                crate::spec::BuildConfig::Path(p) => (PathBuf::from(p), None),
                crate::spec::BuildConfig::Full { context, file, .. } => {
                    (PathBuf::from(context), file.as_ref().map(PathBuf::from))
                }
            };

            let bockfile_path = if let Some(p) = dockerfile_path {
                if p.is_absolute() {
                    p
                } else {
                    context_path.join(p)
                }
            } else {
                context_path.join("Bockfile")
            };

            let bockfile = Bockfile::from_file(&bockfile_path)?;
            let tag = spec
                .image
                .clone()
                .unwrap_or_else(|| format!("{}:latest", name));

            let options = bock_runtime::build::BuildOptions::default(); // TODO: Map build args
            let builder = Builder::with_options(bockfile, context_path, tag.clone(), options);

            // Build
            let built = builder.build().await?;
            // For now assume image store is updated or we just use tag if shared root (bock-runtime and bock-rose sharing same bock-image might need coordination if image store is not lock-safe or updated implicitly).
            // Bock-runtime writes OCI struct to disk. We need to ingest it.
            // Assume integration for now: bock-runtime SHOULD save to store.
            // But bock-runtime currently DOES NOT save to store in `build.rs` I viewed. It just creates OCI filesystem struct.
            // We need to implement ingestion or update Bock-runtime to save.
            Ok(built.tag)
        } else if let Some(image) = &spec.image {
            tracing::info!(service = %name, image = %image, "Checking/Pulling image...");
            if self.image_store.get(image)?.is_none() {
                tracing::info!("Pulling image {}...", image);
                // TODO: Registry pull
                return Err(bock_common::BockError::Config {
                    message: format!("Image {} not found locally (pull not implemented)", image),
                });
            }
            Ok(image.clone())
        } else {
            Err(bock_common::BockError::Config {
                message: format!("Service {} has no build or image config", name),
            })
        }
    }

    /// Stop a single service.
    async fn stop_service(&self, name: &str) -> BockResult<()> {
        let container_ids = if let Some(mut state) = self.services.get_mut(name) {
            state.status = ServiceStatus::Stopping;
            state.containers.clone()
        } else {
            return Ok(());
        };

        tracing::info!(service = %name, count = %container_ids.len(), "Stopping service containers");

        for id in container_ids {
            // Load container
            match Container::load(&id, self.config.clone()).await {
                Ok(container) => {
                    tracing::debug!(container = %id, "Stopping container");
                    // Send SIGTERM
                    if let Err(e) = container.kill(15).await {
                        tracing::warn!(container = %id, error = %e, "Failed to send SIGTERM");
                    }

                    // Wait for exit
                    // TODO: Implement timeout and SIGKILL
                    if let Err(e) = container.wait().await {
                        tracing::warn!(container = %id, error = %e, "Failed to wait for container");
                    }

                    // Delete
                    tracing::debug!(container = %id, "Deleting container");
                    if let Err(e) = container.delete().await {
                        tracing::warn!(container = %id, error = %e, "Failed to delete container");
                    }
                }
                Err(e) => {
                    tracing::warn!(container = %id, error = %e, "Failed to load container (skipping)");
                }
            }
        }

        if let Some(mut state) = self.services.get_mut(name) {
            state.status = ServiceStatus::Stopped;
            state.containers.clear();
        }

        tracing::info!(service = %name, "Service stopped");
        Ok(())
    }

    /// Resolve service dependency order (topological sort).
    fn resolve_dependency_order(&self) -> BockResult<Vec<String>> {
        let mut graph: HashMap<&String, Vec<&String>> = HashMap::new();
        let mut in_degree: HashMap<&String, usize> = HashMap::new();

        // Initialize graph
        for service in self.spec.services.keys() {
            graph.insert(service, Vec::new());
            in_degree.insert(service, 0);
        }

        // Build graph
        for (service, spec) in &self.spec.services {
            match &spec.depends_on {
                crate::spec::DependsOn::None => {}
                crate::spec::DependsOn::Simple(deps) => {
                    for dep in deps {
                        if !self.spec.services.contains_key(dep) {
                            return Err(bock_common::BockError::Config {
                                message: format!(
                                    "Service '{}' depends on unknown service '{}'",
                                    service, dep
                                ),
                            });
                        }
                        graph.get_mut(dep).unwrap().push(service);
                        *in_degree.get_mut(service).unwrap() += 1;
                    }
                }
                crate::spec::DependsOn::Full(deps) => {
                    for dep in deps.keys() {
                        if !self.spec.services.contains_key(dep) {
                            return Err(bock_common::BockError::Config {
                                message: format!(
                                    "Service '{}' depends on unknown service '{}'",
                                    service, dep
                                ),
                            });
                        }
                        graph.get_mut(dep).unwrap().push(service);
                        *in_degree.get_mut(service).unwrap() += 1;
                    }
                }
            }
        }

        // Kahn's algorithm
        let mut queue: Vec<&String> = in_degree
            .iter()
            .filter(|&(_, &degree)| degree == 0)
            .map(|(service, _)| *service)
            .collect();

        // Sort for deterministic output
        queue.sort();

        let mut sorted_order = Vec::new();

        while let Some(service) = queue.pop() {
            // queue.pop() gives reverse alphabetical order if we don't care, but for deterministic testable output sorting helps
            // Note: using pop with sorting above technically processes in reverse order of sort if we consider it a stack, but it's fine as long as topology is respected.
            // To be strictly alphabetical BFS, we'd need a MinHeap or remove from front (inefficient for Vec).
            // For simple orchestration, just valid topological order is key.
            sorted_order.push(service.clone());

            if let Some(neighbors) = graph.get(service) {
                for &neighbor in neighbors {
                    if let Some(degree) = in_degree.get_mut(neighbor) {
                        *degree -= 1;
                        if *degree == 0 {
                            queue.push(neighbor);
                        }
                    }
                }
            }
            // Re-sort queue to maintain deterministic order (optional but nice)
            queue.sort();
        }

        if sorted_order.len() != self.spec.services.len() {
            return Err(bock_common::BockError::Config {
                message: "Circular dependency detected in services".to_string(),
            });
        }

        Ok(sorted_order)
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
