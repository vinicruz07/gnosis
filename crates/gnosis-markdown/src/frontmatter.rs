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
    /// Arquivo de sticker ou imagem vetorial (`sticker: custom.svg`).
    pub sticker: Option<String>,
    /// Cor personalizada associada à nota (`color: #ff5555`).
    pub color: Option<String>,
    /// Campos customizados arbitrários adicionados pelo usuário.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_yaml::Value>,
}

impl Frontmatter {
    /// Retorna o ícone visual prioritário (ícone explícito ou sticker).
    pub fn visual_icon(&self) -> Option<&str> {
        self.icon.as_deref().or(self.sticker.as_deref())
    }

    /// Retorna a cor personalizada associada à nota, se definida.
    pub fn color(&self) -> Option<&str> {
        self.color.as_deref()
    }
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
            "sticker" => {
                frontmatter.sticker = v.as_str().map(|s| s.to_string());
            }
            "color" => {
                frontmatter.color = v.as_str().map(|s| s.to_string());
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

/// Insere ou atualiza os metadados de sticker, ícone e cor no conteúdo bruto Markdown.
///
/// Se o arquivo já contiver frontmatter delimitado por `---`, atualiza ou adiciona as chaves
/// preservando o corpo e os outros campos YAML intactos. Se não houver frontmatter, cria um novo
/// bloco YAML delimitado por `---` no topo do documento.
pub fn update_frontmatter_stickers(
    raw_content: &str,
    icon: Option<&str>,
    sticker: Option<&str>,
    color: Option<&str>,
) -> String {
    let trimmed = raw_content.trim_start();
    if !trimmed.starts_with("---") {
        let mut yaml_block = String::new();
        if let Some(ic) = icon {
            if !ic.is_empty() {
                yaml_block.push_str(&format!("icon: {}\n", ic));
            }
        }
        if let Some(st) = sticker {
            if !st.is_empty() {
                yaml_block.push_str(&format!("sticker: {}\n", st));
            }
        }
        if let Some(co) = color {
            if !co.is_empty() {
                if co.starts_with('#') {
                    yaml_block.push_str(&format!("color: \"{}\"\n", co));
                } else {
                    yaml_block.push_str(&format!("color: {}\n", co));
                }
            }
        }

        if yaml_block.is_empty() {
            return raw_content.to_string();
        }

        let mut result = String::new();
        result.push_str("---\n");
        result.push_str(&yaml_block);
        result.push_str("---\n");
        if !raw_content.starts_with('\n') && !raw_content.is_empty() {
            result.push('\n');
        }
        result.push_str(raw_content);
        return result;
    }

    let after_first_delim = match trimmed[3..]
        .strip_prefix('\r')
        .unwrap_or(&trimmed[3..])
        .strip_prefix('\n')
    {
        Some(rest) => rest,
        None => return raw_content.to_string(),
    };

    let mut end_idx = None;
    let mut search_offset = 0;

    while let Some(pos) = after_first_delim[search_offset..].find("---") {
        let abs_pos = search_offset + pos;
        let before_pos = &after_first_delim[..abs_pos];
        let after_pos = &after_first_delim[abs_pos + 3..];

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
        None => return raw_content.to_string(),
    };

    let yaml_str = &after_first_delim[..closing_pos];
    let mut remainder = &after_first_delim[closing_pos + 3..];

    if let Some(r) = remainder.strip_prefix("\r\n") {
        remainder = r;
    } else if let Some(r) = remainder.strip_prefix('\n') {
        remainder = r;
    }

    let mut found_icon = false;
    let mut found_sticker = false;
    let mut found_color = false;

    let mut new_lines = Vec::new();

    for line in yaml_str.lines() {
        let line_trimmed = line.trim();
        if line_trimmed.starts_with("icon:") {
            found_icon = true;
            if let Some(ic) = icon {
                if !ic.is_empty() {
                    new_lines.push(format!("icon: {}", ic));
                }
            }
        } else if line_trimmed.starts_with("sticker:") {
            found_sticker = true;
            if let Some(st) = sticker {
                if !st.is_empty() {
                    new_lines.push(format!("sticker: {}", st));
                }
            }
        } else if line_trimmed.starts_with("color:") {
            found_color = true;
            if let Some(co) = color {
                if !co.is_empty() {
                    if co.starts_with('#') {
                        new_lines.push(format!("color: \"{}\"", co));
                    } else {
                        new_lines.push(format!("color: {}", co));
                    }
                }
            }
        } else {
            new_lines.push(line.to_string());
        }
    }

    if !found_icon {
        if let Some(ic) = icon {
            if !ic.is_empty() {
                new_lines.push(format!("icon: {}", ic));
            }
        }
    }
    if !found_sticker {
        if let Some(st) = sticker {
            if !st.is_empty() {
                new_lines.push(format!("sticker: {}", st));
            }
        }
    }
    if !found_color {
        if let Some(co) = color {
            if !co.is_empty() {
                if co.starts_with('#') {
                    new_lines.push(format!("color: \"{}\"", co));
                } else {
                    new_lines.push(format!("color: {}", co));
                }
            }
        }
    }

    let mut result = String::new();
    result.push_str("---\n");
    for l in new_lines {
        result.push_str(&l);
        result.push('\n');
    }
    result.push_str("---\n");
    result.push_str(remainder);

    result
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
        let text = r##"---
title: Minha Nota Incrível
tags:
  - rust
  - obsidian/recomp
aliases: [Gnosis, Recomp]
icon: 🧠
sticker: custom_vault.svg
color: "#ff5555"
custom_status: active
---
# Conteúdo da Nota
Texto principal aqui.
"##;

        let (fm_opt, remainder) = parse_frontmatter(text);
        assert!(fm_opt.is_some());

        let fm = match fm_opt {
            Some(fm) => fm,
            None => return,
        };

        assert_eq!(fm.title.as_deref(), Some("Minha Nota Incrível"));
        assert_eq!(fm.icon.as_deref(), Some("🧠"));
        assert_eq!(fm.sticker.as_deref(), Some("custom_vault.svg"));
        assert_eq!(fm.color.as_deref(), Some("#ff5555"));
        assert_eq!(fm.visual_icon(), Some("🧠"));
        assert_eq!(fm.tags, vec!["rust", "obsidian/recomp"]);
        assert_eq!(fm.aliases, vec!["Gnosis", "Recomp"]);
        assert!(remainder.starts_with("# Conteúdo da Nota"));
    }

    #[test]
    fn test_update_frontmatter_stickers_existing() {
        let text = r#"---
title: Nota
icon: 🧠
---
# Corpo
"#;

        let updated = update_frontmatter_stickers(text, Some("🦀"), Some("ferris.svg"), Some("#3399ff"));
        let (fm_opt, remainder) = parse_frontmatter(&updated);
        assert!(fm_opt.is_some());
        let fm = match fm_opt {
            Some(fm) => fm,
            None => return,
        };
        assert_eq!(fm.title.as_deref(), Some("Nota"));
        assert_eq!(fm.icon.as_deref(), Some("🦀"));
        assert_eq!(fm.sticker.as_deref(), Some("ferris.svg"));
        assert_eq!(fm.color.as_deref(), Some("#3399ff"));
        assert!(remainder.starts_with("# Corpo"));
    }

    #[test]
    fn test_update_frontmatter_stickers_new() {
        let text = "# Sem Frontmatter\nApenas markdown.";
        let updated = update_frontmatter_stickers(text, Some("🚀"), None, None);
        let (fm_opt, remainder) = parse_frontmatter(&updated);
        assert!(fm_opt.is_some());
        let fm = match fm_opt {
            Some(fm) => fm,
            None => return,
        };
        assert_eq!(fm.icon.as_deref(), Some("🚀"));
        assert_eq!(fm.visual_icon(), Some("🚀"));
        assert!(remainder.contains("# Sem Frontmatter"));
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

