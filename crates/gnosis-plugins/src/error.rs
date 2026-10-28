//! Gestão de erros tipados do subsistema de plugins.
//!
//! Em conformidade com as diretrizes do Gnosis:
//! - 100% Rust Seguro (`#![forbid(unsafe_code)]`).
//! - Zero pânicos em tempo de execução.
//! - Nenhuma perda de contexto de erro.

use thiserror::Error;

pub type Result<T> = std::result::Result<T, PluginError>;

/// Enum exaustivo de erros possíveis durante a descoberta, carregamento e execução de plugins.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PluginError {
    #[error("Manifesto 'plugin.json' não encontrado para o plugin '{0}'")]
    ManifestNotFound(String),

    #[error("Manifesto de plugin inválido ou corrompido: {0}")]
    InvalidManifest(String),

    #[error("Plugin com ID '{0}' já está registrado no gerenciador")]
    DuplicatePlugin(String),

    #[error("Plugin '{0}' não foi encontrado")]
    PluginNotFound(String),

    #[error("Plugin '{0}' já se encontra carregado e ativo")]
    AlreadyLoaded(String),

    #[error("Plugin '{0}' não está carregado")]
    NotLoaded(String),

    #[error("Falha no ciclo de vida do plugin '{0}': {1}")]
    LifecycleError(String, String),

    #[error("Permissão '{0}' requerida pelo plugin '{1}' não foi concedida")]
    PermissionDenied(String, String),

    #[error("Erro de I/O no sistema de arquivos ao acessar plugin: {0}")]
    Io(String),

    #[error("Falha de serialização/deserialização: {0}")]
    Serialization(String),
}

impl From<serde_json::Error> for PluginError {
    fn from(err: serde_json::Error) -> Self {
        Self::Serialization(err.to_string())
    }
}

impl From<gnosis_core::error::CoreError> for PluginError {
    fn from(err: gnosis_core::error::CoreError) -> Self {
        Self::Io(err.to_string())
    }
}
