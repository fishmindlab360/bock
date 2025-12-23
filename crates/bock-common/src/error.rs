//! Common error types for the Bock ecosystem.

use miette::Diagnostic;
use thiserror::Error;

/// Result type alias using [`BockError`].
pub type BockResult<T> = Result<T, BockError>;

/// Common errors across the Bock ecosystem.
#[derive(Error, Diagnostic, Debug)]
pub enum BockError {
    /// Container not found.
    #[error("Container not found: {id}")]
    #[diagnostic(code(bock::container::not_found))]
    ContainerNotFound {
        /// The container ID that was not found.
        id: String,
    },

    /// Image not found.
    #[error("Image not found: {reference}")]
    #[diagnostic(code(bock::image::not_found))]
    ImageNotFound {
        /// The image reference that was not found.
        reference: String,
    },

    /// Invalid container ID format.
    #[error("Invalid container ID: {id}")]
    #[diagnostic(
        code(bock::container::invalid_id),
        help("Container IDs must be alphanumeric with hyphens and underscores, 1-64 characters")
    )]
    InvalidContainerId {
        /// The invalid container ID.
        id: String,
    },

    /// Invalid resource quantity format.
    #[error("Invalid resource quantity: {value}")]
    #[diagnostic(
        code(bock::resource::invalid_quantity),
        help("Use formats like '512m', '1g', '2.5Gi', '500Mi', '0.5' for CPU cores")
    )]
    InvalidResourceQuantity {
        /// The invalid value.
        value: String,
    },

    /// I/O error.
    #[error("I/O error: {0}")]
    #[diagnostic(code(bock::io))]
    Io(#[from] std::io::Error),

    /// Serialization error.
    #[error("Serialization error: {0}")]
    #[diagnostic(code(bock::serialization))]
    Serialization(String),

    /// Permission denied.
    #[error("Permission denied: {operation}")]
    #[diagnostic(
        code(bock::permission_denied),
        help("Try running with elevated privileges (sudo)")
    )]
    PermissionDenied {
        /// The operation that was denied.
        operation: String,
    },

    /// Feature not supported on this platform.
    #[error("Feature not supported: {feature}")]
    #[diagnostic(
        code(bock::unsupported),
        help("This feature requires Linux kernel 5.10 or later")
    )]
    Unsupported {
        /// The unsupported feature.
        feature: String,
    },

    /// Configuration error.
    #[error("Configuration error: {message}")]
    #[diagnostic(code(bock::config))]
    Config {
        /// The error message.
        message: String,
    },

    /// Internal error (should not happen).
    #[error("Internal error: {message}")]
    #[diagnostic(
        code(bock::internal),
        help("This is a bug, please report it at https://github.com/bock-containers/bock/issues")
    )]
    Internal {
        /// The error message.
        message: String,
    },
}

impl From<serde_json::Error> for BockError {
    fn from(err: serde_json::Error) -> Self {
        BockError::Serialization(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_display() {
        let err = BockError::ContainerNotFound {
            id: "abc123".to_string(),
        };
        assert_eq!(err.to_string(), "Container not found: abc123");
    }

    #[test]
    fn error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file not found");
        let err: BockError = io_err.into();
        assert!(matches!(err, BockError::Io(_)));
    }
}
