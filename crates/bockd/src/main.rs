//! bockd - Bock daemon.
//!
//! Provides both HTTP REST API and gRPC API for container management.

use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod api;
mod grpc;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// HTTP port to listen on
    #[arg(long, default_value_t = 8080)]
    http_port: u16,

    /// gRPC port to listen on
    #[arg(long, default_value_t = 50051)]
    grpc_port: u16,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let args = Args::parse();

    // Spawn HTTP server
    let http_addr = std::net::SocketAddr::from(([0, 0, 0, 0], args.http_port));
    let http_app = api::server::app().await;

    let http_handle = tokio::spawn(async move {
        tracing::info!("HTTP server listening on {}", http_addr);
        let listener = tokio::net::TcpListener::bind(http_addr).await.unwrap();
        axum::serve(listener, http_app).await.unwrap();
    });

    // Spawn gRPC server
    let grpc_addr = std::net::SocketAddr::from(([0, 0, 0, 0], args.grpc_port));

    let grpc_handle = tokio::spawn(async move {
        tracing::info!("gRPC server listening on {}", grpc_addr);
        tonic::transport::Server::builder()
            .add_service(grpc::grpc_server())
            .serve(grpc_addr)
            .await
            .unwrap();
    });

    tracing::info!(
        "bockd started - HTTP: {}, gRPC: {}",
        args.http_port,
        args.grpc_port
    );

    // Wait for both servers
    tokio::select! {
        _ = http_handle => {
            tracing::error!("HTTP server exited unexpectedly");
        }
        _ = grpc_handle => {
            tracing::error!("gRPC server exited unexpectedly");
        }
    }

    Ok(())
}
