use axum::{Json, Router, routing::get};
use serde_json::{Value, json};

pub async fn app() -> Router {
    Router::new()
        .route("/", get(root))
        .route("/version", get(version))
        .route("/containers", get(list_containers))
}

async fn root() -> Json<Value> {
    Json(json!({ "message": "bockd running" }))
}

async fn version() -> Json<Value> {
    Json(json!({ "version": env!("CARGO_PKG_VERSION") }))
}

async fn list_containers() -> Json<Value> {
    // TODO: Connect to bock runtime state
    Json(json!({ "containers": [] }))
}
