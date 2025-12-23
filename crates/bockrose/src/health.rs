//! Health check monitoring.

use bock_common::BockResult;

/// Health check monitor.
pub struct HealthMonitor {
    /// Check interval in seconds.
    interval: u64,
}

impl HealthMonitor {
    /// Create a new health monitor.
    pub fn new(interval: u64) -> Self {
        Self { interval }
    }

    /// Run a health check command.
    pub async fn check_cmd(&self, container_id: &str, cmd: &[String]) -> BockResult<bool> {
        tracing::debug!(container_id, ?cmd, "Running health check");
        // TODO: Implement
        Ok(true)
    }

    /// Run an HTTP health check.
    pub async fn check_http(&self, container_id: &str, url: &str) -> BockResult<bool> {
        tracing::debug!(container_id, url, "Running HTTP health check");
        // TODO: Implement
        Ok(true)
    }
}
