//! Integration tests for container PID persistence.
use bock::runtime::{Container, RuntimeConfig};
use bock_oci::Spec;
use std::error::Error;
use tempfile::TempDir;

#[tokio::test]
async fn test_pid_persistence() -> Result<(), Box<dyn Error>> {
    // Setup
    let temp_dir = TempDir::new()?;
    let root = temp_dir.path().join("bock-root");
    let config = RuntimeConfig::default().with_root(root);

    let bundle_path = temp_dir.path().join("bundle");
    let rootfs = bundle_path.join("rootfs");
    std::fs::create_dir_all(&rootfs)?;

    // Create config.json
    let spec = Spec::default();
    let config_path = bundle_path.join("config.json");
    std::fs::write(&config_path, serde_json::to_string(&spec)?)?;

    // Create container
    let spec = Spec::default();
    let container =
        Container::create("test-persistence", &bundle_path, &spec, config.clone()).await?;
    let id = container.id().as_str();

    // Verify initial PID is None
    assert!(container.pid().await.is_none());

    // Manually inject a PID file to simulate a running container
    // We didn't "start" it because that requires valid rootfs binaries.
    // We are testing that if 'pid' file exists, it is loaded.
    let container_dir = config.paths.container(id);
    std::fs::write(container_dir.join("pid"), "12345")?;

    // Reload container (simulate CLI restart)
    let loaded_container = Container::load(id, config).await?;

    // But wait, 'load' just loads state. It does NOT automatically read PID file into memory.
    // The `pid()` method or `kill()` method reads it lazily?
    // Let's check `kill` or `pid` implementation.
    // In strict implementation:
    // `pid()` method just returns `*self.pid.lock().await`. It does NOT read from file.
    // `kill()` method reads from file if memory is None.

    // So `loaded_container.pid()` will be None initially?
    assert!(loaded_container.pid().await.is_none());

    // But if we try to kill it, it should find the PID.
    // Note: kill(12345) might fail with EPERM, but it shouldn't fail with "No PID found".
    // Or we can mock the PID to be our own PID so kill(0) works?
    // Let's use our own PID for safety.
    let my_pid = std::process::id();
    std::fs::write(container_dir.join("pid"), my_pid.to_string())?;

    // Attempt to kill with signal 0 (check existence)
    let res = loaded_container.kill(0).await;

    // This should succeed if it found the PID.
    assert!(res.is_ok(), "Kill(0) failed: {:?}", res.err());

    // After kill, the PID might be cached in memory?
    assert_eq!(loaded_container.pid().await, Some(my_pid));

    Ok(())
}
