//! Gnosis Plugins: Subsistema de extensões, manifesto formal e barramento de eventos.
//!
//! Em conformidade com os princípios arquiteturais do Gnosis:
//! - 100% Rust Seguro (`#![forbid(unsafe_code)]`).
//! - Zero pânicos em tempo de execução.
//! - Descoberta automática de plugins na pasta `.gnosis/plugins/`.
//! - Barramento de eventos assíncrono desacoplado para integração reativa.
//! - Runtime de sandbox isolado com proteção de memória, timeout e contenção de falhas.

pub mod error;
pub mod events;
pub mod lifecycle;
pub mod manager;
pub mod manifest;
pub mod sandbox;
pub mod ui;

pub use error::{PluginError, Result};
pub use events::{EventBus, EventType, PluginEvent};
pub use lifecycle::{MockPlugin, Plugin, PluginContext, PluginState};
pub use manager::PluginManager;
pub use manifest::{PluginManifest, PluginPermission};
pub use sandbox::{PluginSandbox, PluginSandboxConfig, SandboxFileSystem};
pub use ui::{
    StatusBarAlignment, StatusBarItem, ToolbarItem, UiCommand, UiRegistry, WordCounterDemoPlugin,
};
