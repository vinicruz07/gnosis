//! Gnosis FS Native: Implementação de I/O de disco para Desktop e modo Portable.
//!
//! Diretrizes de Qualidade:
//! - 100% Rust Seguro (`#![forbid(unsafe_code)]`).
//! - Zero pânicos em tempo de execução (`clippy::unwrap_used`, `clippy::expect_used`, `clippy::panic`).
//! - Escrita atômica em disco para proteção contra falhas e quedas de energia.
//! - Varredura multithreaded e monitoramento em tempo real com [`notify`].

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]

pub mod app_config;
pub mod error;
pub mod paths;
pub mod provider;
pub mod scanner;
pub mod trash;
pub mod watcher;

pub use app_config::{GlobalAppConfig, VaultEntry, GLOBAL_APP_CONFIG_FILE};
pub use error::{NativeFsError, Result};
pub use paths::{AppMode, StoragePaths};
pub use provider::NativeFileSystem;
pub use scanner::{AttachmentInfo, AttachmentKind, ScanReport, VaultScanner};
pub use trash::{TrashManifest, TrashedItem, VaultTrash, TRASH_FOLDER_NAME};
pub use watcher::{VaultChangeEvent, VaultFileWatcher};
