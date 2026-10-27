//! Controlador do editor de notas e conversor de AST em blocos renderizáveis para Slint.

use gnosis_core::buffer::NoteBuffer;
use gnosis_markdown::ast::{CalloutKind, MarkdownBlock};
use gnosis_markdown::block_parser::parse_blocks;
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
            Self::Reading => "Leitura (Ctrl+E)",
            Self::Edit => "Edição (Ctrl+E)",
        }
    }
}

/// Bloco plano preparado para renderização rica na interface gráfica Slint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderBlockModel {
    pub kind: String,
    pub text: String,
    pub extra: String,
    pub is_checked: bool,
    pub is_editing_math: bool,
}

/// Controlador do editor de notas, sincronizando o buffer Ropey com a AST e o modo de visualização.
#[derive(Debug, Clone)]
pub struct EditorController {
    pub mode: EditorMode,
    pub buffer: NoteBuffer,
    pub note_path: String,
    pub note_title: String,
    pub editing_math_index: Option<usize>,
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
        }
    }

    /// Inicia o modo interativo estilo Word para edição de fórmula matemática.
    pub fn start_editing_math(&mut self, block_index: usize) {
        self.editing_math_index = Some(block_index);
    }

    /// Cancela a edição interativa de fórmula retornando ao modo gráfico de repouso.
    pub fn cancel_editing_math(&mut self) {
        self.editing_math_index = None;
    }

    /// Aplica a nova fórmula editada no buffer da nota e recompila o bloco.
    pub fn apply_math_edit(&mut self, block_index: usize, new_formula: &str) {
        let content = self.content();
        let blocks = parse_blocks(&content);

        if let Some(MarkdownBlock::MathBlock { formula: old_formula }) = blocks.get(block_index) {
            let old_pattern = format!("$$\n{}\n$$", old_formula);
            let new_pattern = format!("$$\n{}\n$$", new_formula.trim());

            let updated_content = if content.contains(&old_pattern) {
                content.replace(&old_pattern, &new_pattern)
            } else {
                let old_inline_pattern = format!("$${}$$", old_formula);
                if content.contains(&old_inline_pattern) {
                    content.replace(&old_inline_pattern, &new_pattern)
                } else {
                    content
                }
            };

            self.set_content(&updated_content);
        }

        self.editing_math_index = None;
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
        let text = self.content();
        let blocks = parse_blocks(&text);
        let mut render_blocks = Vec::new();

        Self::flatten_blocks(&blocks, &mut render_blocks, self.editing_math_index);
        render_blocks
    }

    fn flatten_blocks(
        blocks: &[MarkdownBlock],
        out: &mut Vec<RenderBlockModel>,
        editing_math_idx: Option<usize>,
    ) {
        let mut math_engine = gnosis_markdown::math::MathEngine::new();

        for (idx, block) in blocks.iter().enumerate() {
            match block {
                MarkdownBlock::Heading { level, text, .. } => {
                    let kind = format!("h{}", level.clamp(&1, &6));
                    out.push(RenderBlockModel {
                        kind,
                        text: text.clone(),
                        extra: String::new(),
                        is_checked: false,
                        is_editing_math: false,
                    });
                }
                MarkdownBlock::Paragraph { text, .. } => {
                    out.push(RenderBlockModel {
                        kind: "paragraph".to_string(),
                        text: text.clone(),
                        extra: String::new(),
                        is_checked: false,
                        is_editing_math: false,
                    });
                }
                MarkdownBlock::CodeBlock { language, content } => {
                    out.push(RenderBlockModel {
                        kind: "code".to_string(),
                        text: content.clone(),
                        extra: language.clone().unwrap_or_else(|| "txt".to_string()),
                        is_checked: false,
                        is_editing_math: false,
                    });
                }
                MarkdownBlock::Callout {
                    kind,
                    title,
                    content,
                } => {
                    let kind_str = match kind {
                        CalloutKind::Note | CalloutKind::Info => "note",
                        CalloutKind::Tip | CalloutKind::Success => "tip",
                        CalloutKind::Warning => "warning",
                        CalloutKind::Caution => "caution",
                        _ => "note",
                    };

                    // Extrai texto dos blocos internos do callout
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
                        kind: "callout".to_string(),
                        text: inner_text,
                        extra: format!("{}:{}", kind_str, title),
                        is_checked: false,
                        is_editing_math: false,
                    });
                }
                MarkdownBlock::TaskList { items } => {
                    for item in items {
                        out.push(RenderBlockModel {
                            kind: "task".to_string(),
                            text: item.text.clone(),
                            extra: String::new(),
                            is_checked: item.checked,
                            is_editing_math: false,
                        });
                    }
                }
                MarkdownBlock::Blockquote { lines } => {
                    out.push(RenderBlockModel {
                        kind: "quote".to_string(),
                        text: lines.join("\n"),
                        extra: String::new(),
                        is_checked: false,
                        is_editing_math: false,
                    });
                }
                MarkdownBlock::MathBlock { formula } => {
                    let is_editing = editing_math_idx == Some(idx);
                    // Compila na engine vetorial para aquecer o cache LRU
                    let _ = math_engine.render_display(formula);
                    let display_label = format_math_label(formula);

                    out.push(RenderBlockModel {
                        kind: "math".to_string(),
                        text: formula.clone(),
                        extra: display_label,
                        is_checked: false,
                        is_editing_math: is_editing,
                    });
                }
                MarkdownBlock::Table { headers, rows } => {
                    let mut table_text = headers.join(" | ");
                    for row in rows {
                        table_text.push('\n');
                        table_text.push_str(&row.join(" | "));
                    }
                    out.push(RenderBlockModel {
                        kind: "table".to_string(),
                        text: table_text,
                        extra: String::new(),
                        is_checked: false,
                        is_editing_math: false,
                    });
                }
                MarkdownBlock::Divider => {
                    out.push(RenderBlockModel {
                        kind: "divider".to_string(),
                        text: String::new(),
                        extra: String::new(),
                        is_checked: false,
                        is_editing_math: false,
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
}
