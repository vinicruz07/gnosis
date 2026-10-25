use thiserror::Error;

pub type Result<T> = std::result::Result<T, CoreError>;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Vault not found at path: {0}")]
    VaultNotFound(std::path::PathBuf),

    #[error("Invalid vault configuration: {0}")]
    InvalidConfig(String),
}
