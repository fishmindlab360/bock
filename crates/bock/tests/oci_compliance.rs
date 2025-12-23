//! Integration tests for OCI compliance.
use bock::runtime::{Container, RuntimeConfig};
use bock_oci::{Spec, state::ContainerStatus};
use std::error::Error;
use tempfile::TempDir;

#[tokio::test]
async fn test_lifecycle_create_state_delete() -> Result<(), Box<dyn Error>> {
    // Setup
    let temp_dir = TempDir::new()?;
    let bundle_path = temp_dir.path().to_owned();
    let rootfs = bundle_path.join("rootfs");
    std::fs::create_dir(&rootfs)?;

    // Create config.json
    let spec = Spec::default();
    let config_path = bundle_path.join("config.json");
    std::fs::write(&config_path, serde_json::to_string(&spec)?)?;

    // Config paths
    let root = temp_dir.path().join("bock-root");
    let config = RuntimeConfig::default().with_root(root);

    // 1. Create
    let container =
        Container::create("test-container", &bundle_path, &spec, config.clone()).await?;

    assert_eq!(container.id().as_str(), "test-container");
    assert_eq!(container.status(), ContainerStatus::Creating);

    // Verify state matches OCI logic (Creating initially? Or Created after save?)
    // In bock::runtime::container::create, we save state immediately as Creating?
    // Let's check state.
    let state = container.state();
    assert_eq!(state.id, "test-container");
    assert_eq!(state.status, ContainerStatus::Creating);
    assert_eq!(state.bundle, bundle_path.display().to_string());

    // 2. Load (Simulate CLI state command)
    let loaded = Container::load("test-container", config.clone()).await?;
    assert_eq!(loaded.id().as_str(), "test-container");

    // 3. Delete
    container.delete().await?;

    // Verify deletion (should fail to load or directory gone)
    let res = Container::load("test-container", config).await;
    assert!(res.is_err());

    Ok(())
}

#[test]
fn test_rootless_namespace_config() {
    use bock::namespace::NamespaceConfig;
    use bock_oci::Spec;
    use bock_oci::runtime::{Linux, Namespace, NamespaceType};

    // Default spec usually implies host namespaces or empty?
    // We should check what OCI Spec default is.
    // If we manually construct a Spec with user namespace:
    let mut spec = Spec::default();
    spec.linux = Some(Linux {
        namespaces: vec![
            Namespace {
                ns_type: NamespaceType::User,
                path: None,
            },
            Namespace {
                ns_type: NamespaceType::Mount,
                path: None,
            },
        ],
        ..Default::default()
    });

    let ns_config = NamespaceConfig::from_spec(&spec);
    assert!(ns_config.user);
    assert!(ns_config.mount);
    assert!(!ns_config.net); // Not requested
}
