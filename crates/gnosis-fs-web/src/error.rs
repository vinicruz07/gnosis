use thiserror::Error;

pub type Result<T> = std::result::Result<T, WebFsError>;

#[derive(Debug, Error)]
pub enum WebFsError {
    #[error("OPFS storage error: {0}")]
    Opfs(String),

    #[error("File not found in web storage: {0}")]
    NotFound(String),

    #[error("Browser permission denied for file system access")]
    PermissionDenied,
}
