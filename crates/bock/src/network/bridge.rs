//! Linux bridge management.

use bock_common::BockResult;

/// Default bridge name.
pub const DEFAULT_BRIDGE: &str = "bock0";

/// Create a Linux bridge.
pub fn create_bridge(name: &str) -> BockResult<()> {
    tracing::debug!(name, "Creating bridge");
    // TODO: Implement using rtnetlink
    Ok(())
}

/// Add an interface to a bridge.
pub fn add_to_bridge(bridge: &str, interface: &str) -> BockResult<()> {
    tracing::debug!(bridge, interface, "Adding interface to bridge");
    // TODO: Implement
    Ok(())
}

/// Delete a bridge.
pub fn delete_bridge(name: &str) -> BockResult<()> {
    tracing::debug!(name, "Deleting bridge");
    // TODO: Implement
    Ok(())
}

/// Setup NAT for the bridge.
pub fn setup_nat(bridge: &str, subnet: &str) -> BockResult<()> {
    tracing::debug!(bridge, subnet, "Setting up NAT");
    // TODO: Implement using iptables
    Ok(())
}
