//! Network namespace operations.

use bock_common::BockResult;

/// Create a new network namespace.
pub fn create_netns(name: &str) -> BockResult<()> {
    tracing::debug!(name, "Creating network namespace");
    // TODO: Implement
    Ok(())
}

/// Delete a network namespace.
pub fn delete_netns(name: &str) -> BockResult<()> {
    tracing::debug!(name, "Deleting network namespace");
    // TODO: Implement
    Ok(())
}

/// Enter a network namespace.
pub fn enter_netns(name: &str) -> BockResult<()> {
    tracing::debug!(name, "Entering network namespace");
    // TODO: Implement
    Ok(())
}
