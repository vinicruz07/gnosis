//! Parser linear de blocos Markdown para a AST do Gnosis.
//!
//! Converte o corpo do documento em uma sequência de [`MarkdownBlock`],
//! pronta para alimentar diretamente a árvore de componentes visuais do Slint.

use crate::ast::{CalloutKind, MarkdownBlock, MarkdownDoc, TaskItem};
use crate::frontmatter::parse_frontmatter;
use crate::tag::scan_tags;
use crate::wikilink::scan_wikilinks;

/// Analisa o texto bruto de uma nota e constrói o [`MarkdownDoc`] completo.
pub fn parse_document(raw_content: &str) -> MarkdownDoc {
    let (frontmatter, body) = parse_frontmatter(raw_content);
    let all_links = scan_wikilinks(body);
    let mut all_tags = scan_tags(body);

    if let Some(ref fm) = frontmatter {
        for tag in &fm.tags {
            if !all_tags.contains(tag) {
                all_tags.push(tag.clone());
            }
        }
    }

    let blocks = parse_blocks(body);

    MarkdownDoc {
        frontmatter,
        blocks,
        all_links,
        all_tags,
    }
}

/// Divide o corpo do texto em blocos estruturados.
pub fn parse_blocks(body: &str) -> Vec<MarkdownBlock> {
    let mut blocks = Vec::new();
    let lines: Vec<&str> = body.lines().collect();
    let total_lines = lines.len();
    let mut i = 0;

    while i < total_lines {
        let line = lines[i];
        let trimmed = line.trim();

        // 1. Linhas vazias são ignoradas entre blocos
        if trimmed.is_empty() {
            i += 1;
            continue;
        }

        // 2. Divisores horizontais (`---`, `***`, `___`)
        if (trimmed == "---" || trimmed == "***" || trimmed == "___") && trimmed.len() >= 3 {
            blocks.push(MarkdownBlock::Divider);
            i += 1;
            continue;
        }

        // 3. Bloco de código delimitado por ```
        if trimmed.starts_with("```") {
            let lang_part = trimmed.trim_start_matches('`').trim();
            let language = if lang_part.is_empty() {
                None
            } else {
                Some(lang_part.to_string())
            };

            let mut code_lines = Vec::new();
            i += 1;

            while i < total_lines {
                let code_line = lines[i];
                if code_line.trim().starts_with("```") {
                    i += 1;
                    break;
                }
                code_lines.push(code_line);
                i += 1;
            }

            blocks.push(MarkdownBlock::CodeBlock {
                language,
                content: code_lines.join("\n"),
            });
            continue;
        }

        // 4. Bloco de matemática centralizado delimitado por $$
        if trimmed.starts_with("$$") {
            // Caso seja em uma única linha: `$$ formula $$`
            if trimmed.len() > 4 && trimmed.ends_with("$$") {
                let formula = trimmed[2..trimmed.len() - 2].trim().to_string();
                blocks.push(MarkdownBlock::MathBlock { formula });
                i += 1;
                continue;
            }

            let mut math_lines = Vec::new();
            i += 1;

            while i < total_lines {
                let math_line = lines[i];
                if math_line.trim().ends_with("$$") {
                    let end_trimmed = math_line.trim().trim_end_matches('$').trim();
                    if !end_trimmed.is_empty() {
                        math_lines.push(end_trimmed);
                    }
                    i += 1;
                    break;
                }
                math_lines.push(math_line);
                i += 1;
            }

            blocks.push(MarkdownBlock::MathBlock {
                formula: math_lines.join("\n"),
            });
            continue;
        }

        // 5. Cabeçalhos (# H1 até ###### H6)
        if trimmed.starts_with('#') {
            let mut level = 0;
            for c in trimmed.chars() {
                if c == '#' {
                    level += 1;
                } else {
                    break;
                }
            }

            if level >= 1 && level <= 6 {
                let rest = trimmed[level..].trim();
                // O caractere seguinte deve ser espaço para ser um cabeçalho Markdown válido
                if trimmed.chars().nth(level) == Some(' ') || rest.is_empty() {
                    // Extrai âncora opcional no final do cabeçalho (ex: `## Título ^ancora`)
                    let (header_text, anchor) = match rest.rsplit_once(" ^") {
                        Some((title, anc)) => (title.trim(), Some(anc.trim().to_string())),
                        None => (rest, None),
                    };

                    blocks.push(MarkdownBlock::Heading {
                        level: level as u8,
                        text: header_text.to_string(),
                        anchor,
                    });
                    i += 1;
                    continue;
                }
            }
        }

        // 6. Citações e Callouts (`> ...`)
        if trimmed.starts_with('>') {
            let quote_header = trimmed.trim_start_matches('>').trim();

            // Verifica se é um Callout no padrão Obsidian: `> [!NOTE] Título`
            if quote_header.starts_with("[!") {
                let (kind_str, title_str) = match quote_header.split_once(']') {
                    Some((k, t)) => (k, t.trim()),
                    None => (quote_header, ""),
                };

                let kind = CalloutKind::parse(kind_str);
                let title = if title_str.is_empty() {
                    kind.default_title().to_string()
                } else {
                    title_str.to_string()
                };

                let mut inner_lines = Vec::new();
                i += 1;

                while i < total_lines {
                    let next_line = lines[i];
                    let next_trimmed = next_line.trim();
                    if next_trimmed.starts_with('>') {
                        inner_lines.push(next_trimmed.trim_start_matches('>').trim());
                        i += 1;
                    } else if next_trimmed.is_empty() {
                        // Linha vazia dentro de citação
                        inner_lines.push("");
                        i += 1;
                    } else {
                        break;
                    }
                }

                let inner_body = inner_lines.join("\n");
                let inner_blocks = parse_blocks(&inner_body);

                blocks.push(MarkdownBlock::Callout {
                    kind,
                    title,
                    content: inner_blocks,
                });
                continue;
            }

            // Citação comum
            let mut quote_lines = Vec::new();
            while i < total_lines {
                let next_line = lines[i];
                let next_trimmed = next_line.trim();
                if next_trimmed.starts_with('>') {
                    quote_lines.push(next_trimmed.trim_start_matches('>').trim().to_string());
                    i += 1;
                } else {
                    break;
                }
            }

            blocks.push(MarkdownBlock::Blockquote { lines: quote_lines });
            continue;
        }

        // 7. Lista de Tarefas (`- [ ] ` ou `- [x] `)
        if is_task_line(trimmed) {
            let mut task_items = Vec::new();

            while i < total_lines {
                let current_line = lines[i].trim();
                if is_task_line(current_line) {
                    let checked = current_line.starts_with("- [x]")
                        || current_line.starts_with("- [X]")
                        || current_line.starts_with("* [x]")
                        || current_line.starts_with("* [X]");

                    let text = current_line[5..].trim().to_string();
                    let links = scan_wikilinks(&text);
                    let tags = scan_tags(&text);

                    task_items.push(TaskItem {
                        checked,
                        text,
                        links,
                        tags,
                    });
                    i += 1;
                } else {
                    break;
                }
            }

            blocks.push(MarkdownBlock::TaskList { items: task_items });
            continue;
        }

        // 8. Tabelas (`| Coluna 1 | Coluna 2 |`)
        if trimmed.starts_with('|') && trimmed.ends_with('|') && i + 1 < total_lines {
            let next_line = lines[i + 1].trim();
            // Verifica se a linha seguinte é o separador de tabela `|---|---|`
            if next_line.starts_with('|') && next_line.contains('-') && next_line.ends_with('|') {
                let headers = parse_table_row(trimmed);
                let mut rows = Vec::new();
                i += 2; // Pula o cabeçalho e o divisor

                while i < total_lines {
                    let row_line = lines[i].trim();
                    if row_line.starts_with('|') && row_line.ends_with('|') {
                        rows.push(parse_table_row(row_line));
                        i += 1;
                    } else {
                        break;
                    }
                }

                blocks.push(MarkdownBlock::Table { headers, rows });
                continue;
            }
        }

        // 9. Parágrafo comum (acumula linhas normais contíguas)
        let mut paragraph_lines = Vec::new();
        while i < total_lines {
            let p_line = lines[i];
            let p_trimmed = p_line.trim();

            if p_trimmed.is_empty()
                || p_trimmed.starts_with('#')
                || p_trimmed.starts_with('>')
                || p_trimmed.starts_with("```")
                || p_trimmed.starts_with("$$")
                || p_trimmed == "---"
                || is_task_line(p_trimmed)
                || (p_trimmed.starts_with('|') && p_trimmed.ends_with('|'))
            {
                break;
            }

            paragraph_lines.push(p_trimmed);
            i += 1;
        }

        if !paragraph_lines.is_empty() {
            let paragraph_text = paragraph_lines.join(" ");
            let links = scan_wikilinks(&paragraph_text);
            let tags = scan_tags(&paragraph_text);

            blocks.push(MarkdownBlock::Paragraph {
                text: paragraph_text,
                links,
                tags,
            });
        }
    }

    blocks
}

fn is_task_line(s: &str) -> bool {
    let t = s.trim();
    t.starts_with("- [ ]")
        || t.starts_with("- [x]")
        || t.starts_with("- [X]")
        || t.starts_with("* [ ]")
        || t.starts_with("* [x]")
        || t.starts_with("* [X]")
}

fn parse_table_row(row: &str) -> Vec<String> {
    let trimmed = row.trim();
    let inner = &trimmed[1..trimmed.len() - 1];
    inner
        .split('|')
        .map(|cell| cell.trim().to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_rich_document_blocks() {
        let note = r#"---
title: Nota de Teste
tags: [rust]
---
# Cabeçalho Principal

Este é um parágrafo inicial com [[Meu Link]] e uma tag #importante.

---

```rust
fn soma(a: i32, b: i32) -> i32 {
    a + b
}
```

> [!TIP] Dica de Performance
> Escreva código em Rust para atingir máxima velocidade.

- [ ] Tarefa pendente com [[Link Na Tarefa]]
- [x] Tarefa concluída #feita

$$
\int_{0}^{\infty} e^{-x^2} dx = \frac{\sqrt{\pi}}{2}
$$

| Nome | Cargo |
|---|---|
| Alice | Desenvolvedora |
| Bob | Designer |
"#;

        let doc = parse_document(note);

        // Valida cabeçalho
        assert!(doc.frontmatter.is_some());

        // Valida blocos
        assert!(doc.blocks.len() >= 8);

        // 1. Heading
        if let MarkdownBlock::Heading { level, text, .. } = &doc.blocks[0] {
            assert_eq!(*level, 1);
            assert_eq!(text, "Cabeçalho Principal");
        } else {
            assert!(false, "Esperava Heading");
        }

        // 2. Paragraph
        if let MarkdownBlock::Paragraph { text, links, tags } = &doc.blocks[1] {
            assert!(text.contains("Este é um parágrafo"));
            assert_eq!(links.len(), 1);
            assert_eq!(links[0].target, "Meu Link");
            assert_eq!(tags, &vec!["importante".to_string()]);
        } else {
            assert!(false, "Esperava Paragraph");
        }

        // 3. Divider
        assert_eq!(doc.blocks[2], MarkdownBlock::Divider);

        // 4. CodeBlock
        if let MarkdownBlock::CodeBlock { language, content } = &doc.blocks[3] {
            assert_eq!(language.as_deref(), Some("rust"));
            assert!(content.contains("fn soma"));
        } else {
            assert!(false, "Esperava CodeBlock");
        }

        // 5. Callout
        if let MarkdownBlock::Callout { kind, title, content } = &doc.blocks[4] {
            assert_eq!(*kind, CalloutKind::Tip);
            assert_eq!(title, "Dica de Performance");
            assert!(!content.is_empty());
        } else {
            assert!(false, "Esperava Callout");
        }

        // 6. TaskList
        if let MarkdownBlock::TaskList { items } = &doc.blocks[5] {
            assert_eq!(items.len(), 2);
            assert!(!items[0].checked);
            assert!(items[1].checked);
            assert_eq!(items[0].links[0].target, "Link Na Tarefa");
        } else {
            assert!(false, "Esperava TaskList");
        }

        // 7. MathBlock
        if let MarkdownBlock::MathBlock { formula } = &doc.blocks[6] {
            assert!(formula.contains("\\int"));
        } else {
            assert!(false, "Esperava MathBlock");
        }

        // 8. Table
        if let MarkdownBlock::Table { headers, rows } = &doc.blocks[7] {
            assert_eq!(headers, &vec!["Nome", "Cargo"]);
            assert_eq!(rows.len(), 2);
            assert_eq!(rows[0], vec!["Alice", "Desenvolvedora"]);
        } else {
            assert!(false, "Esperava Table");
        }
    }
}
