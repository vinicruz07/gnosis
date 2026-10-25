//! Gestão de erros da camada de grafo de conhecimento (`gnosis-graph`).

use thiserror::Error;

pub type Result<T> = std::result::Result<T, GraphError>;

#[derive(Debug, Error)]
pub enum GraphError {
    #[error("Core error: {0}")]
    Core(#[from] gnosis_core::CoreError),

    #[error("Nó não encontrado no grafo: {0}")]
    NodeNotFound(String),

    #[error("Link ou aresta inválida no grafo: {0}")]
    InvalidLink(String),
}
