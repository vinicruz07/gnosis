//! Gnosis FS Native: Implementação de I/O de disco para Desktop e modo Portable.

pub mod error;
pub mod paths;
pub mod provider;

pub use paths::StoragePaths;
pub use provider::NativeFileSystem;
