use oci_spec::OciSpecError;
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
    #[error("Hook error: {0}")]
    Hook(String),

    #[error("Invalid arguments: {0}")]
    InvalidArgs(String),

    #[error("Invalid bundle path at {path}: {reason}")]
    InvalidBundle { path: PathBuf, reason: String },

    #[error("Config error: {0}")]
    InvalidSpec(String),

    #[error("State file not found for container {id}")]
    ContainerStateNotFound { id: String },

    #[error("Container '{id}' not found")]
    ContainerNotFound { id: String },

    #[error("Container '{id}' is already running")]
    ContainerAlreadyExists { id: String },

    #[error("Error with config specification: {0}")]
    OciSpecError(#[from] OciSpecError),

    // --- Subsystem Specific Errors ---
    #[error("Namespace error: {0}")]
    Namespace(String),

    #[error("User namespace error: {0}")]
    Userns(String),

    #[error("Network namespace error: {0}")]
    Netns(String),

    #[error("Cgroup error: {0}")]
    Cgroup(String),

    #[error("Mount error: {0}")]
    Mount(String),

    #[error("Mount failed for target '{target}': {source}")]
    MountFailed {
        target: String,
        #[source]
        source: nix::Error,
    },

    #[error("Capability error: {0}")]
    Capability(String),

    #[error("Rlimits error: {0}")]
    Rlimits(String),

    #[error("No new privileges error: {0}")]
    NoNewPrivs(String),

    #[error("Seccomp error: {0}")]
    Seccomp(String),

    // --- Executation & Process Sync Errors ---
    #[error("Synchronization pipe error: {0}")]
    SyncPipe(String),

    #[error("Command execution failed: {0}")]
    ExecFailed(String),

    // --- Command Errors ---
    #[error("Start error: {0}")]
    Start(String),

    // --- Catch-all Dynamic Error Wrapper ---
    #[error(transparent)]
    Other(#[from] Box<dyn std::error::Error + Send + Sync>),
}

// --- Validation Utils ---
pub fn validate_id(container_id: &str) -> Result<()> {
    if container_id.trim().is_empty() {
        return Err(KuroError::InvalidArgs(
            "container_id cannot be empty".to_string(),
        ));
    }
    Ok(())
}
