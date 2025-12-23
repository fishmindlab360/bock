//! OCI lifecycle hooks.

use bock_common::BockResult;

/// Run prestart hooks.
pub fn run_prestart_hooks() -> BockResult<()> {
    tracing::debug!("Running prestart hooks");
    Ok(())
}

/// Run poststart hooks.
pub fn run_poststart_hooks() -> BockResult<()> {
    tracing::debug!("Running poststart hooks");
    Ok(())
}

/// Run poststop hooks.
pub fn run_poststop_hooks() -> BockResult<()> {
    tracing::debug!("Running poststop hooks");
    Ok(())
}
