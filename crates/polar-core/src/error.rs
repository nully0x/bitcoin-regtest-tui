//! Error types for Polar.

use thiserror::Error;

/// Result type alias using [`Error`].
pub type Result<T> = std::result::Result<T, Error>;

/// Errors that can occur in Polar.
#[derive(Debug, Error)]
pub enum Error {
    /// Network not found.
    #[error("network not found: {0}")]
    NetworkNotFound(String),

    /// Node not found.
    #[error("node not found: {0}")]
    NodeNotFound(String),

    /// Docker error.
    #[error("docker error: {0}")]
    Docker(String),

    /// Configuration error.
    #[error("config error: {0}")]
    Config(String),

    /// IO error.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization error.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}
