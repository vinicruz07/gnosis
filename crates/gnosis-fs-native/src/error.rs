use thiserror::Error;

pub type Result<T> = std::result::Result<T, NativeFsError>;

#[derive(Debug, Error)]
pub enum NativeFsError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Path not found: {0}")]
    NotFound(std::path::PathBuf),

    #[error("Unable to resolve executable directory for portable mode")]
    ExecutablePathError,

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Core error: {0}")]
    Core(#[from] gnosis_core::CoreError),

    #[error("Trash error: {0}")]
    Trash(String),
}

impl From<serde_json::Error> for NativeFsError {
    fn from(err: serde_json::Error) -> Self {
        Self::Serialization(err.to_string())
    }
}
