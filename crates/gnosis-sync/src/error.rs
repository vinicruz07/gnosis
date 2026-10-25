//! Gestão de erros tipados da camada de sincronização e colaboração (`gnosis-sync`).

use thiserror::Error;

pub type Result<T> = std::result::Result<T, SyncError>;

#[derive(Debug, Error)]
pub enum SyncError {
    #[error("Core error: {0}")]
    Core(#[from] gnosis_core::CoreError),

    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("Código de sala ou chave inválida: {0}")]
    InvalidRoom(String),

    #[error("Erro de protocolo de sincronização: {0}")]
    Protocol(String),

    #[error("Erro na árvore Merkle: {0}")]
    Merkle(String),

    #[error("Falha criptográfica E2EE: {0}")]
    Crypto(String),

    #[error("Falha no handshake de autenticação da sala: {0}")]
    HandshakeFailed(String),
}

impl From<serde_json::Error> for SyncError {
    fn from(err: serde_json::Error) -> Self {
        Self::Serialization(err.to_string())
    }
}
