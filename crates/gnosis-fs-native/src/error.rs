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
}
