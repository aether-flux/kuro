use std::path::PathBuf;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, KuroError>;

#[derive(Debug, Error)]
pub enum KuroError {
    // --- Systems/IO Errors ---
    #[error("IO error occurred: {0}")]
    Io(#[from] std::io::Error),

    #[error("System call failed: {0}")]
    Nix(#[from] nix::Error),

    #[error("JSON serialization/deserialization failed: {0}")]
    Json(#[from] serde_json::Error),

    // --- CLI & Configuration Errors ---
    #[error("Invalid arguments: {0}")]
    InvalidArgs(String),

    #[error("Invalid bundle path at {path}: {reason}")]
    InvalidBundle { path: PathBuf, reason: String },

    #[error("Container '{id}' not found")]
    ContainerNotFound { id: String },

    #[error("Container '{id}' is already running")]
    ContainerAlreadyExists { id: String },

    // --- Subsystem Specific Errors ---
    #[error("Namespace error: {0}")]
    Namespace(String),

    #[error("Cgroup error: {0}")]
    Cgroup(String),

    #[error("Mount failed for target '{target}': {source}")]
    MountFailed {
        target: String,
        #[source]
        source: nix::Error,
    },

    // --- Executation & Process Sync Errors ---
    #[error("Synchronization pipe error: {0}")]
    SyncPipe(String),

    #[error("Command execution failed: {0}")]
    ExecFailed(String),

    // --- Catch-all Dynamic Error Wrapper ---
    #[error(transparent)]
    Other(#[from] Box<dyn std::error::Error + Send + Sync>),
}
