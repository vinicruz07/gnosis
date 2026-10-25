//! Extrator de Hashtags no padrão Obsidian (`#tag`, `#sub/tag`).

/// Varre o texto Markdown e extrai todas as hashtags válidas, ignorando cabeçalhos e blocos de código.
pub fn scan_tags(content: &str) -> Vec<String> {
    let mut tags = Vec::new();
    let mut in_code_block = false;
    let mut in_inline_code = false;

    let chars: Vec<char> = content.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        // Detecção de blocos de código com cercas triplas ```
        if i + 2 < len && chars[i] == '`' && chars[i + 1] == '`' && chars[i + 2] == '`' {
            in_code_block = !in_code_block;
            i += 3;
            continue;
        }

        // Detecção de código inline `
        if !in_code_block && chars[i] == '`' {
            in_inline_code = !in_inline_code;
            i += 1;
            continue;
        }

        if in_code_block || in_inline_code {
            i += 1;
            continue;
        }

        // Verifica se encontramos um caractere '#'
        if chars[i] == '#' {
            // Verifica o caractere anterior: deve ser início de linha, espaço ou pontuação
            let is_valid_start = if i == 0 {
                true
            } else {
                let prev = chars[i - 1];
                prev.is_whitespace() || prev == '(' || prev == '[' || prev == '{' || prev == ','
            };

            // O caractere seguinte NÃO pode ser espaço (cabeçalho Markdown `# Título`) nem '#'
            let has_valid_next = if i + 1 < len {
                let next = chars[i + 1];
                !next.is_whitespace() && next != '#' && next != '!' && next != '?'
            } else {
                false
            };

            if is_valid_start && has_valid_next {
                let tag_start = i + 1;
                let mut tag_end = tag_start;
                let mut has_alphabetic = false;

                while tag_end < len {
                    let c = chars[tag_end];
                    if c.is_alphanumeric() || c == '_' || c == '-' || c == '/' {
                        if c.is_alphabetic() {
                            has_alphabetic = true;
                        }
                        tag_end += 1;
                    } else {
                        break;
                    }
                }

                // Não pode terminar em '/' nem ser puramente numérico (como #123)
                let candidate: String = chars[tag_start..tag_end].iter().collect();
                let clean_candidate = candidate.trim_end_matches('/');

                if has_alphabetic && !clean_candidate.is_empty() {
                    // Ignora cores hexadecimais (3 ou 6 dígitos hexadecimais como #fff, #1e1e2e)
                    let is_hex_color = (clean_candidate.len() == 3 || clean_candidate.len() == 6 || clean_candidate.len() == 8)
                        && clean_candidate.chars().all(|c| c.is_ascii_hexdigit());

                    if !is_hex_color && !tags.contains(&clean_candidate.to_string()) {
                        tags.push(clean_candidate.to_string());
                    }
                }

                i = tag_end;
                continue;
            }
        }

        i += 1;
    }

    tags
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_tags() {
        let text = r#"
# Título Principal (Não deve ser tag)
## Subtítulo

Aqui está uma nota sobre #rust e sobre #desenvolvimento/pkm.
Podemos ter tags com traço #meu-projeto_2026.
Não deve pegar #123 (apenas números).
Não deve pegar cor hex #ffffff ou #1e1e2e.
Em código `não deve pegar #tag_em_codigo`.
```rust
// Nem em blocos #tag_comentada
fn main() {}
```
Mas no final #tag_final funciona!
"#;

        let tags = scan_tags(text);
        assert_eq!(tags, vec![
            "rust",
            "desenvolvimento/pkm",
            "meu-projeto_2026",
            "tag_final"
        ]);
    }
}
