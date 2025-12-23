//! Virtual ethernet pair management.

use bock_common::BockResult;

/// Create a veth pair.
pub fn create_veth_pair(name1: &str, name2: &str) -> BockResult<()> {
    tracing::debug!(name1, name2, "Creating veth pair");
    // TODO: Implement using rtnetlink
    Ok(())
}

/// Move a veth interface to a network namespace.
pub fn move_to_netns(interface: &str, pid: u32) -> BockResult<()> {
    tracing::debug!(interface, pid, "Moving interface to network namespace");
    // TODO: Implement using rtnetlink
    Ok(())
}

/// Delete a veth interface.
pub fn delete_veth(name: &str) -> BockResult<()> {
    tracing::debug!(name, "Deleting veth interface");
    // TODO: Implement
    Ok(())
}
