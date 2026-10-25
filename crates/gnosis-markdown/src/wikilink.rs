use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WikiLink {
    /// O destino do link (ex: "Notas/Ideias" em "[[Notas/Ideias|Meu Título]]")
    pub target: String,
    /// Texto alternativo de exibição caso especificado após o pipe "|"
    pub alias: Option<String>,
    /// Link para cabeçalho ou bloco (ex: "#Seção" ou "^block-id")
    pub anchor: Option<String>,
    /// Se é um embed com prefixo de exclamação (ex: "![[Imagem.png]]")
    pub is_embed: bool,
}
