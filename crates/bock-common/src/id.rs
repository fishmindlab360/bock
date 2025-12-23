//! Container and image ID generation and validation.

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

use crate::error::{BockError, BockResult};

/// A validated container ID.
///
/// Container IDs must:
/// - Be 1-64 characters long
/// - Contain only alphanumeric characters, hyphens, and underscores
/// - Start with an alphanumeric character
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ContainerId(String);

impl ContainerId {
    /// Maximum length of a container ID.
    pub const MAX_LENGTH: usize = 64;

    /// Create a new container ID, validating the format.
    ///
    /// # Errors
    ///
    /// Returns an error if the ID format is invalid.
    pub fn new(id: impl Into<String>) -> BockResult<Self> {
        let id = id.into();
        Self::validate(&id)?;
        Ok(Self(id))
    }

    /// Generate a new random container ID.
    ///
    /// The ID is a 12-character hex string derived from a UUID v4.
    #[must_use]
    pub fn generate() -> Self {
        let uuid = uuid::Uuid::new_v4();
        let hex = hex::encode(&uuid.as_bytes()[..6]);
        Self(hex)
    }

    /// Create a container ID without validation.
    ///
    /// # Safety
    ///
    /// The caller must ensure the ID is valid.
    #[must_use]
    pub fn new_unchecked(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Get the container ID as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Validate a container ID string.
    fn validate(id: &str) -> BockResult<()> {
        if id.is_empty() || id.len() > Self::MAX_LENGTH {
            return Err(BockError::InvalidContainerId { id: id.to_string() });
        }

        let first_char = id.chars().next().unwrap();
        if !first_char.is_ascii_alphanumeric() {
            return Err(BockError::InvalidContainerId { id: id.to_string() });
        }

        for c in id.chars() {
            if !c.is_ascii_alphanumeric() && c != '-' && c != '_' {
                return Err(BockError::InvalidContainerId { id: id.to_string() });
            }
        }

        Ok(())
    }

    /// Returns a short version of the ID (first 12 characters).
    #[must_use]
    pub fn short(&self) -> &str {
        if self.0.len() <= 12 {
            &self.0
        } else {
            &self.0[..12]
        }
    }
}

impl fmt::Display for ContainerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl FromStr for ContainerId {
    type Err = BockError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl AsRef<str> for ContainerId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// A content-addressable digest (e.g., sha256:abc123...).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Digest {
    /// The algorithm used (e.g., "sha256").
    pub algorithm: String,
    /// The hex-encoded hash.
    pub hash: String,
}

impl Digest {
    /// Create a new digest.
    #[must_use]
    pub fn new(algorithm: impl Into<String>, hash: impl Into<String>) -> Self {
        Self {
            algorithm: algorithm.into(),
            hash: hash.into(),
        }
    }

    /// Create a SHA-256 digest.
    #[must_use]
    pub fn sha256(hash: impl Into<String>) -> Self {
        Self::new("sha256", hash)
    }
}

impl fmt::Display for Digest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.algorithm, self.hash)
    }
}

impl FromStr for Digest {
    type Err = BockError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.splitn(2, ':').collect();
        if parts.len() != 2 {
            return Err(BockError::InvalidContainerId { id: s.to_string() });
        }
        Ok(Self::new(parts[0], parts[1]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_container_ids() {
        assert!(ContainerId::new("abc123").is_ok());
        assert!(ContainerId::new("my-container").is_ok());
        assert!(ContainerId::new("my_container").is_ok());
        assert!(ContainerId::new("Container-123_test").is_ok());
    }

    #[test]
    fn invalid_container_ids() {
        assert!(ContainerId::new("").is_err());
        assert!(ContainerId::new("-invalid").is_err());
        assert!(ContainerId::new("_invalid").is_err());
        assert!(ContainerId::new("invalid!").is_err());
        assert!(ContainerId::new("a".repeat(65)).is_err());
    }

    #[test]
    fn generate_container_id() {
        let id1 = ContainerId::generate();
        let id2 = ContainerId::generate();
        assert_ne!(id1, id2);
        assert_eq!(id1.as_str().len(), 12);
    }

    #[test]
    fn digest_parsing() {
        let digest: Digest = "sha256:abc123def456".parse().unwrap();
        assert_eq!(digest.algorithm, "sha256");
        assert_eq!(digest.hash, "abc123def456");
        assert_eq!(digest.to_string(), "sha256:abc123def456");
    }
}
