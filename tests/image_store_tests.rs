//! Integration tests for bock-image.

use std::path::Path;
use tempfile::tempdir;

#[test]
fn test_image_store_lifecycle() {
    // Create a temporary store
    let temp = tempdir().unwrap();
    let store = bock_image::ImageStore::new(temp.path()).unwrap();

    // Initially empty
    let images = store.list().unwrap();
    assert!(images.is_empty());
}

#[test]
fn test_blob_storage() {
    let temp = tempdir().unwrap();
    let store = bock_image::ImageStore::new(temp.path()).unwrap();

    // Store a blob
    let data = b"test blob content";
    let digest = store.store_blob(data).unwrap();

    // Verify digest format
    assert!(digest.starts_with("sha256:"));

    // Retrieve blob
    let retrieved = store.get_blob(&digest).unwrap().unwrap();
    assert_eq!(retrieved, data);

    // Check existence
    assert!(store.has_blob(&digest));
    assert!(!store.has_blob("sha256:nonexistent"));
}

#[test]
fn test_credential_store() {
    let temp = tempdir().unwrap();
    let path = temp.path().join("creds.json");

    let mut store = bock_image::FileCredentialStore::new(&path).unwrap();

    // Store credential
    let cred = bock_image::Credential::new("ghcr.io", "user", "token");
    store.store(cred).unwrap();

    // Retrieve credential
    let loaded = store.get("ghcr.io").unwrap().unwrap();
    assert_eq!(loaded.username, "user");

    // List registries
    let registries = store.list().unwrap();
    assert!(registries.contains(&"ghcr.io".to_string()));

    // Delete credential
    store.delete("ghcr.io").unwrap();
    assert!(store.get("ghcr.io").unwrap().is_none());
}

#[test]
fn test_docker_auth_encoding() {
    let cred = bock_image::Credential::new("docker.io", "testuser", "testpass");
    let auth = cred.to_docker_auth();

    // Decode and verify
    let decoded = bock_image::Credential::from_docker_auth("docker.io", &auth).unwrap();
    assert_eq!(decoded.username, "testuser");
    assert_eq!(decoded.password, Some("testpass".to_string()));
}

#[test]
fn test_image_reference_parsing() {
    // These would use the ImageReference parser if available
    // For now, test basic parsing logic

    fn parse_ref(r: &str) -> (String, String) {
        if let Some(idx) = r.rfind(':') {
            let potential_tag = &r[idx + 1..];
            if !potential_tag.contains('/') && potential_tag.parse::<u16>().is_err() {
                return (r[..idx].to_string(), potential_tag.to_string());
            }
        }
        (r.to_string(), "latest".to_string())
    }

    let (repo, tag) = parse_ref("alpine");
    assert_eq!(repo, "alpine");
    assert_eq!(tag, "latest");

    let (repo, tag) = parse_ref("nginx:1.21");
    assert_eq!(repo, "nginx");
    assert_eq!(tag, "1.21");

    let (repo, tag) = parse_ref("ghcr.io/user/image:v1");
    assert_eq!(repo, "ghcr.io/user/image");
    assert_eq!(tag, "v1");
}
