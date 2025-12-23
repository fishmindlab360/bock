//! Linux bridge management.

use bock_common::BockResult;

/// Bridge manager.
pub struct BridgeManager {
    /// Bridge name.
    name: String,
}

impl BridgeManager {
    /// Create a new bridge.
    pub async fn create(name: &str) -> BockResult<Self> {
        tracing::debug!(name, "Creating bridge");
        // TODO: Implement using rtnetlink
        Ok(Self { name: name.to_string() })
    }

    /// Add an interface to the bridge.
    pub async fn add_interface(&self, interface: &str) -> BockResult<()> {
        tracing::debug!(bridge = %self.name, interface, "Adding interface to bridge");
        // TODO: Implement
        Ok(())
    }

    /// Delete the bridge.
    pub async fn delete(&self) -> BockResult<()> {
        tracing::debug!(name = %self.name, "Deleting bridge");
        // TODO: Implement
        Ok(())
    }
}
