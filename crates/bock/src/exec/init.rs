//! Container init process.

use bock_common::BockResult;

/// Run as container init (PID 1).
pub fn container_init() -> BockResult<()> {
    tracing::debug!("Running as container init");
    // TODO: Implement
    Ok(())
}
