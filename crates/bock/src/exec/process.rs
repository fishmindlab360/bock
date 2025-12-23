#![allow(unsafe_code)]
//! Process spawning.

use bock_common::BockResult;

use std::os::unix::process::CommandExt;
use std::process::Command;

/// Spawn a new process with container setup.
pub fn spawn_process<F>(args: &[String], env: &[(String, String)], setup: F) -> BockResult<u32>
where
    F: Fn() -> std::io::Result<()> + Send + Sync + 'static,
{
    tracing::debug!(?args, "Spawning process");

    if args.is_empty() {
        return Err(bock_common::BockError::Config {
            message: "No command specified".to_string(),
        });
    }

    let mut cmd = Command::new(&args[0]);
    cmd.args(&args[1..]);
    cmd.envs(env.iter().cloned());

    unsafe {
        cmd.pre_exec(setup);
    }

    let child = cmd.spawn().map_err(|e| bock_common::BockError::Io(e))?;

    Ok(child.id())
}
