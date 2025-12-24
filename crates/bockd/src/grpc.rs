//! gRPC service implementations for bockd.

use tonic::{Request, Response, Status};

// Include generated protobuf code
pub mod bockd_proto {
    tonic::include_proto!("bockd.v1");
}

use bockd_proto::container_service_server::{ContainerService, ContainerServiceServer};
use bockd_proto::{
    Container, ContainerEvent, ContainerIdRequest, ContainerOperationResponse,
    CreateContainerRequest, GetContainerRequest, KillContainerRequest, ListContainersRequest,
    ListContainersResponse, LogEntry, StopContainerRequest, StreamLogsRequest, WatchEventsRequest,
};

/// Container service implementation.
#[derive(Debug, Default)]
pub struct ContainerServiceImpl;

#[tonic::async_trait]
impl ContainerService for ContainerServiceImpl {
    async fn list_containers(
        &self,
        _request: Request<ListContainersRequest>,
    ) -> Result<Response<ListContainersResponse>, Status> {
        // TODO: Integrate with bock runtime
        let response = ListContainersResponse { containers: vec![] };
        Ok(Response::new(response))
    }

    async fn get_container(
        &self,
        request: Request<GetContainerRequest>,
    ) -> Result<Response<Container>, Status> {
        let id = request.into_inner().id;
        // TODO: Load container from state
        Err(Status::not_found(format!("Container {} not found", id)))
    }

    async fn create_container(
        &self,
        request: Request<CreateContainerRequest>,
    ) -> Result<Response<Container>, Status> {
        let req = request.into_inner();
        tracing::info!(name = %req.name, image = %req.image, "Creating container via gRPC");

        // TODO: Create container using bock runtime
        let container = Container {
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
        // TODO: Start container
        Ok(Response::new(ContainerOperationResponse {
            success: true,
            message: format!("Container {} started", id),
        }))
    }

    async fn stop_container(
        &self,
        request: Request<StopContainerRequest>,
    ) -> Result<Response<ContainerOperationResponse>, Status> {
        let req = request.into_inner();
        tracing::info!(container = %req.id, timeout = %req.timeout_seconds, "Stopping container via gRPC");
        // TODO: Stop container
        Ok(Response::new(ContainerOperationResponse {
            success: true,
            message: format!("Container {} stopped", req.id),
        }))
    }

    async fn kill_container(
        &self,
        request: Request<KillContainerRequest>,
    ) -> Result<Response<ContainerOperationResponse>, Status> {
        let req = request.into_inner();
        tracing::info!(container = %req.id, signal = %req.signal, "Killing container via gRPC");
        // TODO: Kill container
        Ok(Response::new(ContainerOperationResponse {
            success: true,
            message: format!("Container {} killed with signal {}", req.id, req.signal),
        }))
    }

    async fn delete_container(
        &self,
        request: Request<ContainerIdRequest>,
    ) -> Result<Response<ContainerOperationResponse>, Status> {
        let id = request.into_inner().id;
        tracing::info!(container = %id, "Deleting container via gRPC");
        // TODO: Delete container
        Ok(Response::new(ContainerOperationResponse {
            success: true,
            message: format!("Container {} deleted", id),
        }))
    }

    type WatchEventsStream =
        std::pin::Pin<Box<dyn futures::Stream<Item = Result<ContainerEvent, Status>> + Send>>;

    async fn watch_events(
        &self,
        _request: Request<WatchEventsRequest>,
    ) -> Result<Response<Self::WatchEventsStream>, Status> {
        // TODO: Implement event streaming
        let stream = futures::stream::empty();
        Ok(Response::new(Box::pin(stream)))
    }

    type StreamLogsStream =
        std::pin::Pin<Box<dyn futures::Stream<Item = Result<LogEntry, Status>> + Send>>;

    async fn stream_logs(
        &self,
        _request: Request<StreamLogsRequest>,
    ) -> Result<Response<Self::StreamLogsStream>, Status> {
        // TODO: Implement log streaming
        let stream = futures::stream::empty();
        Ok(Response::new(Box::pin(stream)))
    }
}

/// Create the gRPC server.
pub fn grpc_server() -> ContainerServiceServer<ContainerServiceImpl> {
    ContainerServiceServer::new(ContainerServiceImpl)
}
