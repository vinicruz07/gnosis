//! Gnosis FS Web: Implementação de VFS para navegador (OPFS / File System Access API / IndexedDB).

pub mod error;
pub mod provider;

pub use error::WebFsError;
pub use provider::WebFileSystem;
