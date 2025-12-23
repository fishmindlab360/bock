//! Container state persistence.

use bock_common::BockResult;
use bock_oci::ContainerState;

/// Manages container state persistence.
#[derive(Debug)]
pub struct StateManager {
    /// Base path for state files.
    state_dir: std::path::PathBuf,
}

impl StateManager {
    /// Create a new state manager.
    pub fn new(state_dir: impl Into<std::path::PathBuf>) -> Self {
        Self {
            state_dir: state_dir.into(),
        }
    }

    /// Get the path to a container's state file.
    pub fn state_path(&self, container_id: &str) -> std::path::PathBuf {
        self.state_dir.join(container_id).join("state.json")
    }

    /// Save container state.
    pub fn save(&self, state: &ContainerState) -> BockResult<()> {
        let path = self.state_path(&state.id);

        // Create directory if it doesn't exist
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let json = serde_json::to_string_pretty(state)?;
        std::fs::write(&path, json)?;

        tracing::debug!(
            container_id = %state.id,
            path = %path.display(),
            "Saved container state"
        );

        Ok(())
    }

    /// Load container state.
    pub fn load(&self, container_id: &str) -> BockResult<ContainerState> {
        let path = self.state_path(container_id);

        if !path.exists() {
            return Err(bock_common::BockError::ContainerNotFound {
                id: container_id.to_string(),
            });
        }

        let json = std::fs::read_to_string(&path)?;
        let state: ContainerState = serde_json::from_str(&json)?;

        tracing::debug!(
            container_id = %container_id,
            path = %path.display(),
            "Loaded container state"
        );

        Ok(state)
    }

    /// Delete container state.
    pub fn delete(&self, container_id: &str) -> BockResult<()> {
        let container_dir = self.state_dir.join(container_id);

        if container_dir.exists() {
            std::fs::remove_dir_all(&container_dir)?;
            tracing::debug!(
                container_id = %container_id,
                path = %container_dir.display(),
                "Deleted container state"
            );
        }

        Ok(())
    }

    /// List all containers.
    pub fn list(&self) -> BockResult<Vec<String>> {
        let mut containers = Vec::new();

        if !self.state_dir.exists() {
            return Ok(containers);
        }

        for entry in std::fs::read_dir(&self.state_dir)? {
            let entry = entry?;
            if entry.path().is_dir() {
                if let Some(name) = entry.file_name().to_str() {
                    // Check if state.json exists
                    if entry.path().join("state.json").exists() {
                        containers.push(name.to_string());
                    }
                }
            }
        }

        Ok(containers)
    }

    /// Check if a container exists.
    pub fn exists(&self, container_id: &str) -> bool {
        self.state_path(container_id).exists()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn save_and_load_state() {
        let temp = tempdir().unwrap();
        let manager = StateManager::new(temp.path());

        let state = ContainerState::new("test-container", "/bundle");
        manager.save(&state).unwrap();

        let loaded = manager.load("test-container").unwrap();
        assert_eq!(loaded.id, "test-container");
    }

    #[test]
    fn list_containers() {
        let temp = tempdir().unwrap();
        let manager = StateManager::new(temp.path());

        let state1 = ContainerState::new("container-1", "/bundle");
        let state2 = ContainerState::new("container-2", "/bundle");
        manager.save(&state1).unwrap();
        manager.save(&state2).unwrap();

        let containers = manager.list().unwrap();
        assert_eq!(containers.len(), 2);
        assert!(containers.contains(&"container-1".to_string()));
        assert!(containers.contains(&"container-2".to_string()));
    }

    #[test]
    fn delete_state() {
        let temp = tempdir().unwrap();
        let manager = StateManager::new(temp.path());

        let state = ContainerState::new("test-container", "/bundle");
        manager.save(&state).unwrap();
        assert!(manager.exists("test-container"));

        manager.delete("test-container").unwrap();
        assert!(!manager.exists("test-container"));
    }
}
