//! User namespace handling.

use bock_common::BockResult;

/// Setup user namespace ID mappings.
#[allow(dead_code)]
pub fn setup_user_namespace(pid: u32, uid: u32, gid: u32) -> BockResult<()> {
    tracing::debug!(pid, uid, gid, "Setting up user namespace");
    // TODO: Implement user namespace setup
    Ok(())
}
