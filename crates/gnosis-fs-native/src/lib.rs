//! Gnosis FS Native: Implementação de I/O de disco para Desktop e modo Portable.
//!
//! Diretrizes de Qualidade:
//! - 100% Rust Seguro (`#![forbid(unsafe_code)]`).
//! - Zero pânicos em tempo de execução (`clippy::unwrap_used`, `clippy::expect_used`, `clippy::panic`).
//! - Escrita atômica em disco para proteção contra falhas e quedas de energia.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]

pub mod error;
pub mod paths;
pub mod provider;

pub use error::{NativeFsError, Result};
pub use paths::{AppMode, StoragePaths};
pub use provider::NativeFileSystem;
