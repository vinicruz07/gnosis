//! Controlador do editor de notas e conversor de AST em blocos renderizáveis para Slint.

use gnosis_core::buffer::NoteBuffer;
use gnosis_markdown::ast::{CalloutKind, MarkdownBlock, SpannedBlock};
use gnosis_markdown::block_parser::parse_spanned_blocks;
use gnosis_markdown::frontmatter::parse_frontmatter;
use serde::{Deserialize, Serialize};

/// Modo de exibição da nota ativa.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum EditorMode {
    #[default]
    Reading,
    Edit,
}

impl EditorMode {
    pub fn toggle(&self) -> Self {
        match self {
            Self::Reading => Self::Edit,
            Self::Edit => Self::Reading,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Reading => "✨ Live Preview",
            Self::Edit => "📄 Fonte Bruta",
        }
    }

    /// Retorna verdadeiro se estiver no modo padrão Live Preview.
    pub fn is_live_preview(&self) -> bool {
        matches!(self, Self::Reading)
    }

    /// Retorna verdadeiro se estiver no modo técnico de Fonte Bruta.
    pub fn is_source_mode(&self) -> bool {
        matches!(self, Self::Edit)
    }
}

/// Bloco plano preparado para renderização rica e edição in-place na interface gráfica Slint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderBlockModel {
    /// Identificador numérico único do bloco dentro da lista renderizada.
    pub id: usize,
    /// Categoria do bloco (h1..h6, paragraph, code, callout, task, quote, math, table, divider).
    pub kind: String,
    /// Texto processado/exibido no modo gráfico de repouso.
    pub text: String,
    /// Conteúdo bruto em sintaxe Markdown pura deste bloco para edição in-place.
    pub raw_text: String,
    /// Metadados auxiliares (linguagem de código, tipo de callout, rótulo de fórmula matemática).
    pub extra: String,
    /// Estado de checkbox para listas de tarefas.
    pub is_checked: bool,
    /// Se este bloco está em modo ativo de edição in-place no editor.
    pub is_editing: bool,
    /// Se este bloco matemático está em modo de edição de fórmula estilo Word.
    pub is_editing_math: bool,
    /// Posição do caractere inicial deste bloco no `NoteBuffer`.
    pub span_start: usize,
    /// Posição do caractere final deste bloco no `NoteBuffer`.
    pub span_end: usize,
}

/// Controlador do editor de notas, sincronizando o buffer Ropey com a AST e o modo de visualização.
#[derive(Debug, Clone)]
pub struct EditorController {
    pub mode: EditorMode,
    pub buffer: NoteBuffer,
    pub note_path: String,
    pub note_title: String,
    pub editing_math_index: Option<usize>,
    pub active_editing_block_id: Option<usize>,
}

impl Default for EditorController {
    fn default() -> Self {
        Self::new("", "")
    }
}

impl EditorController {
    /// Cria uma nova instância do controlador para uma nota.
    pub fn new(path: impl Into<String>, initial_content: impl Into<String>) -> Self {
        let p = path.into();
        let content = initial_content.into();
        let title = p.rsplit('/').next().unwrap_or(&p).trim_end_matches(".md").to_string();

        Self {
            mode: EditorMode::Reading,
            buffer: NoteBuffer::from_str(&content),
            note_path: p,
            note_title: title,
            editing_math_index: None,
            active_editing_block_id: None,
        }
    }

    /// Inicia a edição in-place de um bloco específico pelo seu ID.
    pub fn start_editing_block(&mut self, block_id: usize) {
        self.active_editing_block_id = Some(block_id);
    }

    /// Cancela a edição in-place de qualquer bloco, retornando ao modo gráfico renderizado.
    pub fn cancel_editing_block(&mut self) {
        self.active_editing_block_id = None;
        self.editing_math_index = None;
    }

    /// Aplica as alterações de texto cru no bloco especificado diretamente no buffer Ropey em O(log N).
    pub fn apply_block_edit(&mut self, block_id: usize, new_raw_text: &str) {
        let blocks = self.generate_render_blocks();
        if let Some(target) = blocks.into_iter().find(|b| b.id == block_id) {
            let _ = self.buffer.replace_range(
                target.span_start,
                target.span_end,
                new_raw_text.trim_end(),
            );
        }
        self.active_editing_block_id = None;
        self.editing_math_index = None;
    }

    /// Divide o bloco atual ou insere um novo bloco logo abaixo e transfere o foco de edição para ele.
    pub fn split_or_insert_block_below(&mut self, block_id: usize, current_text: &str) -> Option<usize> {
        let blocks = self.generate_render_blocks();
        let target = blocks.into_iter().find(|b| b.id == block_id)?;

        let trimmed_text = current_text.trim_end();
        let _ = self.buffer.replace_range(target.span_start, target.span_end, trimmed_text);

        let new_char_len = trimmed_text.chars().count();
        let insert_pos = target.span_start + new_char_len;

        let next_snippet = if target.kind == "task" {
            "\n- [ ] Nova tarefa"
        } else if target.kind == "quote" {
            "\n> Nova citação"
        } else {
            "\n\nNovo parágrafo"
        };

        let _ = self.buffer.insert(insert_pos, next_snippet);

        let new_blocks = self.generate_render_blocks();
        let next_id = (block_id + 1).min(new_blocks.len().saturating_sub(1));
        self.active_editing_block_id = Some(next_id);
        Some(next_id)
    }

    /// Remove um bloco vazio ou indesejado e transfere o foco para o bloco anterior.
    pub fn delete_empty_block_and_merge_previous(&mut self, block_id: usize) -> Option<usize> {
        let blocks = self.generate_render_blocks();
        if blocks.is_empty() {
            return None;
        }

        let target_idx = blocks.iter().position(|b| b.id == block_id)?;
        let target = &blocks[target_idx];

        let delete_start = if target_idx > 0 {
            blocks[target_idx - 1].span_end
        } else {
            target.span_start
        };
        let delete_end = target.span_end;

        if delete_end >= delete_start {
            let _ = self.buffer.replace_range(delete_start, delete_end, "");
        }

        let prev_id = if target_idx > 0 {
            Some(blocks[target_idx - 1].id)
        } else {
            None
        };

        self.active_editing_block_id = prev_id;
        prev_id
    }

    /// Move o foco de edição para o bloco anterior, se existir.
    pub fn navigate_to_previous_block(&mut self) -> Option<usize> {
        let blocks = self.generate_render_blocks();
        let current_id = self.active_editing_block_id?;
        let current_idx = blocks.iter().position(|b| b.id == current_id)?;
        if current_idx > 0 {
            let prev_id = blocks[current_idx - 1].id;
            self.active_editing_block_id = Some(prev_id);
            Some(prev_id)
        } else {
            None
        }
    }

    /// Move o foco de edição para o próximo bloco, se existir.
    pub fn navigate_to_next_block(&mut self) -> Option<usize> {
        let blocks = self.generate_render_blocks();
        let current_id = self.active_editing_block_id?;
        let current_idx = blocks.iter().position(|b| b.id == current_id)?;
        if current_idx + 1 < blocks.len() {
            let next_id = blocks[current_idx + 1].id;
            self.active_editing_block_id = Some(next_id);
            Some(next_id)
        } else {
            None
        }
    }

    /// Salva o bloco atual e move o foco para o bloco anterior.
    pub fn navigate_to_previous_block_with_save(&mut self, current_text: Option<&str>) -> Option<usize> {
        if let (Some(cur_id), Some(text)) = (self.active_editing_block_id, current_text) {
            self.apply_block_edit(cur_id, text);
            self.active_editing_block_id = Some(cur_id);
        }
        self.navigate_to_previous_block()
    }

    /// Salva o bloco atual e move o foco para o próximo bloco.
    pub fn navigate_to_next_block_with_save(&mut self, current_text: Option<&str>) -> Option<usize> {
        if let (Some(cur_id), Some(text)) = (self.active_editing_block_id, current_text) {
            self.apply_block_edit(cur_id, text);
            self.active_editing_block_id = Some(cur_id);
        }
        self.navigate_to_next_block()
    }

    /// Inicia o modo interativo estilo Word para edição de fórmula matemática.
    pub fn start_editing_math(&mut self, block_index: usize) {
        self.editing_math_index = Some(block_index);
        self.active_editing_block_id = Some(block_index);
    }

    /// Cancela a edição interativa de fórmula retornando ao modo gráfico de repouso.
    pub fn cancel_editing_math(&mut self) {
        self.editing_math_index = None;
        self.active_editing_block_id = None;
    }

    /// Aplica a nova fórmula editada no buffer da nota e recompila o bloco.
    pub fn apply_math_edit(&mut self, block_index: usize, new_formula: &str) {
        let blocks = self.generate_render_blocks();
        if let Some(target) = blocks.into_iter().find(|b| b.id == block_index) {
            let formula_trimmed = new_formula.trim();
            let new_raw = format!("$$\n{}\n$$", formula_trimmed);
            let _ = self.buffer.replace_range(target.span_start, target.span_end, &new_raw);
        } else {
            // Fallback baseado em busca de padrão
            let content = self.content();
            let old_pattern = format!("$$\n{}\n$$", new_formula);
            let new_pattern = format!("$$\n{}\n$$", new_formula.trim());
            let updated = if content.contains(&old_pattern) {
                content.replace(&old_pattern, &new_pattern)
            } else {
                content
            };
            self.set_content(&updated);
        }

        self.editing_math_index = None;
        self.active_editing_block_id = None;
    }

    /// Alterna o estado de marcação de uma tarefa (`- [ ]` <-> `- [x]`) preservando o texto da nota.
    pub fn toggle_task_checkbox(&mut self, block_id: usize) {
        let blocks = self.generate_render_blocks();
        if let Some(target) = blocks.into_iter().find(|b| b.id == block_id && b.kind == "task") {
            let current_raw = target.raw_text.trim();
            let new_raw = if current_raw.starts_with("- [ ]") {
                current_raw.replacen("- [ ]", "- [x]", 1)
            } else if current_raw.starts_with("- [x]") || current_raw.starts_with("- [X]") {
                current_raw.replacen("- [x]", "- [ ]", 1).replacen("- [X]", "- [ ]", 1)
            } else if current_raw.starts_with("* [ ]") {
                current_raw.replacen("* [ ]", "* [x]", 1)
            } else if current_raw.starts_with("* [x]") || current_raw.starts_with("* [X]") {
                current_raw.replacen("* [x]", "* [ ]", 1).replacen("* [X]", "* [ ]", 1)
            } else {
                current_raw.to_string()
            };

            let _ = self.buffer.replace_range(target.span_start, target.span_end, &new_raw);
        }
    }

    /// Alterna entre modo de Edição e modo de Leitura com preservação integral do conteúdo.
    pub fn toggle_mode(&mut self) -> EditorMode {
        self.mode = self.mode.toggle();
        self.mode
    }

    /// Atualiza o conteúdo textual no buffer de alta performance Ropey.
    pub fn set_content(&mut self, new_text: &str) {
        self.buffer = NoteBuffer::from_str(new_text);
    }

    /// Retorna o conteúdo textual completo.
    pub fn content(&self) -> String {
        self.buffer.to_string()
    }

    /// Contagem total de palavras no buffer.
    pub fn word_count(&self) -> usize {
        self.buffer.word_count()
    }

    /// Contagem total de caracteres no buffer.
    pub fn char_count(&self) -> usize {
        self.buffer.char_count()
    }

    /// Faz o parse do markdown e gera uma lista plana de blocos tipados prontos para o Slint.
    pub fn generate_render_blocks(&self) -> Vec<RenderBlockModel> {
        let content = self.content();
        let (_fm, body) = parse_frontmatter(&content);

        // Calcula o offset de caracteres UTF-8 onde o corpo da nota começa (após o frontmatter)
        let body_char_offset = if let Some(pos) = content.find(body) {
            content[..pos].chars().count()
        } else {
            0
        };

        let spanned_blocks = parse_spanned_blocks(body);
        let mut render_blocks = Vec::new();

        Self::flatten_spanned_blocks(
            &spanned_blocks,
            body_char_offset,
            &mut render_blocks,
            self.editing_math_index,
            self.active_editing_block_id,
        );
        render_blocks
    }

    fn flatten_spanned_blocks(
        blocks: &[SpannedBlock],
        body_offset: usize,
        out: &mut Vec<RenderBlockModel>,
        editing_math_idx: Option<usize>,
        active_editing_id: Option<usize>,
    ) {
        let mut math_engine = gnosis_markdown::math::MathEngine::new();

        for spanned in blocks {
            let span_start = body_offset + spanned.span_start;
            let span_end = body_offset + spanned.span_end;
            let raw_text = spanned.raw_text.clone();

            match &spanned.block {
                MarkdownBlock::Heading { level, text, .. } => {
                    let id = out.len();
                    let is_editing = active_editing_id == Some(id);
                    let kind = format!("h{}", level.clamp(&1, &6));
                    out.push(RenderBlockModel {
                        id,
                        kind,
                        text: text.clone(),
                        raw_text,
                        extra: String::new(),
                        is_checked: false,
                        is_editing,
                        is_editing_math: false,
                        span_start,
                        span_end,
                    });
                }
                MarkdownBlock::Paragraph { text, .. } => {
                    let id = out.len();
                    let is_editing = active_editing_id == Some(id);
                    out.push(RenderBlockModel {
                        id,
                        kind: "paragraph".to_string(),
                        text: text.clone(),
                        raw_text,
                        extra: String::new(),
                        is_checked: false,
                        is_editing,
                        is_editing_math: false,
                        span_start,
                        span_end,
                    });
                }
                MarkdownBlock::CodeBlock { language, content } => {
                    let id = out.len();
                    let is_editing = active_editing_id == Some(id);
                    out.push(RenderBlockModel {
                        id,
                        kind: "code".to_string(),
                        text: content.clone(),
                        raw_text,
                        extra: language.clone().unwrap_or_else(|| "txt".to_string()),
                        is_checked: false,
                        is_editing,
                        is_editing_math: false,
                        span_start,
                        span_end,
                    });
                }
                MarkdownBlock::Callout {
                    kind,
                    title,
                    content,
                } => {
                    let id = out.len();
                    let is_editing = active_editing_id == Some(id);
                    let kind_str = match kind {
                        CalloutKind::Note | CalloutKind::Info => "note",
                        CalloutKind::Tip | CalloutKind::Success => "tip",
                        CalloutKind::Warning => "warning",
                        CalloutKind::Caution => "caution",
                        _ => "note",
                    };

                    let inner_text = content
                        .iter()
                        .map(|b| match b {
                            MarkdownBlock::Paragraph { text, .. } => text.as_str(),
                            MarkdownBlock::Heading { text, .. } => text.as_str(),
                            _ => "",
                        })
                        .filter(|s| !s.is_empty())
                        .collect::<Vec<_>>()
                        .join("\n");

                    out.push(RenderBlockModel {
                        id,
                        kind: "callout".to_string(),
                        text: inner_text,
                        raw_text,
                        extra: format!("{}:{}", kind_str, title),
                        is_checked: false,
                        is_editing,
                        is_editing_math: false,
                        span_start,
                        span_end,
                    });
                }
                MarkdownBlock::TaskList { items } => {
                    for item in items {
                        let id = out.len();
                        let is_editing = active_editing_id == Some(id);
                        let item_raw = format!("- [{}] {}", if item.checked { "x" } else { " " }, item.text);
                        out.push(RenderBlockModel {
                            id,
                            kind: "task".to_string(),
                            text: item.text.clone(),
                            raw_text: item_raw,
                            extra: String::new(),
                            is_checked: item.checked,
                            is_editing,
                            is_editing_math: false,
                            span_start,
                            span_end,
                        });
                    }
                }
                MarkdownBlock::Blockquote { lines } => {
                    let id = out.len();
                    let is_editing = active_editing_id == Some(id);
                    out.push(RenderBlockModel {
                        id,
                        kind: "quote".to_string(),
                        text: lines.join("\n"),
                        raw_text,
                        extra: String::new(),
                        is_checked: false,
                        is_editing,
                        is_editing_math: false,
                        span_start,
                        span_end,
                    });
                }
                MarkdownBlock::MathBlock { formula } => {
                    let id = out.len();
                    let is_editing = (editing_math_idx == Some(id)) || (active_editing_id == Some(id));
                    let _ = math_engine.render_display(formula);
                    let display_label = format_math_label(formula);

                    out.push(RenderBlockModel {
                        id,
                        kind: "math".to_string(),
                        text: formula.clone(),
                        raw_text,
                        extra: display_label,
                        is_checked: false,
                        is_editing,
                        is_editing_math: is_editing,
                        span_start,
                        span_end,
                    });
                }
                MarkdownBlock::Table { headers, rows } => {
                    let id = out.len();
                    let is_editing = active_editing_id == Some(id);
                    let mut table_text = headers.join(" | ");
                    for row in rows {
                        table_text.push('\n');
                        table_text.push_str(&row.join(" | "));
                    }
                    out.push(RenderBlockModel {
                        id,
                        kind: "table".to_string(),
                        text: table_text,
                        raw_text,
                        extra: String::new(),
                        is_checked: false,
                        is_editing,
                        is_editing_math: false,
                        span_start,
                        span_end,
                    });
                }
                MarkdownBlock::Divider => {
                    let id = out.len();
                    out.push(RenderBlockModel {
                        id,
                        kind: "divider".to_string(),
                        text: String::new(),
                        raw_text,
                        extra: String::new(),
                        is_checked: false,
                        is_editing: false,
                        is_editing_math: false,
                        span_start,
                        span_end,
                    });
                }
            }
        }
    }
}

/// Formata a expressão matemática para exibição tipográfica no modo gráfico de repouso.
fn format_math_label(formula: &str) -> String {
    let mut s = formula.trim().to_string();
    let glyphs = [
        (r"\sum", "∑"), (r"\prod", "∏"), (r"\int", "∫"), (r"\iint", "∬"), (r"\oint", "∮"),
        (r"\alpha", "α"), (r"\beta", "β"), (r"\gamma", "γ"), (r"\delta", "δ"),
        (r"\pi", "π"), (r"\sigma", "σ"), (r"\theta", "θ"), (r"\lambda", "λ"),
        (r"\infty", "∞"), (r"\pm", "±"), (r"\sqrt", "√"),
        (r"\leq", "≤"), (r"\geq", "≥"), (r"\neq", "≠"), (r"\approx", "≈"),
        (r"\rightarrow", "→"), (r"\partial", "∂"), (r"\nabla", "∇"),
        (r"\begin{pmatrix}", "("), (r"\end{pmatrix}", ")"),
        (r"\begin{bmatrix}", "["), (r"\end{bmatrix}", "]"),
        (r"\\", " "), (r"&", " "),
    ];

    for (pattern, rep) in glyphs {
        s = s.replace(pattern, rep);
    }

    if s.contains(r"\frac") {
        s = s.replace(r"\frac{", "(").replace("}{", " / ").replace('}', ")");
    }

    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_editor_controller_mode_toggle_and_content_preservation() {
        let mut controller = EditorController::new("Notas/Teste.md", "# Título Original\n\nConteúdo aqui.");

        assert_eq!(controller.mode, EditorMode::Reading);
        assert_eq!(controller.note_title, "Teste");

        // Alterna para edição
        let mode = controller.toggle_mode();
        assert_eq!(mode, EditorMode::Edit);
        assert_eq!(controller.mode, EditorMode::Edit);

        // Edita texto no buffer Ropey
        controller.set_content("# Título Modificado\n\nNova linha inserida.");

        // Alterna de volta para leitura
        let mode_back = controller.toggle_mode();
        assert_eq!(mode_back, EditorMode::Reading);
        assert!(controller.content().contains("Título Modificado"));
    }

    #[test]
    fn test_generate_render_blocks_for_all_markdown_types() {
        let markdown = r#"# Cabeçalho Principal
Este é um parágrafo explicativo com wikilink.

```rust
fn main() { println!("Gnosis"); }
```

> [!NOTE] Dica de Ouro
> Mantenha suas anotações atômicas e interligadas.

- [x] Tarefa concluída
- [ ] Tarefa pendente

> Esta é uma citação importante.
"#;

        let controller = EditorController::new("Guia.md", markdown);
        let blocks = controller.generate_render_blocks();

        // Deve conter H1, Paragraph, Code, Callout, Task (2x) e Quote
        assert!(blocks.iter().any(|b| b.kind == "h1" && b.text == "Cabeçalho Principal"));
        assert!(blocks.iter().any(|b| b.kind == "paragraph"));
        assert!(blocks.iter().any(|b| b.kind == "code" && b.extra == "rust"));
        assert!(blocks.iter().any(|b| b.kind == "callout" && b.extra.contains("note")));
        assert!(blocks.iter().any(|b| b.kind == "task" && b.is_checked));
        assert!(blocks.iter().any(|b| b.kind == "task" && !b.is_checked));
        assert!(blocks.iter().any(|b| b.kind == "quote"));
    }

    #[test]
    fn test_interactive_math_editing_mode_word_style() {
        let markdown = "# Fórmula\n\n$$\nx^2 + y^2 = z^2\n$$\n";
        let mut controller = EditorController::new("Math.md", markdown);

        let blocks = controller.generate_render_blocks();
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[1].kind, "math");
        assert_eq!(blocks[1].is_editing_math, false);
        assert!(blocks[1].extra.contains("x^2"));

        // Ativa modo interativo estilo Word no bloco 1
        controller.start_editing_math(1);
        let blocks_editing = controller.generate_render_blocks();
        assert_eq!(blocks_editing[1].is_editing_math, true);

        // Aplica nova fórmula editada
        controller.apply_math_edit(1, "E = mc^2");
        let blocks_after = controller.generate_render_blocks();
        assert_eq!(blocks_after[1].is_editing_math, false);
        assert_eq!(blocks_after[1].text, "E = mc^2");
        assert!(controller.content().contains("E = mc^2"));
    }

    #[test]
    fn test_block_editing_lifecycle_and_spans_in_live_preview() {
        let markdown = "---\ntitle: Filosofia\n---\n# Introdução\n\nEste é o primeiro parágrafo.\n\n## Subseção\n\nOutro texto aqui.";
        let mut controller = EditorController::new("Filosofia.md", markdown);

        let blocks = controller.generate_render_blocks();
        assert_eq!(blocks.len(), 4);

        // Bloco 0 é o H1
        assert_eq!(blocks[0].id, 0);
        assert_eq!(blocks[0].kind, "h1");
        assert_eq!(blocks[0].text, "Introdução");
        assert_eq!(blocks[0].raw_text, "# Introdução");
        assert!(!blocks[0].is_editing);

        // Bloco 1 é o parágrafo
        assert_eq!(blocks[1].id, 1);
        assert_eq!(blocks[1].kind, "paragraph");
        assert_eq!(blocks[1].raw_text, "Este é o primeiro parágrafo.");

        // Inicia a edição in-place do parágrafo (bloco 1)
        controller.start_editing_block(1);
        let editing_blocks = controller.generate_render_blocks();
        assert!(editing_blocks[1].is_editing);
        assert!(!editing_blocks[0].is_editing);

        // Aplica edição cirúrgica via Ropey
        controller.apply_block_edit(1, "Este é o parágrafo modificado cirurgicamente.");
        let updated_blocks = controller.generate_render_blocks();
        assert!(!updated_blocks[1].is_editing);
        assert_eq!(updated_blocks[1].text, "Este é o parágrafo modificado cirurgicamente.");

        // O frontmatter e os outros blocos foram preservados 100% intactos
        let content = controller.content();
        assert!(content.contains("title: Filosofia"));
        assert!(content.contains("# Introdução"));
        assert!(content.contains("## Subseção"));
        assert!(content.contains("Este é o parágrafo modificado cirurgicamente."));
    }

    #[test]
    fn test_task_toggle_and_code_table_editing_in_live_preview() {
        let markdown = "- [ ] Ler Nietzsche\n\n```rust\nfn rodar() {}\n```\n\n| Item | Preço |\n|---|---|\n| Livro | 50 |";
        let mut controller = EditorController::new("Tarefas.md", markdown);

        let blocks = controller.generate_render_blocks();
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0].kind, "task");
        assert!(!blocks[0].is_checked);

        // Alterna o checkbox da tarefa
        controller.toggle_task_checkbox(0);
        assert!(controller.content().contains("- [x] Ler Nietzsche"));
        let blocks_toggled = controller.generate_render_blocks();
        assert!(blocks_toggled[0].is_checked);

        // Alterna de volta
        controller.toggle_task_checkbox(0);
        assert!(controller.content().contains("- [ ] Ler Nietzsche"));

        // Edita o bloco de código (bloco 1)
        assert_eq!(blocks[1].kind, "code");
        controller.start_editing_block(1);
        controller.apply_block_edit(1, "```python\nprint('Python')\n```");
        assert!(controller.content().contains("```python"));
        assert!(controller.content().contains("print('Python')"));

        // Edita a tabela (bloco 2)
        let blocks_after_code = controller.generate_render_blocks();
        assert_eq!(blocks_after_code[2].kind, "table");
        controller.start_editing_block(2);
        controller.apply_block_edit(2, "| Item | Preço |\n|---|---|\n| Caderno | 25 |");
        assert!(controller.content().contains("Caderno"));
        assert!(controller.content().contains("25"));
    }

    #[test]
    fn test_live_preview_flow_enter_backspace_and_navigation() {
        let markdown = "# Título Principal\n\nPrimeiro parágrafo de texto.";
        let mut controller = EditorController::new("Nota.md", markdown);

        let initial_blocks = controller.generate_render_blocks();
        assert_eq!(initial_blocks.len(), 2);

        // Foca no bloco 0 (título)
        controller.start_editing_block(0);
        assert_eq!(controller.active_editing_block_id, Some(0));

        // Pressiona Enter no bloco 0: insere novo parágrafo abaixo e foca nele
        let next_id = controller.split_or_insert_block_below(0, "# Título Editado");
        assert_eq!(next_id, Some(1));
        assert_eq!(controller.active_editing_block_id, Some(1));

        let blocks_after_enter = controller.generate_render_blocks();
        assert_eq!(blocks_after_enter.len(), 3);
        assert_eq!(blocks_after_enter[0].raw_text, "# Título Editado");
        assert_eq!(blocks_after_enter[1].raw_text, "Novo parágrafo");
        assert_eq!(blocks_after_enter[2].raw_text, "Primeiro parágrafo de texto.");

        // Modifica o bloco 1 e navega para cima com salvamento automático
        let prev_id = controller.navigate_to_previous_block_with_save(Some("Parágrafo intermediário digitado."));
        assert_eq!(prev_id, Some(0));
        assert_eq!(controller.active_editing_block_id, Some(0));
        assert!(controller.content().contains("Parágrafo intermediário digitado."));

        // Navega para baixo para o bloco 1
        let down_id = controller.navigate_to_next_block();
        assert_eq!(down_id, Some(1));
        assert_eq!(controller.active_editing_block_id, Some(1));

        // Insere mais um bloco abaixo do bloco 1
        let third_id = controller.split_or_insert_block_below(1, "Parágrafo intermediário digitado.");
        assert_eq!(third_id, Some(2));
        assert_eq!(controller.active_editing_block_id, Some(2));
        assert_eq!(controller.generate_render_blocks().len(), 4);

        // Deleta o bloco 2 com backspace/unir
        let merged_id = controller.delete_empty_block_and_merge_previous(2);
        assert_eq!(merged_id, Some(1));
        assert_eq!(controller.active_editing_block_id, Some(1));
        assert_eq!(controller.generate_render_blocks().len(), 3);
    }
}
