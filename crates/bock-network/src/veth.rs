//! Virtual ethernet pair management.

use bock_common::BockResult;

/// Virtual ethernet pair.
pub struct VethPair {
    /// Host-side interface name.
    pub host: String,
    /// Container-side interface name.
    pub container: String,
}

impl VethPair {
    /// Create a new veth pair.
    pub async fn create(host_name: &str, container_name: &str) -> BockResult<Self> {
        tracing::debug!(host_name, container_name, "Creating veth pair");
        // TODO: Implement using rtnetlink
        Ok(Self {
            host: host_name.to_string(),
            container: container_name.to_string(),
        })
    }

    /// Move the container side to a network namespace.
    pub async fn move_to_netns(&self, pid: u32) -> BockResult<()> {
        tracing::debug!(interface = %self.container, pid, "Moving to netns");
        // TODO: Implement
        Ok(())
    }

    /// Delete the veth pair.
    pub async fn delete(&self) -> BockResult<()> {
        tracing::debug!(host = %self.host, "Deleting veth pair");
        // TODO: Implement
        Ok(())
    }
}
