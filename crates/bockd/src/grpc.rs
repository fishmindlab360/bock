//! gRPC service implementations for bockd.

use std::sync::Arc;
use tonic::{Request, Response, Status};

// Include generated protobuf code
pub mod bockd_proto {
    tonic::include_proto!("bockd.v1");
}

use bock::runtime::{Container, RuntimeConfig, RuntimeEvent};
use bockd_proto::container_service_server::{ContainerService, ContainerServiceServer};
use bockd_proto::{
    Container as ProtoContainer, ContainerEvent, ContainerIdRequest, ContainerOperationResponse,
    CreateContainerRequest, GetContainerRequest, KillContainerRequest, ListContainersRequest,
    ListContainersResponse, LogEntry, StopContainerRequest, StreamLogsRequest, WatchEventsRequest,
};

/// Container service implementation with runtime integration.
pub struct ContainerServiceImpl {
    config: Arc<RuntimeConfig>,
}

impl ContainerServiceImpl {
    /// Create new service with runtime config.
    pub fn new(config: RuntimeConfig) -> Self {
        Self {
            config: Arc::new(config),
        }
    }

    /// Helper to get config clone
    fn config(&self) -> RuntimeConfig {
        (*self.config).clone()
    }
}

#[tonic::async_trait]
impl ContainerService for ContainerServiceImpl {
    async fn list_containers(
        &self,
        request: Request<ListContainersRequest>,
    ) -> Result<Response<ListContainersResponse>, Status> {
        let req = request.into_inner();
        tracing::debug!(all = req.all, "Listing containers via gRPC");

        // List containers from state directory
        let containers_dir = self.config.paths.containers();
        let mut containers = Vec::new();

        if let Ok(entries) = std::fs::read_dir(&containers_dir) {
            for entry in entries.flatten() {
                if entry.path().is_dir() {
                    let name = entry.file_name().to_string_lossy().to_string();

                    // Check state file exists
                    let state_file = entry.path().join("state.json");
                    if state_file.exists() {
                        if let Ok(content) = std::fs::read_to_string(&state_file) {
                            if let Ok(state) =
                                serde_json::from_str::<bock_oci::state::ContainerState>(&content)
                            {
                                // Filter by running if not showing all
                                if !req.all
                                    && state.status != bock_oci::state::ContainerStatus::Running
                                {
                                    continue;
                                }

                                containers.push(ProtoContainer {
                                    id: state.id.to_string(),
                                    name: name.clone(),
                                    image: String::new(), // Image name not in state currently
                                    status: format!("{:?}", state.status),
                                    created_at: 0,
                                    labels: std::collections::HashMap::new(),
                                });
                            }
                        }
                    }
                }
            }
        }

        Ok(Response::new(ListContainersResponse { containers }))
    }

    async fn get_container(
        &self,
        request: Request<GetContainerRequest>,
    ) -> Result<Response<ProtoContainer>, Status> {
        let id = request.into_inner().id;
        let state_file = self.config.paths.container(&id).join("state.json");

        if let Ok(content) = std::fs::read_to_string(&state_file) {
            if let Ok(state) = serde_json::from_str::<bock_oci::state::ContainerState>(&content) {
                return Ok(Response::new(ProtoContainer {
                    id: state.id.to_string(),
                    name: id,
                    image: String::new(),
                    status: format!("{:?}", state.status),
                    created_at: 0,
                    labels: std::collections::HashMap::new(),
                }));
            }
        }

        Err(Status::not_found(format!("Container {} not found", id)))
    }

    async fn create_container(
        &self,
        request: Request<CreateContainerRequest>,
    ) -> Result<Response<ProtoContainer>, Status> {
        let req = request.into_inner();
        tracing::info!(name = %req.name, image = %req.image, "Creating container via gRPC");

        // Stub - full implementation needs image extraction and OCI spec creation
        // This is still extensive work to do proper create via API
        let container = ProtoContainer {
            id: uuid::Uuid::new_v4().to_string(),
            name: req.name,
            image: req.image,
            status: "created".to_string(),
            created_at: chrono::Utc::now().timestamp(),
            labels: req.labels,
        };
        Ok(Response::new(container))
    }

    async fn start_container(
        &self,
        request: Request<ContainerIdRequest>,
    ) -> Result<Response<ContainerOperationResponse>, Status> {
        let id = request.into_inner().id;
        tracing::info!(container = %id, "Starting container via gRPC");

        let config = self.config();
        match Container::load(&id, config).await {
            Ok(container) => {
                if let Err(e) = container.start().await {
                    return Err(Status::internal(format!("Failed to start: {}", e)));
                }
                Ok(Response::new(ContainerOperationResponse {
                    success: true,
                    message: format!("Container {} started", id),
                }))
            }
            Err(e) => Err(Status::not_found(format!(
                "Container {} not found: {}",
                id, e
            ))),
        }
    }

    async fn stop_container(
        &self,
        request: Request<StopContainerRequest>,
    ) -> Result<Response<ContainerOperationResponse>, Status> {
        let req = request.into_inner();
        tracing::info!(container = %req.id, timeout = %req.timeout_seconds, "Stopping container via gRPC");

        let config = self.config();
        match Container::load(&req.id, config).await {
            Ok(container) => {
                // Send SIGTERM
                if let Err(e) = container.kill(15).await {
                    tracing::warn!(container = %req.id, error = %e, "Failed to send SIGTERM");
                }
                // Wait for container to exit
                let _ = container.wait().await;

                Ok(Response::new(ContainerOperationResponse {
                    success: true,
                    message: format!("Container {} stopped", req.id),
                }))
            }
            Err(e) => Err(Status::not_found(format!(
                "Container {} not found: {}",
                req.id, e
            ))),
        }
    }

    async fn kill_container(
        &self,
        request: Request<KillContainerRequest>,
    ) -> Result<Response<ContainerOperationResponse>, Status> {
        let req = request.into_inner();
        tracing::info!(container = %req.id, signal = %req.signal, "Killing container via gRPC");

        let config = self.config();
        match Container::load(&req.id, config).await {
            Ok(container) => {
                if let Err(e) = container.kill(req.signal).await {
                    return Err(Status::internal(format!("Failed to kill: {}", e)));
                }
                Ok(Response::new(ContainerOperationResponse {
                    success: true,
                    message: format!("Container {} killed with signal {}", req.id, req.signal),
                }))
            }
            Err(e) => Err(Status::not_found(format!(
                "Container {} not found: {}",
                req.id, e
            ))),
        }
    }

    async fn delete_container(
        &self,
        request: Request<ContainerIdRequest>,
    ) -> Result<Response<ContainerOperationResponse>, Status> {
        let id = request.into_inner().id;
        tracing::info!(container = %id, "Deleting container via gRPC");

        let config = self.config();
        match Container::load(&id, config).await {
            Ok(container) => {
                if let Err(e) = container.delete().await {
                    return Err(Status::internal(format!("Failed to delete: {}", e)));
                }
                Ok(Response::new(ContainerOperationResponse {
                    success: true,
                    message: format!("Container {} deleted", id),
                }))
            }
            Err(e) => Err(Status::not_found(format!(
                "Container {} not found: {}",
                id, e
            ))),
        }
    }

    type WatchEventsStream =
        std::pin::Pin<Box<dyn futures::Stream<Item = Result<ContainerEvent, Status>> + Send>>;

    async fn watch_events(
        &self,
        request: Request<WatchEventsRequest>,
    ) -> Result<Response<Self::WatchEventsStream>, Status> {
        let req = request.into_inner();
        let mut rx = self.config.event_bus.subscribe();
        let (tx, out_rx) = tokio::sync::mpsc::channel(100);

        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        let (id, type_, ts) = match event {
                            RuntimeEvent::ContainerCreated { id, timestamp } => {
                                (id, "create".to_string(), timestamp)
                            }
                            RuntimeEvent::ContainerStarted { id, timestamp } => {
                                (id, "start".to_string(), timestamp)
                            }
                            RuntimeEvent::ContainerStopped { id, timestamp } => {
                                (id, "stop".to_string(), timestamp)
                            }
                            RuntimeEvent::ContainerPaused { id, timestamp } => {
                                (id, "pause".to_string(), timestamp)
                            }
                            RuntimeEvent::ContainerResumed { id, timestamp } => {
                                (id, "resume".to_string(), timestamp)
                            }
                            RuntimeEvent::ContainerDeleted { id, timestamp } => {
                                (id, "delete".to_string(), timestamp)
                            }
                        };

                        // Filter check
                        if !req.container_ids.is_empty() && !req.container_ids.contains(&id) {
                            continue;
                        }

                        let proto_event = ContainerEvent {
                            container_id: id,
                            event_type: type_,
                            timestamp: ts,
                            attributes: std::collections::HashMap::new(),
                        };

                        if tx.send(Ok(proto_event)).await.is_err() {
                            break;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            }
        });

        let stream = tokio_stream::wrappers::ReceiverStream::new(out_rx);
        Ok(Response::new(Box::pin(stream)))
    }

    type StreamLogsStream =
        std::pin::Pin<Box<dyn futures::Stream<Item = Result<LogEntry, Status>> + Send>>;

    async fn stream_logs(
        &self,
        request: Request<StreamLogsRequest>,
    ) -> Result<Response<Self::StreamLogsStream>, Status> {
        let req = request.into_inner();
        let id = req.container_id;
        let follow = req.follow;

        let config = self.config();
        let container_dir = config.paths.container(&id);
        let log_path = container_dir.join("stdout.log");

        // Create a channel for the stream
        let (tx, rx) = tokio::sync::mpsc::channel(100);

        tokio::spawn(async move {
            let file = match tokio::fs::File::open(&log_path).await {
                Ok(f) => f,
                Err(_) => return, // No log file or error
            };

            use tokio::io::{AsyncBufReadExt, BufReader};
            let mut reader = BufReader::new(file);
            let mut line = String::new();

            loop {
                line.clear();
                match reader.read_line(&mut line).await {
                    Ok(0) => {
                        // EOF
                        if !follow {
                            break;
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    }
                    Ok(_) => {
                        if tx
                            .send(Ok(LogEntry {
                                container_id: id.clone(),
                                stream: "stdout".to_string(),
                                data: line.trim_end().as_bytes().to_vec(),
                                timestamp: chrono::Utc::now().timestamp(),
                            }))
                            .await
                            .is_err()
                        {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        // Convert receiver to stream
        let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
        Ok(Response::new(Box::pin(stream)))
    }
}

/// Create the gRPC server with runtime config.
pub fn grpc_server(config: RuntimeConfig) -> ContainerServiceServer<ContainerServiceImpl> {
    ContainerServiceServer::new(ContainerServiceImpl::new(config))
}
