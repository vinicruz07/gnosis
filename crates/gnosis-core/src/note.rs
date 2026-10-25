//! Modelo de dados para notas do Vault (`VaultNote` e `NoteId`).

use crate::buffer::NoteBuffer;
use crate::path::VaultPath;
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
    buffer: NoteBuffer,
    is_dirty: bool,
    modified_timestamp_millis: Option<u64>,
}

impl VaultNote {
    /// Inicializa uma nova nota a partir de um caminho e conteúdo textual.
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
            buffer: NoteBuffer::from_str(initial_content),
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

    /// Define o título da nota (ex: obtido do frontmatter YAML).
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
