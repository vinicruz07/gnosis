//! Extração e parser resiliente de Frontmatter YAML no padrão Obsidian.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Estrutura tipada de metadados extraídos do cabeçalho YAML delimitado por `---`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Frontmatter {
    /// Título opcional definido explicitamente no frontmatter (`title: "..."`).
    pub title: Option<String>,
    /// Lista de tags declaradas no frontmatter (`tags: [a, b]` ou `- a`).
    pub tags: Vec<String>,
    /// Lista de aliases/apelidos da nota (`aliases: [nome1, nome2]`).
    pub aliases: Vec<String>,
    /// Ícone ou sticker da nota (`icon: 🧠`).
    pub icon: Option<String>,
    /// Campos customizados arbitrários adicionados pelo usuário.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_yaml::Value>,
}

/// Extrai o frontmatter YAML inicial do arquivo, se presente, e retorna o restante do corpo Markdown.
///
/// Não entra em pânico se o YAML for malformado — retorna `None` e o conteúdo original intacto.
pub fn parse_frontmatter(content: &str) -> (Option<Frontmatter>, &str) {
    let trimmed = content.trim_start();
    if !trimmed.starts_with("---") {
        return (None, content);
    }

    // O delimitador inicial deve estar isolado na primeira linha
    let after_first_delim = match trimmed[3..].strip_prefix('\r').unwrap_or(&trimmed[3..]).strip_prefix('\n') {
        Some(rest) => rest,
        None => return (None, content),
    };

    // Procura o delimitador de fechamento `\n---`
    let mut end_idx = None;
    let mut search_offset = 0;

    while let Some(pos) = after_first_delim[search_offset..].find("---") {
        let abs_pos = search_offset + pos;
        let before_pos = &after_first_delim[..abs_pos];
        let after_pos = &after_first_delim[abs_pos + 3..];

        // Deve ser precedido por quebra de linha (ou ser início) e seguido por quebra de linha ou fim de texto
        let is_start_of_line = abs_pos == 0 || before_pos.ends_with('\n');
        let is_end_of_line = after_pos.is_empty()
            || after_pos.starts_with('\n')
            || after_pos.starts_with("\r\n");

        if is_start_of_line && is_end_of_line {
            end_idx = Some(abs_pos);
            break;
        }

        search_offset = abs_pos + 3;
    }

    let closing_pos = match end_idx {
        Some(pos) => pos,
        None => return (None, content),
    };

    let yaml_str = &after_first_delim[..closing_pos];
    let mut remainder = &after_first_delim[closing_pos + 3..];

    // Remove quebra de linha imediatamente após o fechamento
    if let Some(r) = remainder.strip_prefix("\r\n") {
        remainder = r;
    } else if let Some(r) = remainder.strip_prefix('\n') {
        remainder = r;
    }

    // Faz o parse do YAML
    let raw_value: serde_yaml::Value = match serde_yaml::from_str(yaml_str) {
        Ok(v) => v,
        Err(_) => return (None, content),
    };

    let mapping = match raw_value.as_mapping() {
        Some(m) => m,
        None => return (None, content),
    };

    let mut frontmatter = Frontmatter::default();

    for (k, v) in mapping {
        let key_str = match k.as_str() {
            Some(s) => s,
            None => continue,
        };

        match key_str {
            "title" => {
                frontmatter.title = v.as_str().map(|s| s.to_string());
            }
            "icon" => {
                frontmatter.icon = v.as_str().map(|s| s.to_string());
            }
            "tags" | "tag" => {
                extract_string_or_list(v, &mut frontmatter.tags);
            }
            "aliases" | "alias" => {
                extract_string_or_list(v, &mut frontmatter.aliases);
            }
            other => {
                let _ = frontmatter.extra.insert(other.to_string(), v.clone());
            }
        }
    }

    (Some(frontmatter), remainder)
}

fn extract_string_or_list(value: &serde_yaml::Value, dest: &mut Vec<String>) {
    if let Some(seq) = value.as_sequence() {
        for item in seq {
            if let Some(s) = item.as_str() {
                let cleaned = s.trim().trim_start_matches('#');
                if !cleaned.is_empty() && !dest.contains(&cleaned.to_string()) {
                    dest.push(cleaned.to_string());
                }
            }
        }
    } else if let Some(s) = value.as_str() {
        // Suporte a tags separadas por vírgula ou espaço no frontmatter
        for part in s.split([',', ' ']) {
            let cleaned = part.trim().trim_start_matches('#');
            if !cleaned.is_empty() && !dest.contains(&cleaned.to_string()) {
                dest.push(cleaned.to_string());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_frontmatter() {
        let text = r#"---
title: Minha Nota Incrível
tags:
  - rust
  - obsidian/recomp
aliases: [Gnosis, Recomp]
icon: 🧠
custom_status: active
---
# Conteúdo da Nota
Texto principal aqui.
"#;

        let (fm_opt, remainder) = parse_frontmatter(text);
        assert!(fm_opt.is_some());

        let fm = match fm_opt {
            Some(fm) => fm,
            None => return,
        };

        assert_eq!(fm.title.as_deref(), Some("Minha Nota Incrível"));
        assert_eq!(fm.icon.as_deref(), Some("🧠"));
        assert_eq!(fm.tags, vec!["rust", "obsidian/recomp"]);
        assert_eq!(fm.aliases, vec!["Gnosis", "Recomp"]);
        assert!(remainder.starts_with("# Conteúdo da Nota"));
    }

    #[test]
    fn test_missing_frontmatter() {
        let text = "# Apenas Markdown\nSem frontmatter aqui.";
        let (fm, remainder) = parse_frontmatter(text);
        assert!(fm.is_none());
        assert_eq!(remainder, text);
    }

    #[test]
    fn test_malformed_frontmatter_graceful() {
        let text = "---\n: YAML TOTALMENTE INVALIDO [[[\n---\n# Corpo";
        let (fm, remainder) = parse_frontmatter(text);
        assert!(fm.is_none());
        assert_eq!(remainder, text);
    }
}
