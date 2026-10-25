//! Tratamento de erros tipados para o parser de Markdown.

use thiserror::Error;

pub type Result<T> = std::result::Result<T, MarkdownError>;

#[derive(Debug, Error)]
pub enum MarkdownError {
    #[error("Falha ao analisar frontmatter YAML: {0}")]
    YamlError(String),

    #[error("Sintaxe Markdown inválida: {0}")]
    SyntaxError(String),
}

impl From<serde_yaml::Error> for MarkdownError {
    fn from(err: serde_yaml::Error) -> Self {
        Self::YamlError(err.to_string())
    }
}
