//! Gnosis Core: Gerenciamento de Vaults, indexação e abstração de sistema de arquivos.
//!
//! Garantias de Qualidade:
//! - 100% Rust Seguro (`#![forbid(unsafe_code)]`).
//! - Zero pânicos em tempo de execução (`clippy::unwrap_used`, `clippy::expect_used`, `clippy::panic`).
//! - Tratamento tipado de erros com [`thiserror`].

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]

pub mod buffer;
pub mod canvas;
pub mod config;
pub mod error;
pub mod index;
pub mod note;
pub mod path;
pub mod vault;
pub mod vfs;
pub mod watcher;

pub use buffer::NoteBuffer;
pub use canvas::{
    BackgroundStyle, CanvasData, CanvasEdge, CanvasNode, EdgeEnd, FileNode, GroupNode, LinkNode,
    NodeSide, TextNode,
};
pub use config::{VaultConfig, GNOSIS_CONFIG_PATH, OBSIDIAN_WORKSPACE_PATH};
pub use error::{CoreError, Result};
pub use index::VaultIndex;
pub use note::{NoteId, VaultNote};
pub use path::VaultPath;
pub use vault::Vault;
pub use vfs::{FileMetadata, FileSystemProvider, MemoryFileSystem};
