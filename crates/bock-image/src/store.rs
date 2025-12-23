//! Image store.

use std::path::PathBuf;

use bock_common::BockResult;

/// Local image store.
pub struct ImageStore {
    /// Storage root directory.
    #[allow(dead_code)]
    root: PathBuf,
}

impl ImageStore {
    /// Create a new image store.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// List all stored images.
    pub fn list(&self) -> BockResult<Vec<String>> {
        // TODO: Implement
        Ok(Vec::new())
    }

    /// Get an image by reference.
    pub fn get(&self, reference: &str) -> BockResult<Option<StoredImage>> {
        tracing::debug!(reference, "Getting image");
        // TODO: Implement
        Ok(None)
    }

    /// Store an image.
    pub fn store(&self, image: &StoredImage) -> BockResult<()> {
        tracing::debug!(reference = %image.reference, "Storing image");
        // TODO: Implement
        Ok(())
    }

    /// Delete an image.
    pub fn delete(&self, reference: &str) -> BockResult<()> {
        tracing::debug!(reference, "Deleting image");
        // TODO: Implement
        Ok(())
    }
}

/// A stored image.
pub struct StoredImage {
    /// Image reference.
    pub reference: String,
    /// Manifest digest.
    pub digest: String,
    /// Layer digests.
    pub layers: Vec<String>,
}
