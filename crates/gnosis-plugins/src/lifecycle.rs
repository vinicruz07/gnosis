//! Ciclo de vida controlado de plugins (`on_load`, `on_unload`, `on_event`).
//!
//! Cada plugin implementa a trait assíncrona [`Plugin`], mantendo estado isolado
//! e reagindo aos eventos enviados pelo barramento central.

use crate::error::Result;
use crate::events::{EventBus, PluginEvent};
use crate::manifest::{PluginManifest, PluginPermission};
use crate::ui::UiRegistry;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Estado operacional de um plugin dentro do Gnosis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginState {
    /// O manifesto foi descoberto e registrado, mas o código não foi carregado na memória.
    Unloaded,
    /// O plugin foi carregado e inicializado com sucesso via `on_load`.
    Loaded,
    /// O plugin está ativo e escutando eventos do barramento.
    Active,
    /// O plugin sofreu uma falha em seu ciclo de vida ou pânico na execução.
    Error(String),
}

impl std::fmt::Display for PluginState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unloaded => write!(f, "Descarregado"),
            Self::Loaded => write!(f, "Carregado"),
            Self::Active => write!(f, "Ativo"),
            Self::Error(msg) => write!(f, "Erro: {}", msg),
        }
    }
}

/// Contexto e capacidades passados para o plugin durante o ciclo de vida.
#[derive(Clone)]
pub struct PluginContext {
    pub plugin_id: String,
    pub event_bus: Arc<EventBus>,
    pub permissions: Vec<PluginPermission>,
    pub ui_registry: Arc<UiRegistry>,
}

impl PluginContext {
    /// Cria um novo contexto para o plugin fornecido com registro de UI integrado.
    pub fn new(
        plugin_id: impl Into<String>,
        event_bus: Arc<EventBus>,
        permissions: Vec<PluginPermission>,
        ui_registry: Arc<UiRegistry>,
    ) -> Self {
        Self {
            plugin_id: plugin_id.into(),
            event_bus,
            permissions,
            ui_registry,
        }
    }

    /// Cria um contexto com registro de UI padrão em memória (útil para testes isolados).
    pub fn new_isolated(
        plugin_id: impl Into<String>,
        event_bus: Arc<EventBus>,
        permissions: Vec<PluginPermission>,
    ) -> Self {
        Self {
            plugin_id: plugin_id.into(),
            event_bus,
            permissions,
            ui_registry: Arc::new(UiRegistry::new()),
        }
    }

    /// Verifica se uma permissão foi concedida a este contexto.
    pub fn has_permission(&self, perm: PluginPermission) -> bool {
        self.permissions.contains(&perm)
    }
}

/// Trait central que define o ciclo de vida e a interface de um plugin do Gnosis.
#[async_trait]
pub trait Plugin: Send + Sync {
    /// Retorna uma referência ao manifesto do plugin.
    fn manifest(&self) -> &PluginManifest;

    /// Invocado quando o plugin é carregado na memória e ativado.
    async fn on_load(&mut self, context: &PluginContext) -> Result<()>;

    /// Invocado quando o plugin é desativado ou descarregado da memória.
    async fn on_unload(&mut self) -> Result<()>;

    /// Invocado quando um evento do barramento é despachado a este plugin.
    async fn on_event(&mut self, event: &PluginEvent) -> Result<()>;
}

/// Plugin demonstrativo em memória para testes de ciclo de vida.
#[derive(Debug, Clone)]
pub struct MockPlugin {
    manifest: PluginManifest,
    pub loaded: bool,
    pub events_received: Vec<PluginEvent>,
}

impl MockPlugin {
    pub fn new(id: &str, name: &str) -> Self {
        Self {
            manifest: PluginManifest::new(
                id,
                name,
                "1.0.0",
                "Plugin demonstrativo para validação de ciclo de vida",
                "Gnosis Core Team",
                "index.js",
            ),
            loaded: false,
            events_received: Vec::new(),
        }
    }
}

#[async_trait]
impl Plugin for MockPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    async fn on_load(&mut self, _context: &PluginContext) -> Result<()> {
        self.loaded = true;
        Ok(())
    }

    async fn on_unload(&mut self) -> Result<()> {
        self.loaded = false;
        Ok(())
    }

    async fn on_event(&mut self, event: &PluginEvent) -> Result<()> {
        self.events_received.push(event.clone());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gnosis_core::path::VaultPath;

    #[tokio::test]
    async fn test_mock_plugin_lifecycle() -> Result<()> {
        let mut plugin = MockPlugin::new("mock-test", "Mock Test");
        assert_eq!(plugin.manifest().id, "mock-test");
        assert!(!plugin.loaded);

        let bus = Arc::new(EventBus::new(16));
        let ctx = PluginContext::new_isolated("mock-test", bus, vec![PluginPermission::ReadNotes]);

        plugin.on_load(&ctx).await?;
        assert!(plugin.loaded);

        let path = VaultPath::parse("diario.md").expect("valid path");
        let ev = PluginEvent::NoteOpened {
            path,
            title: "Diário".to_string(),
        };

        plugin.on_event(&ev).await?;
        assert_eq!(plugin.events_received.len(), 1);
        assert_eq!(plugin.events_received[0], ev);

        plugin.on_unload().await?;
        assert!(!plugin.loaded);

        Ok(())
    }
}
