//! Gnosis App: Shell de interface gráfica nativa em Slint.
//!
//! Diretrizes de Qualidade:
//! - 100% Rust Seguro (`#![forbid(unsafe_code)]`).
//! - Zero pânicos em tempo de execução (`clippy::unwrap_used`, `clippy::expect_used`, `clippy::panic`).
//! - Interface leve e responsiva (FemtoVG/Software) calibrada para hardware de 2015-.

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]

pub mod state;

pub use state::{AppState, FileTreeEntry, TreeItemModel};

// Inclui os módulos e componentes de janela compilados pelo Slint no build.rs
slint::include_modules!();
