//! Image reference parsing.

use std::str::FromStr;

use bock_common::BockResult;

/// A parsed image reference.
#[derive(Debug, Clone)]
pub struct ImageReference {
    /// Registry hostname.
    pub registry: String,
    /// Repository name.
    pub repository: String,
    /// Tag or digest.
    pub reference: ImageTag,
}

/// Image tag or digest.
#[derive(Debug, Clone)]
pub enum ImageTag {
    /// A tag (e.g., "latest").
    Tag(String),
    /// A digest (e.g., "sha256:abc123...").
    Digest(String),
}

impl ImageReference {
    /// Default registry.
    pub const DEFAULT_REGISTRY: &'static str = "docker.io";
    /// Default tag.
    pub const DEFAULT_TAG: &'static str = "latest";

    /// Parse an image reference string.
    ///
    /// Examples:
    /// - `alpine` -> docker.io/library/alpine:latest
    /// - `alpine:3.19` -> docker.io/library/alpine:3.19
    /// - `myuser/myapp` -> docker.io/myuser/myapp:latest
    /// - `ghcr.io/org/app:v1.0` -> ghcr.io/org/app:v1.0
    pub fn parse(reference: &str) -> BockResult<Self> {
        let reference = reference.trim();

        // Check for digest
        let (name, tag) = if let Some(idx) = reference.find('@') {
            let (name, digest) = reference.split_at(idx);
            (name, ImageTag::Digest(digest[1..].to_string()))
        } else if let Some(idx) = reference.rfind(':') {
            // Check if this is a port number (registry:port/image)
            let potential_tag = &reference[idx + 1..];
            if potential_tag.contains('/') {
                (reference, ImageTag::Tag(Self::DEFAULT_TAG.to_string()))
            } else {
                let (name, tag) = reference.split_at(idx);
                (name, ImageTag::Tag(tag[1..].to_string()))
            }
        } else {
            (reference, ImageTag::Tag(Self::DEFAULT_TAG.to_string()))
        };

        // Parse registry and repository
        let (registry, repository) = if name.contains('/') {
            let first_slash = name.find('/').unwrap();
            let potential_registry = &name[..first_slash];

            // Check if it looks like a registry (has dots or is localhost)
            if potential_registry.contains('.') || potential_registry == "localhost" {
                (
                    potential_registry.to_string(),
                    name[first_slash + 1..].to_string(),
                )
            } else {
                // It's a Docker Hub user/repo
                (Self::DEFAULT_REGISTRY.to_string(), name.to_string())
            }
        } else {
            // Official image (e.g., "alpine" -> "library/alpine")
            (
                Self::DEFAULT_REGISTRY.to_string(),
                format!("library/{}", name),
            )
        };

        Ok(Self {
            registry,
            repository,
            reference: tag,
        })
    }

    /// Get the full reference string.
    #[must_use]
    pub fn full_reference(&self) -> String {
        let tag = match &self.reference {
            ImageTag::Tag(t) => format!(":{}", t),
            ImageTag::Digest(d) => format!("@{}", d),
        };
        format!("{}/{}{}", self.registry, self.repository, tag)
    }
}

impl FromStr for ImageReference {
    type Err = bock_common::BockError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::parse(s)
    }
}

impl std::fmt::Display for ImageReference {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.full_reference())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple() {
        let ref_ = ImageReference::parse("alpine").unwrap();
        assert_eq!(ref_.registry, "docker.io");
        assert_eq!(ref_.repository, "library/alpine");
        assert!(matches!(ref_.reference, ImageTag::Tag(t) if t == "latest"));
    }

    #[test]
    fn parse_with_tag() {
        let ref_ = ImageReference::parse("alpine:3.19").unwrap();
        assert_eq!(ref_.registry, "docker.io");
        assert_eq!(ref_.repository, "library/alpine");
        assert!(matches!(ref_.reference, ImageTag::Tag(t) if t == "3.19"));
    }

    #[test]
    fn parse_user_repo() {
        let ref_ = ImageReference::parse("myuser/myapp").unwrap();
        assert_eq!(ref_.registry, "docker.io");
        assert_eq!(ref_.repository, "myuser/myapp");
    }

    #[test]
    fn parse_custom_registry() {
        let ref_ = ImageReference::parse("ghcr.io/org/app:v1.0").unwrap();
        assert_eq!(ref_.registry, "ghcr.io");
        assert_eq!(ref_.repository, "org/app");
        assert!(matches!(ref_.reference, ImageTag::Tag(t) if t == "v1.0"));
    }
}
