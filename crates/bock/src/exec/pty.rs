//! PTY (pseudo-terminal) handling.

use bock_common::BockResult;

/// Allocate a PTY.
pub fn allocate_pty() -> BockResult<PtyPair> {
    tracing::debug!("Allocating PTY");
    // TODO: Implement
    Ok(PtyPair {
        master: 0,
        slave: 0,
    })
}

/// PTY file descriptor pair.
pub struct PtyPair {
    /// Master file descriptor.
    pub master: i32,
    /// Slave file descriptor.
    pub slave: i32,
}
