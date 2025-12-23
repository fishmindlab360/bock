//! Volume management for bockrose.

use bock_common::BockResult;

/// Manage volumes for a stack.
pub struct VolumeManager {
    /// Stack name prefix.
    prefix: String,
}

impl VolumeManager {
    /// Create a new volume manager.
    pub fn new(stack_name: &str) -> Self {
        Self {
            prefix: stack_name.to_string(),
        }
    }

    /// Create a volume.
    pub async fn create(&self, name: &str) -> BockResult<String> {
        let full_name = format!("{}_{}", self.prefix, name);
        tracing::info!(volume = %full_name, "Creating volume");
        // TODO: Implement
        Ok(full_name)
    }

    /// Delete a volume.
    pub async fn delete(&self, name: &str) -> BockResult<()> {
        let full_name = format!("{}_{}", self.prefix, name);
        tracing::info!(volume = %full_name, "Deleting volume");
        // TODO: Implement
        Ok(())
    }
}
