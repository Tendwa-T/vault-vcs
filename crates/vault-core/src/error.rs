use thiserror::Error;

#[derive(Debug, Error)]
pub enum VaultError {
    #[error("Not a vault repository (no .vault directory found)")]
    NotARepo,

    #[error("Object Not found: {0}")]
    ObjectNotFound(String),

    #[error("Branch not found: {0}")]
    BranchNotFound(String),

    #[error("Branch already exists")]
    BranchExists(String),

    #[error("Nothing to undo - operation log is empty")]
    NothingToUndo,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Repository already initialised at {0}")]
    AlreadyInit(String),
}

pub type Result<T> = std::result::Result<T, VaultError>;
