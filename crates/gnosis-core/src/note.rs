//! Modelo de dados para notas do Vault (`VaultNote` e `NoteId`).

use crate::buffer::NoteBuffer;
use crate::path::VaultPath;
use gnosis_markdown::{ParsedMetadata, WikiLink};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Identificador único e estável de uma nota dentro do Vault.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct NoteId(String);

impl NoteId {
    /// Cria um ID a partir de uma string arbitrária.
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Deriva um ID canônico a partir do caminho da nota no Vault.
    pub fn from_path(path: &VaultPath) -> Self {
        Self(path.as_str().to_string())
    }

    /// Retorna o valor do ID como `&str`.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for NoteId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NoteId(\"{}\")", self.0)
    }
}

impl fmt::Display for NoteId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Representa uma nota completa aberta na memória do Vault.
#[derive(Debug, Clone)]
pub struct VaultNote {
    id: NoteId,
    path: VaultPath,
    title: String,
    aliases: Vec<String>,
    tags: Vec<String>,
    links: Vec<WikiLink>,
    icon: Option<String>,
    sticker: Option<String>,
    color: Option<String>,
    buffer: NoteBuffer,
    is_dirty: bool,
    modified_timestamp_millis: Option<u64>,
}

impl VaultNote {
    /// Inicializa uma nova nota básica a partir de um caminho e conteúdo textual.
    pub fn new(path: VaultPath, initial_content: &str) -> Self {
        let id = NoteId::from_path(&path);
        let title = path
            .file_stem()
            .unwrap_or("Untitled")
            .to_string();

        Self {
            id,
            path,
            title,
            aliases: Vec::new(),
            tags: Vec::new(),
            links: Vec::new(),
            icon: None,
            sticker: None,
            color: None,
            buffer: NoteBuffer::from_str(initial_content),
            is_dirty: false,
            modified_timestamp_millis: None,
        }
    }

    /// Construtor inteligente que analisa o texto Markdown e extrai metadados, frontmatter, tags e links.
    pub fn from_raw_markdown(path: VaultPath, raw_content: &str) -> Self {
        let id = NoteId::from_path(&path);
        let parsed = ParsedMetadata::parse(raw_content);

        // Se o frontmatter definir um título explícito, utiliza-o; senão usa o nome do arquivo
        let title = match &parsed.frontmatter {
            Some(fm) if fm.title.is_some() => {
                match &fm.title {
                    Some(t) => t.clone(),
                    None => path.file_stem().unwrap_or("Untitled").to_string(),
                }
            }
            _ => path.file_stem().unwrap_or("Untitled").to_string(),
        };

        let aliases = match &parsed.frontmatter {
            Some(fm) => fm.aliases.clone(),
            None => Vec::new(),
        };

        let (icon, sticker, color) = match &parsed.frontmatter {
            Some(fm) => (fm.icon.clone(), fm.sticker.clone(), fm.color.clone()),
            None => (None, None, None),
        };

        Self {
            id,
            path,
            title,
            aliases,
            tags: parsed.tags,
            links: parsed.links,
            icon,
            sticker,
            color,
            buffer: NoteBuffer::from_str(raw_content),
            is_dirty: false,
            modified_timestamp_millis: None,
        }
    }

    /// Retorna o identificador único da nota.
    pub fn id(&self) -> &NoteId {
        &self.id
    }

    /// Retorna o caminho relativo da nota no Vault.
    pub fn path(&self) -> &VaultPath {
        &self.path
    }

    /// Atualiza o caminho da nota (ex: quando renomeada ou movida).
    pub fn set_path(&mut self, new_path: VaultPath) {
        self.id = NoteId::from_path(&new_path);
        self.path = new_path;
        self.is_dirty = true;
    }

    /// Retorna o título da nota.
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Define o título da nota.
    pub fn set_title(&mut self, title: impl Into<String>) {
        self.title = title.into();
    }

    /// Retorna a lista de apelidos (aliases) associados à nota.
    pub fn aliases(&self) -> &[String] {
        &self.aliases
    }

    /// Atualiza a lista de aliases.
    pub fn set_aliases(&mut self, aliases: Vec<String>) {
        self.aliases = aliases;
    }

    /// Retorna a lista de tags da nota.
    pub fn tags(&self) -> &[String] {
        &self.tags
    }

    /// Retorna os links e embeds contidos na nota.
    pub fn links(&self) -> &[WikiLink] {
        &self.links
    }

    /// Retorna o emoji ou ícone da nota, se houver.
    pub fn icon(&self) -> Option<&str> {
        self.icon.as_deref()
    }

    /// Define o emoji ou ícone da nota.
    pub fn set_icon(&mut self, icon: Option<impl Into<String>>) {
        self.icon = icon.map(|s| s.into());
        self.is_dirty = true;
    }

    /// Retorna o sticker ou arquivo de imagem vetorial da nota.
    pub fn sticker(&self) -> Option<&str> {
        self.sticker.as_deref()
    }

    /// Define o sticker da nota.
    pub fn set_sticker(&mut self, sticker: Option<impl Into<String>>) {
        self.sticker = sticker.map(|s| s.into());
        self.is_dirty = true;
    }

    /// Retorna a cor personalizada da nota.
    pub fn color(&self) -> Option<&str> {
        self.color.as_deref()
    }

    /// Define a cor personalizada da nota.
    pub fn set_color(&mut self, color: Option<impl Into<String>>) {
        self.color = color.map(|s| s.into());
        self.is_dirty = true;
    }

    /// Retorna o ícone visual prioritário (ícone explícito ou sticker).
    pub fn visual_icon(&self) -> Option<&str> {
        self.icon.as_deref().or(self.sticker.as_deref())
    }

    /// Atualiza sincronizadamente o buffer Markdown e os metadados de sticker/ícone/cor.
    pub fn update_stickers_in_buffer(
        &mut self,
        icon: Option<&str>,
        sticker: Option<&str>,
        color: Option<&str>,
    ) {
        let current_text = self.buffer.to_string();
        let updated_text = gnosis_markdown::update_frontmatter_stickers(
            &current_text,
            icon,
            sticker,
            color,
        );
        self.buffer = NoteBuffer::from_str(&updated_text);
        self.icon = icon.map(|s| s.to_string());
        self.sticker = sticker.map(|s| s.to_string());
        self.color = color.map(|s| s.to_string());
        self.is_dirty = true;
    }

    /// Retorna uma referência somente leitura ao buffer de texto.
    pub fn buffer(&self) -> &NoteBuffer {
        &self.buffer
    }

    /// Retorna uma referência mutável ao buffer de texto, marcando a nota como modificada (dirty).
    pub fn buffer_mut(&mut self) -> &mut NoteBuffer {
        self.is_dirty = true;
        &mut self.buffer
    }

    /// Retorna se a nota possui modificações em memória ainda não persistidas no disco/VFS.
    pub fn is_dirty(&self) -> bool {
        self.is_dirty
    }

    /// Marca a nota como sincronizada (clean).
    pub fn mark_clean(&mut self) {
        self.is_dirty = false;
    }

    /// Define o timestamp da última modificação.
    pub fn set_modified_timestamp(&mut self, millis: Option<u64>) {
        self.modified_timestamp_millis = millis;
    }

    /// Retorna o timestamp da última modificação, se houver.
    pub fn modified_timestamp(&self) -> Option<u64> {
        self.modified_timestamp_millis
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_note_stickers_extraction_and_mutation() {
        let path = match VaultPath::parse("Notas/Gnosis.md") {
            Ok(p) => p,
            Err(_) => return,
        };

        let raw = r##"---
title: Gnosis Docs
icon: 🧠
sticker: custom.svg
color: "#ff5555"
---
# Documentação
Texto da nota aqui.
"##;

        let mut note = VaultNote::from_raw_markdown(path, raw);
        assert_eq!(note.title(), "Gnosis Docs");
        assert_eq!(note.icon(), Some("🧠"));
        assert_eq!(note.sticker(), Some("custom.svg"));
        assert_eq!(note.color(), Some("#ff5555"));
        assert_eq!(note.visual_icon(), Some("🧠"));
        assert!(!note.is_dirty());

        // Atualiza stickers no buffer
        note.update_stickers_in_buffer(Some("🦀"), Some("ferris.svg"), Some("#00ffcc"));
        assert_eq!(note.icon(), Some("🦀"));
        assert_eq!(note.sticker(), Some("ferris.svg"));
        assert_eq!(note.color(), Some("#00ffcc"));
        assert_eq!(note.visual_icon(), Some("🦀"));
        assert!(note.is_dirty());
        assert!(note.buffer().to_string().contains("icon: 🦀"));
        assert!(note.buffer().to_string().contains("sticker: ferris.svg"));
    }
}

