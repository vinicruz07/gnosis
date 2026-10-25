//! Gnosis Core: Gerenciamento de Vaults, indexação e monitoramento de arquivos.

pub mod error;
pub mod vault;
pub mod watcher;

pub use error::{CoreError, Result};
pub use vault::Vault;
