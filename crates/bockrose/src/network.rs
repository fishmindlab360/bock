//! Network management for bockrose.

use bock_common::BockResult;

/// Manage networks for a stack.
pub struct NetworkManager {
    /// Stack name prefix.
    prefix: String,
}

impl NetworkManager {
    /// Create a new network manager.
    pub fn new(stack_name: &str) -> Self {
        Self {
            prefix: stack_name.to_string(),
        }
    }

    /// Create a network.
    pub async fn create(&self, name: &str) -> BockResult<String> {
        let full_name = format!("{}_{}", self.prefix, name);
        tracing::info!(network = %full_name, "Creating network");
        // TODO: Implement
        Ok(full_name)
    }

    /// Delete a network.
    pub async fn delete(&self, name: &str) -> BockResult<()> {
        let full_name = format!("{}_{}", self.prefix, name);
        tracing::info!(network = %full_name, "Deleting network");
        // TODO: Implement
        Ok(())
    }
}
