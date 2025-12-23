//! Image builder.

use std::path::PathBuf;

use bock_common::BockResult;

use crate::bockfile::Bockfile;

/// Image builder.
pub struct Builder {
    /// Bockfile specification.
    bockfile: Bockfile,
    /// Build context directory.
    context: PathBuf,
    /// Target tag.
    tag: String,
}

impl Builder {
    /// Create a new builder.
    pub fn new(bockfile: Bockfile, context: PathBuf, tag: String) -> Self {
        Self {
            bockfile,
            context,
            tag,
        }
    }

    /// Build the image.
    pub async fn build(&self) -> BockResult<String> {
        tracing::info!(tag = %self.tag, "Building image");

        // TODO: Implement build process
        // 1. Parse Bockfile
        // 2. Build dependency graph of stages
        // 3. Execute stages in parallel where possible
        // 4. Create layer for each step
        // 5. Generate OCI image

        Ok("sha256:placeholder".to_string())
    }
}
