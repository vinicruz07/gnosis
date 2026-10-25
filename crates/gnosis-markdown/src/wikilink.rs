//! Scanner e extrator de Wikilinks e Embeds no formato padrão do Obsidian.

use serde::{Deserialize, Serialize};

/// Representa um link bidirecional ou anexo embutido (`[[...]]` ou `![[...]]`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WikiLink {
    /// O caminho ou nome da nota/anexo de destino (ex: `"Projetos/Gnosis"`).
    pub target: String,
    /// Texto alternativo exibido após o pipe (ex: `"Gnosis Rust"` em `[[Projetos/Gnosis|Gnosis Rust]]`).
    pub alias: Option<String>,
    /// Âncora de cabeçalho ou identificador de bloco (ex: `"#Arquitetura"` ou `"^block-123"`).
    pub anchor: Option<String>,
    /// Se é um embed com ponto de exclamação no início (`![[...]]`).
    pub is_embed: bool,
    /// Posição inicial (em bytes) no texto de origem.
    pub span_start: usize,
    /// Posição final (em bytes) no texto de origem.
    pub span_end: usize,
}

impl WikiLink {
    /// Faz o parse do conteúdo interno entre colchetes duplos `[[` e `]]`.
    pub fn parse_inner(inner: &str, is_embed: bool, span_start: usize, span_end: usize) -> Option<Self> {
        let trimmed = inner.trim();
        if trimmed.is_empty() {
            return None;
        }

        // Separa o alias se houver pipe '|'
        let (path_and_anchor, alias) = match trimmed.split_once('|') {
            Some((target_part, alias_part)) => {
                let alias_trimmed = alias_part.trim();
                let alias_opt = if alias_trimmed.is_empty() {
                    None
                } else {
                    Some(alias_trimmed.to_string())
                };
                (target_part.trim(), alias_opt)
            }
            None => (trimmed, None),
        };

        // Separa a âncora de cabeçalho ou bloco se houver '#'
        let (target, anchor) = match path_and_anchor.split_once('#') {
            Some((target_part, anchor_part)) => {
                let a = anchor_part.trim();
                let anchor_opt = if a.is_empty() {
                    None
                } else {
                    Some(format!("#{}", a))
                };
                (target_part.trim(), anchor_opt)
            }
            None => (path_and_anchor, None),
        };

        if target.is_empty() && anchor.is_none() {
            return None;
        }

        Some(Self {
            target: target.to_string(),
            alias,
            anchor,
            is_embed,
            span_start,
            span_end,
        })
    }
}

/// Varre o texto e extrai todos os Wikilinks e Embeds válidos.
pub fn scan_wikilinks(content: &str) -> Vec<WikiLink> {
    let mut links = Vec::new();
    let bytes = content.as_bytes();
    let len = bytes.len();
    let mut i = 0;

    while i < len {
        // Verifica se inicia um embed `![[` ou wikilink `[[`
        let is_embed = bytes[i] == b'!' && i + 2 < len && bytes[i + 1] == b'[' && bytes[i + 2] == b'[';
        let is_link = !is_embed && bytes[i] == b'[' && i + 1 < len && bytes[i + 1] == b'[';

        if is_embed || is_link {
            let start_pos = i;
            let content_start = if is_embed { i + 3 } else { i + 2 };

            // Procura o fechamento `]]`
            if let Some(close_offset) = content[content_start..].find("]]") {
                let content_end = content_start + close_offset;
                let end_pos = content_end + 2;

                let inner = &content[content_start..content_end];
                if let Some(link) = WikiLink::parse_inner(inner, is_embed, start_pos, end_pos) {
                    links.push(link);
                }

                i = end_pos;
                continue;
            }
        }

        i += 1;
    }

    links
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_various_wikilinks() {
        let text = r#"
Este é um link simples para [[Minha Nota]].
Aqui está um link com alias [[Projetos/Rust|Linguagem Rust]].
Aqui está um link com cabeçalho [[Arquitetura#Visão Geral]].
Aqui está um link com cabeçalho e alias [[Documento#Capítulo 1|Introdução]].
Aqui está um embed de imagem ![[assets/diagrama.png|500]].
E um embed de nota ![[Resumo]].
"#;

        let links = scan_wikilinks(text);
        assert_eq!(links.len(), 6);

        // 1. Link simples
        assert_eq!(links[0].target, "Minha Nota");
        assert_eq!(links[0].alias, None);
        assert_eq!(links[0].anchor, None);
        assert!(!links[0].is_embed);

        // 2. Link com alias
        assert_eq!(links[1].target, "Projetos/Rust");
        assert_eq!(links[1].alias.as_deref(), Some("Linguagem Rust"));
        assert!(!links[1].is_embed);

        // 3. Link com âncora
        assert_eq!(links[2].target, "Arquitetura");
        assert_eq!(links[2].anchor.as_deref(), Some("#Visão Geral"));

        // 4. Link com âncora e alias
        assert_eq!(links[3].target, "Documento");
        assert_eq!(links[3].anchor.as_deref(), Some("#Capítulo 1"));
        assert_eq!(links[3].alias.as_deref(), Some("Introdução"));

        // 5. Embed com tamanho
        assert_eq!(links[4].target, "assets/diagrama.png");
        assert_eq!(links[4].alias.as_deref(), Some("500"));
        assert!(links[4].is_embed);

        // 6. Embed de nota
        assert_eq!(links[5].target, "Resumo");
        assert!(links[5].is_embed);
    }
}
