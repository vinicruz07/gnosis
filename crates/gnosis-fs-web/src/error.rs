//! Gestão de erros tipados para a camada Web VFS (OPFS & IndexedDB).
//!
//! Em conformidade com as diretrizes do projeto:
//! - 100% Rust Seguro (`#![forbid(unsafe_code)]`).
//! - Sem perda de contexto de erro.
//! - Conversão transparente para `gnosis_core::CoreError`.

use gnosis_core::error::CoreError;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, WebFsError>;

/// Enum exaustivo de erros possíveis nas operações de armazenamento no navegador.
#[derive(Debug, Error)]
pub enum WebFsError {
    #[error("Erro na API Origin Private File System (OPFS): {0}")]
    Opfs(String),

    #[error("Erro no armazenamento IndexedDB do navegador: {0}")]
    IndexedDb(String),

    #[error("Arquivo ou diretório web não encontrado: '{0}'")]
    NotFound(String),

    #[error("Caminho já existe no armazenamento web: '{0}'")]
    AlreadyExists(String),

    #[error("Permissão de acesso ao sistema de arquivos do navegador negada: '{0}'")]
    PermissionDenied(String),

    #[error("O caminho esperado como arquivo é uma pasta: '{0}'")]
    IsDirectory(String),

    #[error("O caminho esperado como pasta é um arquivo: '{0}'")]
    NotDirectory(String),

    #[error("Falha de serialização/deserialização do armazenamento web: {0}")]
    Serialization(String),

    #[error("Dados corrompidos ou inválidos no payload web: {0}")]
    CorruptedPayload(String),

    #[error("Erro de I/O na camada web: {0}")]
    Io(String),
}

impl From<WebFsError> for CoreError {
    fn from(err: WebFsError) -> Self {
        match err {
            WebFsError::NotFound(p) => CoreError::NotFound(p),
            WebFsError::AlreadyExists(p) => CoreError::AlreadyExists(p),
            WebFsError::PermissionDenied(p) => CoreError::PermissionDenied(p),
            WebFsError::IsDirectory(p) => CoreError::IsDirectory(p),
            WebFsError::NotDirectory(p) => CoreError::NotDirectory(p),
            WebFsError::Serialization(s) => CoreError::Serialization(s),
            WebFsError::CorruptedPayload(s) => CoreError::Serialization(s),
            WebFsError::Opfs(s) => CoreError::Io(format!("OPFS: {}", s)),
            WebFsError::IndexedDb(s) => CoreError::Io(format!("IndexedDB: {}", s)),
            WebFsError::Io(s) => CoreError::Io(s),
        }
    }
}
