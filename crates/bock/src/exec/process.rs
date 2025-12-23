//! Process spawning.

use bock_common::BockResult;

/// Spawn a new process.
pub fn spawn_process(args: &[String], env: &[String]) -> BockResult<u32> {
    tracing::debug!(?args, "Spawning process");
    // TODO: Implement
    Ok(0)
}
