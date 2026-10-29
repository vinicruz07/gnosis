//! Gnosis FS Web: Implementação de VFS para navegador (OPFS / IndexedDB).
//!
//! Fornece armazenamento resiliente de alta velocidade e persistente no cliente
//! para execução autônoma via WebAssembly (WASM).

pub mod error;
pub mod provider;
pub mod pwa;

pub use error::{Result, WebFsError};
pub use provider::{WebFileSystem, WebFsConfig, WebStorageType};
pub use pwa::{PwaBundleValidator, PwaIcon, PwaManifest};
