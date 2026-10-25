//! Orquestrador de extração de metadados, links e tags de uma nota Markdown.

use crate::frontmatter::{parse_frontmatter, Frontmatter};
use crate::tag::scan_tags;
use crate::wikilink::{scan_wikilinks, WikiLink};
use serde::{Deserialize, Serialize};

/// Metadados e conexões extraídos de uma nota Markdown.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParsedMetadata {
    /// Frontmatter YAML opcional contido no cabeçalho.
    pub frontmatter: Option<Frontmatter>,
    /// Lista de todos os links bidirecionais e embeds encontrados.
    pub links: Vec<WikiLink>,
    /// Conjunto unificado de tags (frontmatter + corpo da nota, sem duplicatas).
    pub tags: Vec<String>,
    /// O corpo textual do documento limpo (sem o bloco de frontmatter inicial).
    pub body: String,
}

impl ParsedMetadata {
    /// Extrai todos os metadados, wikilinks e tags do texto bruto da nota.
    pub fn parse(raw_content: &str) -> Self {
        let (frontmatter, body) = parse_frontmatter(raw_content);

        let links = scan_wikilinks(body);
        let mut tags = scan_tags(body);

        // Se houver tags declaradas no frontmatter, adiciona sem duplicar
        if let Some(ref fm) = frontmatter {
            for fm_tag in &fm.tags {
                if !tags.contains(fm_tag) {
                    tags.push(fm_tag.clone());
                }
            }
        }

        Self {
            frontmatter,
            links,
            tags,
            body: body.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_metadata_full() {
        let note = r#"---
title: Arquitetura Gnosis
tags:
  - rust
  - obsidian
aliases:
  - Gnosis Arch
---
# Introdução
O projeto Gnosis é uma reimplementação em Rust de [[Obsidian PKM]].
Veja também o diagrama em ![[diagrama_vfs.png|400]] e #desenvolvimento.
"#;

        let parsed = ParsedMetadata::parse(note);

        // Frontmatter
        assert!(parsed.frontmatter.is_some());
        let fm = match parsed.frontmatter {
            Some(f) => f,
            None => return,
        };
        assert_eq!(fm.title.as_deref(), Some("Arquitetura Gnosis"));
        assert_eq!(fm.aliases, vec!["Gnosis Arch"]);

        // Links
        assert_eq!(parsed.links.len(), 2);
        assert_eq!(parsed.links[0].target, "Obsidian PKM");
        assert!(!parsed.links[0].is_embed);
        assert_eq!(parsed.links[1].target, "diagrama_vfs.png");
        assert!(parsed.links[1].is_embed);

        // Tags unificadas
        assert!(parsed.tags.contains(&"rust".to_string()));
        assert!(parsed.tags.contains(&"obsidian".to_string()));
        assert!(parsed.tags.contains(&"desenvolvimento".to_string()));

        // Body sem frontmatter
        assert!(parsed.body.starts_with("# Introdução"));
    }
}
