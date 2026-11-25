//! Editor de documento contínuo estilo Word compatível com hardware legado (CPU/Software).
//!
//! Usa `parley::PlainEditor` para layout/cursor/seleção contínua e `swash` para rasterização
//! de glifos diretamente em `slint::SharedPixelBuffer` (apenas a viewport visível),
//! garantindo 0% de dependência de GPU e baixo consumo de memória.

#![deny(unsafe_code)]

use parley::editing::PlainEditor;
use parley::layout::PositionedLayoutItem;
use parley::style::StyleProperty;
use parley::{FontContext, LayoutContext};
use slint::{Image, Rgba8Pixel, SharedPixelBuffer};
use swash::scale::image::Content as SwashContent;
use swash::scale::{Render, ScaleContext, Source, StrikeWith};
use swash::zeno::Format;
use swash::FontRef;

/// Resultado do processamento de uma tecla no `DocumentEditor`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocumentKeyOutcome {
    /// Texto foi modificado (exige re-render e auto-save).
    Modified,
    /// Apenas cursor, seleção ou rolagem mudou (exige re-render).
    CursorMoved,
    /// Usuário pressionou Ctrl+C com texto selecionado.
    Copy(String),
    /// Usuário pressionou Ctrl+X com texto selecionado.
    Cut(String),
    /// Usuário pressionou Ctrl+V.
    PasteRequested,
    /// Tecla não tratada pelo editor.
    Ignored,
}

/// Cores temáticas para renderização do documento.
#[derive(Debug, Clone, Copy)]
pub struct DocumentThemeColors {
    pub background: [u8; 4],
    pub text_primary: [u8; 4],
    pub text_muted: [u8; 4],
    pub accent: [u8; 4],
    pub selection: [u8; 4],
    pub surface: [u8; 4],
}

/// Controlador do editor contínuo de documento.
pub struct DocumentEditor {
    font_cx: FontContext,
    layout_cx: LayoutContext<[u8; 4]>,
    scale_cx: ScaleContext,
    editor: PlainEditor<[u8; 4]>,
    pub scroll_y: f32,
    pub viewport_w: f32,
    pub viewport_h: f32,
    pub padding: f32,
    pub cursor_visible: bool,
    undo_stack: Vec<String>,
    redo_stack: Vec<String>,
}

impl Default for DocumentEditor {
    fn default() -> Self {
        Self::new(15.0)
    }
}

impl DocumentEditor {
    /// Cria um novo editor contínuo com tamanho de fonte base.
    pub fn new(font_size: f32) -> Self {
        let mut editor = PlainEditor::new(font_size);
        editor.set_scale(1.0);
        let styles = editor.edit_styles();
        styles.insert(StyleProperty::LineHeight(parley::LineHeight::FontSizeRelative(1.45)));

        Self {
            font_cx: FontContext::new(),
            layout_cx: LayoutContext::new(),
            scale_cx: ScaleContext::new(),
            editor,
            scroll_y: 0.0,
            viewport_w: 800.0,
            viewport_h: 600.0,
            padding: 28.0,
            cursor_visible: true,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
        }
    }

    /// Define o texto completo do documento (limpa histórico de undo/redo).
    pub fn set_text(&mut self, text: &str) {
        self.editor.set_text(text);
        self.scroll_y = 0.0;
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    /// Retorna o texto atual completo do documento.
    pub fn text(&self) -> String {
        self.editor.text().chars().collect()
    }

    /// Atualiza as dimensões da viewport visível e a largura de quebra de linha.
    pub fn set_viewport(&mut self, width: f32, height: f32) {
        let w = width.max(120.0);
        let h = height.max(120.0);
        self.viewport_w = w;
        self.viewport_h = h;
        let text_w = (w - self.padding * 2.0).max(80.0);
        self.editor.set_width(Some(text_w));
    }

    /// Rola o documento verticalmente respeitando os limites do conteúdo.
    pub fn scroll_by(&mut self, delta_y: f32) {
        let layout = self.editor.layout(&mut self.font_cx, &mut self.layout_cx);
        let max_scroll = (layout.height() + self.padding * 2.0 - self.viewport_h).max(0.0);
        self.scroll_y = (self.scroll_y - delta_y).clamp(0.0, max_scroll);
    }

    /// Posiciona o cursor ao clicar com o mouse na superfície do documento.
    pub fn handle_pointer_down(&mut self, x: f32, y: f32, shift: bool) {
        let doc_x = (x - self.padding).max(0.0);
        let doc_y = (y - self.padding + self.scroll_y).max(0.0);
        let mut drv = self.editor.driver(&mut self.font_cx, &mut self.layout_cx);
        if shift {
            drv.extend_selection_to_point(doc_x, doc_y);
        } else {
            drv.move_to_point(doc_x, doc_y);
        }
        self.cursor_visible = true;
    }

    /// Estende a seleção ao arrastar o mouse.
    pub fn handle_pointer_move(&mut self, x: f32, y: f32) {
        let doc_x = (x - self.padding).max(0.0);
        let doc_y = (y - self.padding + self.scroll_y).max(0.0);
        let mut drv = self.editor.driver(&mut self.font_cx, &mut self.layout_cx);
        drv.extend_selection_to_point(doc_x, doc_y);
    }

    /// Retorna o texto atualmente selecionado, se houver.
    pub fn selected_text(&self) -> Option<String> {
        self.editor.selected_text().map(|s| s.to_string())
    }

    fn push_undo(&mut self) {
        let snap = self.text();
        if self.undo_stack.last().map(|s| s == &snap).unwrap_or(false) {
            return;
        }
        self.undo_stack.push(snap);
        if self.undo_stack.len() > 80 {
            self.undo_stack.remove(0);
        }
        self.redo_stack.clear();
    }

    /// Insere texto na posição atual do cursor (substituindo seleção se houver).
    pub fn insert_text(&mut self, s: &str) {
        if s.is_empty() {
            return;
        }
        self.push_undo();
        let mut drv = self.editor.driver(&mut self.font_cx, &mut self.layout_cx);
        drv.insert_or_replace_selection(s);
        self.cursor_visible = true;
        self.ensure_cursor_visible();
    }

    /// Garante que o cursor permaneça visível na viewport após digitar/navegar.
    fn ensure_cursor_visible(&mut self) {
        let _ = self.editor.layout(&mut self.font_cx, &mut self.layout_cx);
        if let Some(cursor_rect) = self.editor.cursor_geometry(2.0) {
            let cy_top = cursor_rect.y0 as f32 + self.padding;
            let cy_bot = cursor_rect.y1 as f32 + self.padding;
            if cy_top < self.scroll_y + 16.0 {
                self.scroll_y = (cy_top - 16.0).max(0.0);
            } else if cy_bot > self.scroll_y + self.viewport_h - 16.0 {
                self.scroll_y = (cy_bot - self.viewport_h + 24.0).max(0.0);
            }
        }
    }

    /// Processa eventos de teclado vindos do `FocusScope` do Slint.
    pub fn handle_key_event(
        &mut self,
        key_text: &str,
        ctrl: bool,
        shift: bool,
        alt: bool,
    ) -> DocumentKeyOutcome {
        if alt {
            return DocumentKeyOutcome::Ignored;
        }

        self.cursor_visible = true;

        // Atalhos com Ctrl
        if ctrl {
            let lower = key_text.to_ascii_lowercase();
            match lower.as_str() {
                "a" => {
                    self.editor.driver(&mut self.font_cx, &mut self.layout_cx).select_all();
                    return DocumentKeyOutcome::CursorMoved;
                }
                "c" => {
                    if let Some(sel) = self.selected_text() {
                        if !sel.is_empty() {
                            return DocumentKeyOutcome::Copy(sel);
                        }
                    }
                    return DocumentKeyOutcome::Ignored;
                }
                "x" => {
                    if let Some(sel) = self.selected_text() {
                        if !sel.is_empty() {
                            self.push_undo();
                            self.editor
                                .driver(&mut self.font_cx, &mut self.layout_cx)
                                .delete_selection();
                            return DocumentKeyOutcome::Cut(sel);
                        }
                    }
                    return DocumentKeyOutcome::Ignored;
                }
                "v" => return DocumentKeyOutcome::PasteRequested,
                "z" => {
                    if let Some(prev) = self.undo_stack.pop() {
                        self.redo_stack.push(self.text());
                        self.editor.set_text(&prev);
                        return DocumentKeyOutcome::Modified;
                    }
                    return DocumentKeyOutcome::Ignored;
                }
                "y" => {
                    if let Some(next) = self.redo_stack.pop() {
                        self.undo_stack.push(self.text());
                        self.editor.set_text(&next);
                        return DocumentKeyOutcome::Modified;
                    }
                    return DocumentKeyOutcome::Ignored;
                }
                _ => {}
            }
        }

        // Teclas especiais Slint (Private Use Area Unicode)
        match key_text {
            // Backspace (\u{0008})
            "\u{0008}" => {
                self.push_undo();
                let mut drv = self.editor.driver(&mut self.font_cx, &mut self.layout_cx);
                if ctrl {
                    drv.backdelete_word();
                } else {
                    drv.backdelete();
                }
                self.ensure_cursor_visible();
                return DocumentKeyOutcome::Modified;
            }
            // Delete (\u{007f})
            "\u{007f}" => {
                self.push_undo();
                let mut drv = self.editor.driver(&mut self.font_cx, &mut self.layout_cx);
                if ctrl {
                    drv.delete_word();
                } else {
                    drv.delete();
                }
                self.ensure_cursor_visible();
                return DocumentKeyOutcome::Modified;
            }
            // Return / Enter (\n or \r)
            "\n" | "\r" => {
                self.push_undo();
                self.editor
                    .driver(&mut self.font_cx, &mut self.layout_cx)
                    .insert_or_replace_selection("\n");
                self.ensure_cursor_visible();
                return DocumentKeyOutcome::Modified;
            }
            // Tab (\t)
            "\t" => {
                self.push_undo();
                self.editor
                    .driver(&mut self.font_cx, &mut self.layout_cx)
                    .insert_or_replace_selection("    ");
                self.ensure_cursor_visible();
                return DocumentKeyOutcome::Modified;
            }
            // LeftArrow (\u{f702})
            "\u{f702}" => {
                let mut drv = self.editor.driver(&mut self.font_cx, &mut self.layout_cx);
                match (ctrl, shift) {
                    (true, true) => drv.select_word_left(),
                    (true, false) => drv.move_word_left(),
                    (false, true) => drv.select_left(),
                    (false, false) => drv.move_left(),
                }
                self.ensure_cursor_visible();
                return DocumentKeyOutcome::CursorMoved;
            }
            // RightArrow (\u{f703})
            "\u{f703}" => {
                let mut drv = self.editor.driver(&mut self.font_cx, &mut self.layout_cx);
                match (ctrl, shift) {
                    (true, true) => drv.select_word_right(),
                    (true, false) => drv.move_word_right(),
                    (false, true) => drv.select_right(),
                    (false, false) => drv.move_right(),
                }
                self.ensure_cursor_visible();
                return DocumentKeyOutcome::CursorMoved;
            }
            // UpArrow (\u{f700})
            "\u{f700}" => {
                let mut drv = self.editor.driver(&mut self.font_cx, &mut self.layout_cx);
                if shift {
                    drv.select_up();
                } else {
                    drv.move_up();
                }
                self.ensure_cursor_visible();
                return DocumentKeyOutcome::CursorMoved;
            }
            // DownArrow (\u{f701})
            "\u{f701}" => {
                let mut drv = self.editor.driver(&mut self.font_cx, &mut self.layout_cx);
                if shift {
                    drv.select_down();
                } else {
                    drv.move_down();
                }
                self.ensure_cursor_visible();
                return DocumentKeyOutcome::CursorMoved;
            }
            // Home (\u{f729})
            "\u{f729}" => {
                let mut drv = self.editor.driver(&mut self.font_cx, &mut self.layout_cx);
                match (ctrl, shift) {
                    (true, true) => drv.select_to_text_start(),
                    (true, false) => drv.move_to_text_start(),
                    (false, true) => drv.select_to_line_start(),
                    (false, false) => drv.move_to_line_start(),
                }
                self.ensure_cursor_visible();
                return DocumentKeyOutcome::CursorMoved;
            }
            // End (\u{f72b})
            "\u{f72b}" => {
                let mut drv = self.editor.driver(&mut self.font_cx, &mut self.layout_cx);
                match (ctrl, shift) {
                    (true, true) => drv.select_to_text_end(),
                    (true, false) => drv.move_to_text_end(),
                    (false, true) => drv.select_to_line_end(),
                    (false, false) => drv.move_to_line_end(),
                }
                self.ensure_cursor_visible();
                return DocumentKeyOutcome::CursorMoved;
            }
            _ => {}
        }

        // Inserção normal de caracteres imprimíveis
        if !ctrl && !key_text.is_empty() && key_text.chars().all(|c| !c.is_control()) {
            self.push_undo();
            self.editor
                .driver(&mut self.font_cx, &mut self.layout_cx)
                .insert_or_replace_selection(key_text);
            self.ensure_cursor_visible();
            return DocumentKeyOutcome::Modified;
        }

        DocumentKeyOutcome::Ignored
    }

    /// Rasteriza apenas a viewport visível do documento em um `slint::Image` RGBA8 (100% CPU/Software).
    pub fn render_to_slint_image(&mut self, theme: DocumentThemeColors) -> Image {
        let w = (self.viewport_w.round() as u32).max(1);
        let h = (self.viewport_h.round() as u32).max(1);
        let mut pixel_buffer = SharedPixelBuffer::<Rgba8Pixel>::new(w, h);
        let pixels = pixel_buffer.make_mut_bytes();

        // Preenche fundo
        for chunk in pixels.chunks_exact_mut(4) {
            chunk.copy_from_slice(&theme.background);
        }

        // Atualiza estilos base do texto com a cor do tema
        self.editor
            .edit_styles()
            .insert(StyleProperty::Brush(theme.text_primary));

        let padding = self.padding;
        let scroll_y = self.scroll_y;
        let _ = self.editor.layout(&mut self.font_cx, &mut self.layout_cx);

        // 1. Desenha retângulos de seleção
        for (bbox, _) in self.editor.selection_geometry() {
            let x0 = (bbox.x0 as f32 + padding).round() as i32;
            let y0 = (bbox.y0 as f32 + padding - scroll_y).round() as i32;
            let x1 = (bbox.x1 as f32 + padding).round() as i32;
            let y1 = (bbox.y1 as f32 + padding - scroll_y).round() as i32;
            fill_rect_blend(pixels, w, h, x0, y0, x1 - x0, y1 - y0, theme.selection);
        }

        // 2. Desenha linhas visíveis + glifos via Swash
        let full_text = self.text();
        let layout = self.editor.layout(&mut self.font_cx, &mut self.layout_cx);

        for line in layout.lines() {
            let metrics = line.metrics();
            let line_top = (metrics.baseline - metrics.line_height * 0.8) + padding - scroll_y;
            let line_bot = line_top + metrics.line_height;
            if line_bot < 0.0 || line_top > h as f32 {
                continue;
            }

            // Decoração sutil estilo Word/Markdown para títulos, citações e código
            let range = line.text_range();
            let line_slice = full_text.get(range.start..range.end).unwrap_or("");
            let trimmed = line_slice.trim_start();
            if trimmed.starts_with('#') {
                fill_rect_blend(
                    pixels,
                    w,
                    h,
                    (padding - 10.0) as i32,
                    line_top as i32 + 2,
                    3,
                    (line_bot - line_top).max(4.0) as i32 - 4,
                    theme.accent,
                );
            } else if trimmed.starts_with('>') {
                fill_rect_blend(
                    pixels,
                    w,
                    h,
                    padding as i32,
                    line_top as i32,
                    (w as i32) - (padding as i32) * 2,
                    (line_bot - line_top).max(4.0) as i32,
                    [theme.surface[0], theme.surface[1], theme.surface[2], 140],
                );
                fill_rect_blend(
                    pixels,
                    w,
                    h,
                    padding as i32,
                    line_top as i32,
                    3,
                    (line_bot - line_top).max(4.0) as i32,
                    theme.accent,
                );
            } else if trimmed.starts_with("```") {
                fill_rect_blend(
                    pixels,
                    w,
                    h,
                    padding as i32,
                    line_top as i32,
                    (w as i32) - (padding as i32) * 2,
                    (line_bot - line_top).max(4.0) as i32,
                    [theme.surface[0], theme.surface[1], theme.surface[2], 180],
                );
            }

            for item in line.items() {
                if let PositionedLayoutItem::GlyphRun(glyph_run) = item {
                    let run = glyph_run.run();
                    let font_data = run.font();
                    let Some(font_ref) =
                        FontRef::from_index(font_data.data.as_ref(), font_data.index as usize)
                    else {
                        continue;
                    };

                    let color = if trimmed.starts_with('#') {
                        theme.accent
                    } else if trimmed.starts_with("```") || trimmed.starts_with('>') {
                        theme.text_muted
                    } else {
                        glyph_run.style().brush
                    };

                    let mut scaler = self
                        .scale_cx
                        .builder(font_ref)
                        .size(run.font_size())
                        .normalized_coords(run.normalized_coords().iter().map(|c| c.to_bits()))
                        .build();

                    for glyph in glyph_run.positioned_glyphs() {
                        if glyph.id == 0 {
                            continue;
                        }
                        let Some(img) = Render::new(&[
                            Source::ColorOutline(0),
                            Source::ColorBitmap(StrikeWith::BestFit),
                            Source::Outline,
                        ])
                        .format(Format::Alpha)
                        .render(&mut scaler, glyph.id as u16)
                        else {
                            continue;
                        };

                        let gx = (glyph.x + padding).round() as i32 + img.placement.left;
                        let gy =
                            (glyph.y + padding - scroll_y).round() as i32 - img.placement.top;
                        let gw = img.placement.width as i32;
                        let gh = img.placement.height as i32;

                        match img.content {
                            SwashContent::Mask => {
                                for py in 0..gh {
                                    let dy = gy + py;
                                    if dy < 0 || dy >= h as i32 {
                                        continue;
                                    }
                                    for px in 0..gw {
                                        let dx = gx + px;
                                        if dx < 0 || dx >= w as i32 {
                                            continue;
                                        }
                                        let alpha = img.data[(py * gw + px) as usize];
                                        if alpha > 0 {
                                            blend_pixel(
                                                pixels,
                                                w,
                                                dx as u32,
                                                dy as u32,
                                                [color[0], color[1], color[2], alpha],
                                            );
                                        }
                                    }
                                }
                            }
                            SwashContent::Color => {
                                for py in 0..gh {
                                    let dy = gy + py;
                                    if dy < 0 || dy >= h as i32 {
                                        continue;
                                    }
                                    for px in 0..gw {
                                        let dx = gx + px;
                                        if dx < 0 || dx >= w as i32 {
                                            continue;
                                        }
                                        let src_idx = ((py * gw + px) * 4) as usize;
                                        if src_idx + 3 < img.data.len() {
                                            blend_pixel(
                                                pixels,
                                                w,
                                                dx as u32,
                                                dy as u32,
                                                [
                                                    img.data[src_idx],
                                                    img.data[src_idx + 1],
                                                    img.data[src_idx + 2],
                                                    img.data[src_idx + 3],
                                                ],
                                            );
                                        }
                                    }
                                }
                            }
                            SwashContent::SubpixelMask => {}
                        }
                    }
                }
            }
        }

        // 3. Desenha cursor piscante
        if self.cursor_visible {
            if let Some(cbox) = self.editor.cursor_geometry(2.0) {
                let cx = (cbox.x0 as f32 + padding).round() as i32;
                let cy = (cbox.y0 as f32 + padding - scroll_y).round() as i32;
                let ch = (cbox.y1 - cbox.y0).max(14.0).round() as i32;
                fill_rect_blend(pixels, w, h, cx, cy, 2, ch, theme.accent);
            }
        }

        Image::from_rgba8(pixel_buffer)
    }
}

fn fill_rect_blend(
    pixels: &mut [u8],
    buf_w: u32,
    buf_h: u32,
    x: i32,
    y: i32,
    rw: i32,
    rh: i32,
    rgba: [u8; 4],
) {
    if rw <= 0 || rh <= 0 || rgba[3] == 0 {
        return;
    }
    let x0 = x.max(0) as u32;
    let y0 = y.max(0) as u32;
    let x1 = (x + rw).min(buf_w as i32).max(0) as u32;
    let y1 = (y + rh).min(buf_h as i32).max(0) as u32;
    for py in y0..y1 {
        for px in x0..x1 {
            blend_pixel(pixels, buf_w, px, py, rgba);
        }
    }
}

fn blend_pixel(pixels: &mut [u8], buf_w: u32, x: u32, y: u32, src: [u8; 4]) {
    let idx = ((y * buf_w + x) * 4) as usize;
    if idx + 3 >= pixels.len() {
        return;
    }
    let a = src[3] as u32;
    if a == 255 {
        pixels[idx] = src[0];
        pixels[idx + 1] = src[1];
        pixels[idx + 2] = src[2];
        pixels[idx + 3] = 255;
        return;
    }
    if a == 0 {
        return;
    }
    let inv = 255 - a;
    pixels[idx] = ((src[0] as u32 * a + pixels[idx] as u32 * inv) / 255) as u8;
    pixels[idx + 1] = ((src[1] as u32 * a + pixels[idx + 1] as u32 * inv) / 255) as u8;
    pixels[idx + 2] = ((src[2] as u32 * a + pixels[idx + 2] as u32 * inv) / 255) as u8;
    pixels[idx + 3] = 255;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_document_editor_typing_undo_and_software_render() {
        let mut doc = DocumentEditor::new(15.0);
        doc.set_viewport(640.0, 400.0);
        doc.set_text("# Título\nOlá mundo");
        assert_eq!(doc.text(), "# Título\nOlá mundo");

        doc.insert_text("!");
        assert!(doc.text().contains("!"));

        let undo = doc.handle_key_event("z", true, false, false);
        assert_eq!(undo, DocumentKeyOutcome::Modified);

        let img = doc.render_to_slint_image(DocumentThemeColors {
            background: [24, 24, 37, 255],
            text_primary: [205, 214, 244, 255],
            text_muted: [166, 173, 200, 255],
            accent: [137, 180, 250, 255],
            selection: [137, 180, 250, 80],
            surface: [49, 50, 68, 255],
        });
        assert_eq!(img.size().width, 640);
        assert_eq!(img.size().height, 400);
    }
}
