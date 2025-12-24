//! Multi-container orchestrator.

use std::collections::HashMap;
use std::path::PathBuf;

use bock_common::BockResult;
use dashmap::DashMap;

use crate::spec::BockoseSpec;
use bock::runtime::{Container, ContainerStats, NetworkConfig, RuntimeConfig};
use bock_image::store::ImageStore;
use bock_oci::runtime::{Mount, Spec};
use bock_oci::state::ContainerStatus;
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
    /// Assigned IP addresses.
    pub ips: Vec<String>,
}

impl ServiceState {
    fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            containers: Vec::new(),
            status: ServiceStatus::Starting,
            ips: Vec::new(),
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
    /// Next available IP octet (simple sequential IPAM).
    next_ip: std::sync::atomic::AtomicU8,
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
            next_ip: std::sync::atomic::AtomicU8::new(2),
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
        for (name, network_spec) in &self.spec.networks {
            let network_name = format!("{}_{}", stack_name, name);
            tracing::info!(network = %network_name, "Creating network");

            // Create bridge network
            match bock_network::BridgeManager::create(&network_name).await {
                Ok(bridge) => {
                    // Set IP if IPAM config provided
                    if let Some(ipam) = &network_spec.ipam {
                        if let Some(subnet) = &ipam.subnet {
                            // Use gateway or derive from subnet
                            let gateway = ipam.gateway.clone().unwrap_or_else(|| {
                                // Default gateway: first IP in subnet (e.g., 172.18.0.1/16)
                                subnet.replace(".0/", ".1/")
                            });
                            if let Err(e) = bridge.set_ip(&gateway).await {
                                tracing::warn!(network = %network_name, error = %e, "Failed to set gateway IP");
                            }
                        }
                    }
                    tracing::info!(network = %network_name, "Network created");
                }
                Err(e) => {
                    tracing::warn!(network = %network_name, error = %e, "Failed to create network (continuing)");
                }
            }
        }

        // Create volumes
        for (name, _volume_spec) in &self.spec.volumes {
            let volume_name = format!("{}_{}", stack_name, name);
            let volume_path = self.config.paths.volumes().join(&volume_name);
            tracing::info!(volume = %volume_name, path = %volume_path.display(), "Creating volume");

            if let Err(e) = std::fs::create_dir_all(&volume_path) {
                tracing::warn!(volume = %volume_name, error = %e, "Failed to create volume directory");
            }
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
            let network_name = format!("{}_{}", stack_name, name);
            tracing::info!(network = %network_name, "Removing network");

            if let Ok(bridge) = bock_network::BridgeManager::get(&network_name) {
                if let Err(e) = bridge.delete().await {
                    tracing::warn!(network = %network_name, error = %e, "Failed to remove network");
                }
            }
        }

        // Remove volumes if requested
        if remove_volumes {
            for (name, _) in &self.spec.volumes {
                let volume_name = format!("{}_{}", stack_name, name);
                let volume_path = self.config.paths.volumes().join(&volume_name);
                tracing::info!(volume = %volume_name, path = %volume_path.display(), "Removing volume");

                if let Err(e) = std::fs::remove_dir_all(&volume_path) {
                    tracing::warn!(volume = %volume_name, error = %e, "Failed to remove volume");
                }
            }
        }

        Ok(())
    }

    /// Start a single service.
    pub async fn start_service(&self, name: &str) -> BockResult<()> {
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
        // TODO: Merge image config with service spec
        // For now using default spec + simple overrides
        let mut spec = Spec::default();
        if !service_spec.command.is_empty() {
            if let Some(process) = &mut spec.process {
                process.args = service_spec.command.clone();
            }
        }

        // Volumes
        for volume in &service_spec.volumes {
            let parts: Vec<&str> = volume.split(':').collect();
            if parts.len() >= 2 {
                let source = parts[0];
                let destination = parts[1];
                let mode = if parts.len() > 2 { parts[2] } else { "rw" };

                // Check if source is a named volume
                let host_path = if self.spec.volumes.contains_key(source) {
                    // Named volume
                    self.config.paths.volumes().join(format!(
                        "{}_{}",
                        self.spec.stack_name(),
                        source
                    ))
                } else {
                    // Bind mount (host path)
                    PathBuf::from(source)
                };

                let mount = Mount {
                    destination: PathBuf::from(destination),
                    mount_type: Some("bind".to_string()),
                    source: Some(host_path),
                    options: vec![
                        "rbind".to_string(),
                        "rprivate".to_string(),
                        mode.to_string(),
                    ],
                };
                spec.mounts.push(mount);
            }
        }

        // 2. Prepare containers
        let replicas = service_spec
            .deploy
            .as_ref()
            .map(|d| d.replicas)
            .unwrap_or(1);

        for i in 1..=replicas {
            let container_name = format!("{}_{}_{}", self.spec.stack_name(), name, i);
            tracing::info!(container = %container_name, "Preparing replica {}/{}", i, replicas);

            // Prepare bundle
            // Using <data_root>/containers/<name>/bundle
            let container_dir = self.config.paths.container(&container_name);
            let bundle_path = container_dir.join("bundle");

            if bundle_path.exists() {
                if let Ok(existing) = Container::load(&container_name, self.config.clone()).await {
                    // If working properly, check state.
                    // For now, if load succeeds assume checking state is safe.
                    let s = existing.state();
                    if s.status == ContainerStatus::Running {
                        tracing::info!(container=%container_name, "Container already running, skipping");
                        if let Some(mut state) = self.services.get_mut(name) {
                            if !state.containers.contains(&container_name) {
                                state.containers.push(container_name.clone());
                            }
                            if let Some(net) = existing.network_config() {
                                if !state.ips.contains(&net.ip) {
                                    state.ips.push(net.ip.clone());
                                }
                            }
                        }
                        continue;
                    }
                }

                // Cleanup capability would be needed here for restart
                tracing::warn!("Container bundle already exists (not running), cleaning up...");
                std::fs::remove_dir_all(&bundle_path).ok();
            }
            std::fs::create_dir_all(&bundle_path)?;

            // Write config.json (Spec is reused, but needs config.json)
            let config_json = serde_json::to_string_pretty(&spec).map_err(|e| {
                bock_common::BockError::Config {
                    message: format!("Failed to serialize spec: {}", e),
                }
            })?;
            std::fs::write(bundle_path.join("config.json"), &config_json)?;

            // 3. Extract rootfs (Should ideally be done once and shared/copied, but for simplicity extract again or hardlink?)
            // Extracting again for now. Optimization: OverlayFS.
            // 3. Extract rootfs
            tracing::info!("Extracting image layers...");
            let image = self.image_store.get(&image_ref)?.ok_or_else(|| {
                bock_common::BockError::Internal {
                    message: format!("Image {} missing after ensure_image", image_ref),
                }
            })?;
            let rootfs = bundle_path.join("rootfs");
            self.image_store.extract_layers(&image, &rootfs)?;

            // 5. Create Container
            tracing::info!(container = %container_name, "Creating container");
            let mut container =
                Container::create(&container_name, &bundle_path, &spec, self.config.clone())
                    .await?;

            // 6. Network Configuration
            let host_octet = self
                .next_ip
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let ip_cidr = format!("172.18.0.{}/16", host_octet);
            let gateway = "172.18.0.1".to_string();

            let network_config = NetworkConfig {
                ip: ip_cidr.clone(),
                gateway,
            };
            container.set_network_config(network_config)?;

            // Generate /etc/hosts
            let pure_ip = ip_cidr.split('/').next().unwrap_or(&ip_cidr);
            let mut hosts_content =
                String::from("127.0.0.1\tlocalhost\n::1\tlocalhost ip6-localhost ip6-loopback\n");
            hosts_content.push_str(&format!("{}\t{}\n", pure_ip, name)); // Self
            hosts_content.push_str(&format!("{}\t{}\n", pure_ip, container_name)); // Self container name

            // Add peers
            for entry in &self.services {
                // Add all IPs of other services
                if entry.key() != name {
                    for other_ip in &entry.value().ips {
                        let other_pure = other_ip.split('/').next().unwrap_or(other_ip);
                        hosts_content.push_str(&format!("{}\t{}\n", other_pure, entry.key()));
                    }
                } else {
                    // Add other replicas of self?
                    // They might not be started yet if we are in loop.
                    // Service Discovery usually only knows about *already started* services in this simple model.
                }
            }

            let hosts_path = bundle_path.join("rootfs/etc/hosts");
            std::fs::create_dir_all(hosts_path.parent().unwrap())?;
            std::fs::write(hosts_path, hosts_content)?;

            // Update state
            if let Some(mut state) = self.services.get_mut(name) {
                state.containers.push(container_name.clone());
                state.ips.push(ip_cidr.clone());
            }

            // 7. Start Container
            tracing::info!(container = %container_name, "Starting container");
            container.start().await?;
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
    pub async fn stop_service(&self, name: &str) -> BockResult<()> {
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
        Ok(())
    }

    /// Execute a command in a service container.
    pub async fn exec(&self, name: &str, cmd: Vec<String>) -> BockResult<i32> {
        if let Some(state) = self.services.get(name) {
            // Use first container
            if let Some(container_name) = state.containers.first() {
                // Load container
                let container = Container::load(container_name, self.config.clone()).await?;
                return container.exec_command(&cmd).await;
            }
        }

        Err(bock_common::BockError::Config {
            message: format!("Service {} has no running containers", name),
        })
    }

    /// Scale a service.
    pub async fn scale(&self, name: &str, replicas: u32) -> BockResult<()> {
        let service_spec = self
            .spec
            .services
            .get(name)
            .ok_or_else(|| bock_common::BockError::Config {
                message: format!("Service not found: {}", name),
            })?
            .clone();

        // Update spec temporarily (in memory)
        // Note: self.spec is immutable in this context if we don't have interior mutability.
        // But we are in async method. `self` is &self.
        // We can't modify self.spec.
        // We can treat self.spec as "source of truth for configuration on disk/load".
        // But for runtime operations, we might want to override.
        // `start_service` reads from `self.spec`.
        // We need to pass the override to `start_service` or update `self.spec` (if we had a RwLock).
        // Since `Orchestrator` fields are private and immutable references here...
        // We can't easily change `self.spec`.

        // Refactoring opportunity: `spec` should be `RwLock` or `start_service` should take optional overrides.
        // For now, I will manually handle scaling logic here:
        // 1. Get current replicas (from state or spec).
        // 2. If N < replicas: start (using logic from start_service but custom loop).
        // 3. If N > replicas: stop (reverse loop).

        // Actually `start_service` reads `self.spec`. I can't force it to use new replica count unless I change `self.spec`.
        // But I can implemented "scale logic" here which mimics start_service loop.

        tracing::info!(service=%name, replicas, "Scaling service");

        // 1. Determine current count
        let current_count = if let Some(state) = self.services.get(name) {
            state.containers.len()
        } else {
            0
        };

        if (replicas as usize) > current_count {
            // Scale UP
            // We need to run start logic for indices (current_count + 1) to replicas.
            // But indices might be sparse if some were deleted?
            // `start_service` assumes 1..N.
            // If we just loop 1..N with our idempotent `start_service` logic, it will fill gaps and add new ones if we could override the replica count it sees!
            // BUT `start_service` reads `self.spec.services.get(name).deploy`.

            // Simplest fix: Interior Mutability or move Spec to DashMap or similar.
            // Since I can't change struct definition easily now without broader impact, I will COPY the logic from start_service
            // but with the new replica count.
            // Or better: Implement `ensure_service_replicas(name, count)` helper.
            self.ensure_service_replicas(name, replicas, &service_spec)
                .await?;
        } else if (replicas as usize) < current_count {
            // Scale DOWN
            // Stop containers with highest indices
            // Assuming strict naming convention `_i`
            // State containers list might be unordered?
            // Let's rely on reconstructing names.

            for i in (replicas + 1)..=100 {
                // limit to avoid infinite loop
                let container_name = format!("{}_{}_{}", self.spec.stack_name(), name, i);
                // Check if running/exists
                if let Ok(container) = Container::load(&container_name, self.config.clone()).await {
                    // Stop and delete
                    tracing::info!(container=%container_name, "Stopping excess replica");
                    container.kill(15).await.ok();
                    container.delete().await.ok();

                    // Updates state
                    if let Some(mut state) = self.services.get_mut(name) {
                        state.containers.retain(|c| c != &container_name);
                        // Also need to remove IP?
                        // IP in state.ips needs to be removed.
                        // Container network config is gone with container (if we deleted it).
                        // But we need to update memory state.
                        // We can just call refresh_state() at the end? Easiest.
                    }
                } else {
                    // If we can't load it, assume we are done with consecutive indices?
                    // Or maybe we just reached end of deployed replicas.
                    if i > (current_count as u32 + 5) {
                        break;
                    } // heuristic
                }
            }
            self.refresh_state().await?;
        }

        Ok(())
    }

    /// Helper to ensure N replicas running (extracted/modified from start_service logic)
    async fn ensure_service_replicas(
        &self,
        name: &str,
        replicas: u32,
        service_spec: &crate::spec::ServiceSpec,
    ) -> BockResult<()> {
        let image_ref = self.ensure_image(name, service_spec).await?;

        // Copied loop from start_service but with explicit 'replicas' count
        let mut spec = Spec::default();
        if !service_spec.command.is_empty() {
            if let Some(process) = &mut spec.process {
                process.args = service_spec.command.clone();
            }
        }

        // Volumes (copy-paste from start_service or refactor to helper)
        for volume in &service_spec.volumes {
            let parts: Vec<&str> = volume.split(':').collect();
            if parts.len() >= 2 {
                let source = parts[0];
                let destination = parts[1];
                let mode = if parts.len() > 2 { parts[2] } else { "rw" };
                let host_path = if self.spec.volumes.contains_key(source) {
                    self.config.paths.volumes().join(format!(
                        "{}_{}",
                        self.spec.stack_name(),
                        source
                    ))
                } else {
                    PathBuf::from(source)
                };
                let mount = Mount {
                    destination: PathBuf::from(destination),
                    mount_type: Some("bind".to_string()),
                    source: Some(host_path),
                    options: vec![
                        "rbind".to_string(),
                        "rprivate".to_string(),
                        mode.to_string(),
                    ],
                };
                spec.mounts.push(mount);
            }
        }

        for i in 1..=replicas {
            let container_name = format!("{}_{}_{}", self.spec.stack_name(), name, i);
            let container_dir = self.config.paths.container(&container_name);
            let bundle_path = container_dir.join("bundle");

            if bundle_path.exists() {
                if let Ok(existing) = Container::load(&container_name, self.config.clone()).await {
                    let s = existing.state();
                    if s.status == ContainerStatus::Running {
                        if let Some(mut state) = self.services.get_mut(name) {
                            if !state.containers.contains(&container_name) {
                                state.containers.push(container_name.clone());
                            }
                            if let Some(net) = existing.network_config() {
                                if !state.ips.contains(&net.ip) {
                                    state.ips.push(net.ip.clone());
                                }
                            }
                        }
                        continue;
                    }
                }
                std::fs::remove_dir_all(&bundle_path).ok();
            }
            std::fs::create_dir_all(&bundle_path)?;

            // Re-write config
            let config_json = serde_json::to_string_pretty(&spec).map_err(|e| {
                bock_common::BockError::Config {
                    message: format!("Failed to serialize spec: {}", e),
                }
            })?;
            std::fs::write(bundle_path.join("config.json"), &config_json)?;

            // Extract layers
            let image = self.image_store.get(&image_ref)?.unwrap();
            let rootfs = bundle_path.join("rootfs");
            self.image_store.extract_layers(&image, &rootfs)?;

            // Create
            let mut container =
                Container::create(&container_name, &bundle_path, &spec, self.config.clone())
                    .await?;

            // Network
            let host_octet = self
                .next_ip
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let ip_cidr = format!("172.18.0.{}/16", host_octet);
            let gateway = "172.18.0.1".to_string();
            container.set_network_config(NetworkConfig {
                ip: ip_cidr.clone(),
                gateway,
            })?;

            if let Some(mut state) = self.services.get_mut(name) {
                state.ips.push(ip_cidr.clone());
            }

            // Hosts (simplified for this method - ideally share logic)
            let pure_ip = ip_cidr.split('/').next().unwrap_or(&ip_cidr);
            let mut hosts_content =
                String::from("127.0.0.1\tlocalhost\n::1\tlocalhost ip6-localhost ip6-loopback\n");
            hosts_content.push_str(&format!("{}\t{}\n", pure_ip, name));
            hosts_content.push_str(&format!("{}\t{}\n", pure_ip, container_name));
            for entry in &self.services {
                if entry.key() != name {
                    for other_ip in &entry.value().ips {
                        let other_pure = other_ip.split('/').next().unwrap_or(other_ip);
                        hosts_content.push_str(&format!("{}\t{}\n", other_pure, entry.key()));
                    }
                }
            }
            let hosts_path = bundle_path.join("rootfs/etc/hosts");
            std::fs::create_dir_all(hosts_path.parent().unwrap())?;
            std::fs::write(hosts_path, hosts_content)?;

            container.start().await?;
            if let Some(mut state) = self.services.get_mut(name) {
                state.containers.push(container_name.clone());
            }
        }
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

    /// Check health of all services.
    pub async fn check_health(&self) -> BockResult<()> {
        tracing::debug!("Checking health of services");
        // Clone keys and partial state to avoid holding DashMap locks across await
        let services: Vec<(String, ServiceStatus, Vec<String>)> = self
            .services
            .iter()
            .map(|e| {
                (
                    e.key().clone(),
                    e.value().status,
                    e.value().containers.clone(),
                )
            })
            .collect();

        for (name, status, containers) in services {
            if status == ServiceStatus::Stopped || status == ServiceStatus::Stopping {
                continue;
            }

            if let Some(spec) = self.spec.services.get(&name) {
                if let Some(health) = &spec.healthcheck {
                    let mut all_healthy = true;
                    for id in containers {
                        let container = match Container::load(&id, self.config.clone()).await {
                            Ok(c) => c,
                            Err(_) => {
                                tracing::warn!(service=%name, container=%id, "Failed to load container during health check");
                                all_healthy = false;
                                continue;
                            }
                        };

                        let healthy = if !health.cmd.is_empty() {
                            match container.exec_command(&health.cmd).await {
                                Ok(0) => true,
                                _ => false,
                            }
                        } else if let Some(url) = &health.http {
                            // Run curl from host against container IP
                            if let Some(net) = container.network_config() {
                                let ip = net.ip.split('/').next().unwrap_or(&net.ip);
                                let target_url =
                                    url.replace("localhost", ip).replace("127.0.0.1", ip);

                                tracing::debug!(service=%name, url=%target_url, "Checking HTTP health");
                                std::process::Command::new("curl")
                                    .args(&["-s", "-f", "-o", "/dev/null", &target_url])
                                    .status()
                                    .map(|s| s.success())
                                    .unwrap_or(false)
                            } else {
                                tracing::warn!(service=%name, "HTTP health check failed: no network config",);
                                false
                            }
                        } else {
                            true // No check defined means healthy?
                        };

                        if !healthy {
                            all_healthy = false;
                            tracing::warn!(service=%name, container=%id, "Health check failed");
                        }
                    }

                    // Update status
                    if let Some(mut state) = self.services.get_mut(&name) {
                        state.status = if all_healthy {
                            ServiceStatus::Healthy
                        } else {
                            ServiceStatus::Unhealthy
                        };
                        tracing::debug!(service=%name, status=?state.status, "Updated service health status");
                    }
                }
            }
        }
        Ok(())
    }

    /// Refresh service state from running containers.
    pub async fn refresh_state(&self) -> BockResult<()> {
        let stack_name = self.spec.stack_name();
        tracing::debug!("Refreshing service state");

        for (name, service_spec) in &self.spec.services {
            // Ensure entry exists
            if !self.services.contains_key(name) {
                self.services.insert(name.clone(), ServiceState::new(name));
            }

            let replicas = service_spec
                .deploy
                .as_ref()
                .map(|d| d.replicas)
                .unwrap_or(1);
            let mut current_containers = Vec::new();
            let mut current_ips = Vec::new();
            let mut any_running = false;

            for i in 1..=replicas {
                let container_name = format!("{}_{}_{}", stack_name, name, i);
                if let Ok(container) = Container::load(&container_name, self.config.clone()).await {
                    current_containers.push(container_name);
                    if let Some(net) = container.network_config() {
                        current_ips.push(net.ip.clone());
                    }
                    any_running = true;
                }
            }

            if let Some(mut state) = self.services.get_mut(name) {
                state.containers = current_containers;
                state.ips = current_ips;
                state.status = if any_running {
                    ServiceStatus::Running
                } else {
                    ServiceStatus::Stopped
                };
            }
        }
        Ok(())
    }
    /// Get stats for all services.
    pub async fn get_service_stats(&self) -> BockResult<Vec<(String, String, ContainerStats)>> {
        let mut stats = Vec::new();
        for entry in &self.services {
            let name = entry.key();
            let state = entry.value();
            for container_id in &state.containers {
                if let Ok(container) = Container::load(container_id, self.config.clone()).await {
                    if let Ok(s) = container.stats() {
                        stats.push((name.clone(), container_id.clone(), s));
                    }
                }
            }
        }
        Ok(stats)
    }
}
