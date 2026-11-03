//! Parser linear de blocos Markdown para a AST do Gnosis.
//!
//! Converte o corpo do documento em uma sequência de [`MarkdownBlock`],
//! pronta para alimentar diretamente a árvore de componentes visuais do Slint.

use crate::ast::{CalloutKind, MarkdownBlock, MarkdownDoc, SpannedBlock, TaskItem};
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

/// Informações de linha com cálculo de deslocamento de caracteres UTF-8.
struct LineInfo<'a> {
    text: &'a str,
    char_start: usize,
    char_end: usize,
}

/// Divide o corpo do texto em blocos estruturados.
pub fn parse_blocks(body: &str) -> Vec<MarkdownBlock> {
    parse_spanned_blocks(body).into_iter().map(|sb| sb.block).collect()
}

/// Divide o corpo do texto em blocos estruturados acompanhados de coordenadas de span e texto bruto.
pub fn parse_spanned_blocks(body: &str) -> Vec<SpannedBlock> {
    let mut blocks = Vec::new();

    // Mapeia cada linha com seus índices de caracteres absolutos no corpo
    let mut line_infos: Vec<LineInfo> = Vec::new();
    let mut char_cursor = 0;
    for line in body.split('\n') {
        let clean = line.trim_end_matches('\r');
        let line_chars = clean.chars().count();
        let full_line_chars = line.chars().count();
        line_infos.push(LineInfo {
            text: clean,
            char_start: char_cursor,
            char_end: char_cursor + line_chars,
        });
        char_cursor += full_line_chars + 1; // +1 para a quebra de linha '\n'
    }

    let total_lines = line_infos.len();
    let mut i = 0;

    while i < total_lines {
        let trimmed = line_infos[i].text.trim();

        // 1. Linhas vazias são ignoradas entre blocos
        if trimmed.is_empty() {
            i += 1;
            continue;
        }

        // 2. Divisores horizontais (`---`, `***`, `___`)
        if (trimmed == "---" || trimmed == "***" || trimmed == "___") && trimmed.len() >= 3 {
            let start_line = i;
            let span_start = line_infos[start_line].char_start;
            let span_end = line_infos[start_line].char_end;
            let raw_text = line_infos[start_line].text.to_string();
            i += 1;

            blocks.push(SpannedBlock {
                block: MarkdownBlock::Divider,
                span_start,
                span_end,
                raw_text,
            });
            continue;
        }

        // 3. Bloco de código delimitado por ```
        if trimmed.starts_with("```") {
            let start_line = i;
            let lang_part = trimmed.trim_start_matches('`').trim();
            let language = if lang_part.is_empty() {
                None
            } else {
                Some(lang_part.to_string())
            };

            let mut code_lines = Vec::new();
            i += 1;

            while i < total_lines {
                let code_line = line_infos[i].text;
                if code_line.trim().starts_with("```") {
                    i += 1;
                    break;
                }
                code_lines.push(code_line);
                i += 1;
            }

            let span_start = line_infos[start_line].char_start;
            let span_end = line_infos[i - 1].char_end;
            let raw_text = line_infos[start_line..i]
                .iter()
                .map(|l| l.text)
                .collect::<Vec<_>>()
                .join("\n");

            blocks.push(SpannedBlock {
                block: MarkdownBlock::CodeBlock {
                    language,
                    content: code_lines.join("\n"),
                },
                span_start,
                span_end,
                raw_text,
            });
            continue;
        }

        // 4. Bloco de matemática centralizado delimitado por $$
        if trimmed.starts_with("$$") {
            let start_line = i;

            // Caso seja em uma única linha: `$$ formula $$`
            if trimmed.len() > 4 && trimmed.ends_with("$$") {
                let formula = trimmed[2..trimmed.len() - 2].trim().to_string();
                let span_start = line_infos[start_line].char_start;
                let span_end = line_infos[start_line].char_end;
                let raw_text = line_infos[start_line].text.to_string();
                i += 1;

                blocks.push(SpannedBlock {
                    block: MarkdownBlock::MathBlock { formula },
                    span_start,
                    span_end,
                    raw_text,
                });
                continue;
            }

            let mut math_lines = Vec::new();
            i += 1;

            while i < total_lines {
                let math_line = line_infos[i].text;
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

            let span_start = line_infos[start_line].char_start;
            let span_end = line_infos[i - 1].char_end;
            let raw_text = line_infos[start_line..i]
                .iter()
                .map(|l| l.text)
                .collect::<Vec<_>>()
                .join("\n");

            blocks.push(SpannedBlock {
                block: MarkdownBlock::MathBlock {
                    formula: math_lines.join("\n"),
                },
                span_start,
                span_end,
                raw_text,
            });
            continue;
        }

        // 5. Cabeçalhos (# H1 até ###### H6)
        if is_valid_heading_line(trimmed) {
            let mut level = 0;
            for c in trimmed.chars() {
                if c == '#' {
                    level += 1;
                } else {
                    break;
                }
            }

            let rest = trimmed[level..].trim();
            let (header_text, anchor) = match rest.rsplit_once(" ^") {
                Some((title, anc)) => (title.trim(), Some(anc.trim().to_string())),
                None => (rest, None),
            };

            let start_line = i;
            let span_start = line_infos[start_line].char_start;
            let span_end = line_infos[start_line].char_end;
            let raw_text = line_infos[start_line].text.to_string();
            i += 1;

            blocks.push(SpannedBlock {
                block: MarkdownBlock::Heading {
                    level: level as u8,
                    text: header_text.to_string(),
                    anchor,
                },
                span_start,
                span_end,
                raw_text,
            });
            continue;
        }

        // 6. Citações e Callouts (`> ...`)
        if trimmed.starts_with('>') {
            let start_line = i;
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
                    let next_line = line_infos[i].text;
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

                let span_start = line_infos[start_line].char_start;
                let span_end = line_infos[i - 1].char_end;
                let raw_text = line_infos[start_line..i]
                    .iter()
                    .map(|l| l.text)
                    .collect::<Vec<_>>()
                    .join("\n");

                blocks.push(SpannedBlock {
                    block: MarkdownBlock::Callout {
                        kind,
                        title,
                        content: inner_blocks,
                    },
                    span_start,
                    span_end,
                    raw_text,
                });
                continue;
            }

            // Citação comum
            let mut quote_lines = Vec::new();
            while i < total_lines {
                let next_line = line_infos[i].text;
                let next_trimmed = next_line.trim();
                if next_trimmed.starts_with('>') {
                    quote_lines.push(next_trimmed.trim_start_matches('>').trim().to_string());
                    i += 1;
                } else {
                    break;
                }
            }

            let span_start = line_infos[start_line].char_start;
            let span_end = line_infos[i - 1].char_end;
            let raw_text = line_infos[start_line..i]
                .iter()
                .map(|l| l.text)
                .collect::<Vec<_>>()
                .join("\n");

            blocks.push(SpannedBlock {
                block: MarkdownBlock::Blockquote { lines: quote_lines },
                span_start,
                span_end,
                raw_text,
            });
            continue;
        }

        // 7. Lista de Tarefas (`- [ ] ` ou `- [x] `)
        if is_task_line(trimmed) {
            let start_line = i;
            let mut task_items = Vec::new();

            while i < total_lines {
                let current_line = line_infos[i].text.trim();
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

            let span_start = line_infos[start_line].char_start;
            let span_end = line_infos[i - 1].char_end;
            let raw_text = line_infos[start_line..i]
                .iter()
                .map(|l| l.text)
                .collect::<Vec<_>>()
                .join("\n");

            blocks.push(SpannedBlock {
                block: MarkdownBlock::TaskList { items: task_items },
                span_start,
                span_end,
                raw_text,
            });
            continue;
        }

        // 8. Tabelas (`| Coluna 1 | Coluna 2 |`)
        if trimmed.starts_with('|') && trimmed.ends_with('|') && i + 1 < total_lines {
            let next_line = line_infos[i + 1].text.trim();
            // Verifica se a linha seguinte é o separador de tabela `|---|---|`
            if next_line.starts_with('|') && next_line.contains('-') && next_line.ends_with('|') {
                let start_line = i;
                let headers = parse_table_row(trimmed);
                let mut rows = Vec::new();
                i += 2; // Pula o cabeçalho e o divisor

                while i < total_lines {
                    let row_line = line_infos[i].text.trim();
                    if row_line.starts_with('|') && row_line.ends_with('|') {
                        rows.push(parse_table_row(row_line));
                        i += 1;
                    } else {
                        break;
                    }
                }

                let span_start = line_infos[start_line].char_start;
                let span_end = line_infos[i - 1].char_end;
                let raw_text = line_infos[start_line..i]
                    .iter()
                    .map(|l| l.text)
                    .collect::<Vec<_>>()
                    .join("\n");

                blocks.push(SpannedBlock {
                    block: MarkdownBlock::Table { headers, rows },
                    span_start,
                    span_end,
                    raw_text,
                });
                continue;
            }
        }

        // 9. Parágrafo comum (acumula linhas normais contíguas)
        let start_line = i;
        let mut paragraph_lines = Vec::new();
        while i < total_lines {
            let p_line = line_infos[i].text;
            let p_trimmed = p_line.trim();

            if p_trimmed.is_empty()
                || is_valid_heading_line(p_trimmed)
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

            let span_start = line_infos[start_line].char_start;
            let span_end = line_infos[i - 1].char_end;
            let raw_text = line_infos[start_line..i]
                .iter()
                .map(|l| l.text)
                .collect::<Vec<_>>()
                .join("\n");

            blocks.push(SpannedBlock {
                block: MarkdownBlock::Paragraph {
                    text: paragraph_text,
                    links,
                    tags,
                },
                span_start,
                span_end,
                raw_text,
            });
        } else if i == start_line && i < total_lines {
            // Invariante de Segurança: Garante que nenhuma iteração do parser falhe em avançar i
            let p_line = line_infos[i].text.trim();
            let span_start = line_infos[i].char_start;
            let span_end = line_infos[i].char_end;
            let raw_text = line_infos[i].text.to_string();
            let links = scan_wikilinks(p_line);
            let tags = scan_tags(p_line);

            blocks.push(SpannedBlock {
                block: MarkdownBlock::Paragraph {
                    text: p_line.to_string(),
                    links,
                    tags,
                },
                span_start,
                span_end,
                raw_text,
            });
            i += 1;
        }
    }

    blocks
}

/// Verifica se uma linha é um cabeçalho Markdown estritamente válido (# H1 até ###### H6 com espaço).
fn is_valid_heading_line(s: &str) -> bool {
    let trimmed = s.trim();
    if !trimmed.starts_with('#') {
        return false;
    }

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
        trimmed.chars().nth(level) == Some(' ') || rest.is_empty()
    } else {
        false
    }
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

    #[test]
    fn test_parse_spanned_blocks_coordinates_and_raw_text() {
        let body = "# Título 1\n\nPrimeiro parágrafo de teste.\n\n- [ ] Tarefa 1\n- [x] Tarefa 2\n\n$$\nx^2\n$$";
        let spanned = parse_spanned_blocks(body);

        assert_eq!(spanned.len(), 4);

        let slice_chars = |s: &str, start: usize, end: usize| -> String {
            s.chars().skip(start).take(end - start).collect()
        };

        // Bloco 0: Heading
        assert_eq!(spanned[0].raw_text, "# Título 1");
        assert_eq!(spanned[0].span_start, 0);
        assert_eq!(spanned[0].span_end, 10);
        assert_eq!(slice_chars(body, spanned[0].span_start, spanned[0].span_end), "# Título 1");

        // Bloco 1: Paragraph
        assert_eq!(spanned[1].raw_text, "Primeiro parágrafo de teste.");
        assert_eq!(slice_chars(body, spanned[1].span_start, spanned[1].span_end), "Primeiro parágrafo de teste.");

        // Bloco 2: TaskList
        assert!(spanned[2].raw_text.contains("- [ ] Tarefa 1"));
        assert!(spanned[2].raw_text.contains("- [x] Tarefa 2"));

        // Bloco 3: MathBlock
        assert!(spanned[3].raw_text.contains("$$"));
        assert!(spanned[3].raw_text.contains("x^2"));
    }

    #[test]
    fn test_heading_with_more_than_6_hashes_never_loops_infinitely() {
        let input = "####### Testes\n\n######## Mais de 6\n\n#tag_sem_espaco\n\n# Heading Válido";
        let start = std::time::Instant::now();
        let spanned = parse_spanned_blocks(input);
        let elapsed = start.elapsed();

        // Deve executar em menos de 10ms (sem loop infinito)
        assert!(elapsed.as_millis() < 50, "Parser demorou {:?} (possível loop infinito)", elapsed);
        assert_eq!(spanned.len(), 4);

        // Primeiro bloco deve ser Parágrafo comum
        if let MarkdownBlock::Paragraph { text, .. } = &spanned[0].block {
            assert_eq!(text, "####### Testes");
        } else {
            panic!("Esperava Paragraph para 7 hashtags");
        }

        // Segundo bloco deve ser Parágrafo comum
        if let MarkdownBlock::Paragraph { text, .. } = &spanned[1].block {
            assert_eq!(text, "######## Mais de 6");
        } else {
            panic!("Esperava Paragraph para 8 hashtags");
        }

        // Terceiro bloco deve ser Parágrafo comum com a tag
        if let MarkdownBlock::Paragraph { text, .. } = &spanned[2].block {
            assert_eq!(text, "#tag_sem_espaco");
        } else {
            panic!("Esperava Paragraph para tag no início da linha");
        }

        // Quarto bloco deve ser Heading 1 válido
        if let MarkdownBlock::Heading { level, text, .. } = &spanned[3].block {
            assert_eq!(*level, 1);
            assert_eq!(text, "Heading Válido");
        } else {
            panic!("Esperava Heading para `# Heading Válido`");
        }
    }
}
