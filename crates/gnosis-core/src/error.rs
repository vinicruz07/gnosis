//! Gestão de erros tipados e exaustivos do Gnosis Core.
//!
//! Em conformidade com as diretrizes do projeto:
//! - Zero pânicos em tempo de execução.
//! - Nenhuma perda de contexto de erro.
//! - 100% Rust Seguro (`#![forbid(unsafe_code)]`).

use thiserror::Error;

/// Tipo padrão de resultado para todas as operações do núcleo do Gnosis.
pub type Result<T> = std::result::Result<T, CoreError>;

/// Enum exaustivo de erros possíveis nas operações do Vault e VFS.
#[derive(Debug, Error)]
pub enum CoreError {
    #[error("Tentativa de travessia ilegal de diretório (Path Traversal detectado): '{0}'")]
    PathTraversal(String),

    #[error("Caminho do Vault inválido: '{0}' (motivo: {1})")]
    InvalidPath(String, &'static str),

    #[error("Índice fora dos limites do buffer: posição {0}, tamanho total {1}")]
    IndexOutOfBounds(usize, usize),


    #[error("Recurso não encontrado no Vault: '{0}'")]
    NotFound(String),

    #[error("Recurso já existente no caminho: '{0}'")]
    AlreadyExists(String),

    #[error("Permissão negada para acessar o caminho: '{0}'")]
    PermissionDenied(String),

    #[error("O caminho esperado como arquivo é um diretório: '{0}'")]
    IsDirectory(String),

    #[error("O caminho esperado como diretório é um arquivo: '{0}'")]
    NotDirectory(String),

    #[error("Falha de I/O na camada de armazenamento: {0}")]
    Io(String),

    #[error("Erro de codificação UTF-8 no conteúdo da nota: {0}")]
    Utf8Error(String),

    #[error("Configuração do Vault corrompida ou inválida: {0}")]
    InvalidVaultConfig(String),

    #[error("Falha de serialização/deserialização: {0}")]
    Serialization(String),
}

impl From<std::io::Error> for CoreError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err.to_string())
    }
}

impl From<std::string::FromUtf8Error> for CoreError {
    fn from(err: std::string::FromUtf8Error) -> Self {
        Self::Utf8Error(err.to_string())
    }
}

impl From<serde_json::Error> for CoreError {
    fn from(err: serde_json::Error) -> Self {
        Self::Serialization(err.to_string())
    }
}
