//! API de extensão de interface gráfica para plugins (comandos na paleta, barra de ferramentas e status bar).
//!
//! Permite que extensões registrem comandos no Quick Switcher (`Ctrl+O`), botões de ação
//! rápida e widgets dinâmicos na barra de status inferior.

use crate::error::{PluginError, Result};
use crate::manifest::{PluginManifest, PluginPermission};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Alinhamento de um widget na barra de status inferior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum StatusBarAlignment {
    Left,
    Right,
}

impl Default for StatusBarAlignment {
    fn default() -> Self {
        Self::Right
    }
}

/// Comando de ação registrado por um plugin para exibição na Paleta de Comandos (`Ctrl+O`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiCommand {
    /// Identificador do comando no formato `<plugin_id>:<action>` (ex: "word-count:show-metrics").
    pub id: String,
    /// Rótulo em linguagem natural exibido ao usuário (ex: "Estatísticas Detalhadas do Vault").
    pub title: String,
    /// Atalho de teclado opcional associado (ex: "Ctrl+Shift+W").
    pub hotkey: Option<String>,
    /// Ícone ou emoji decorativo associado ao comando.
    pub icon: Option<String>,
    /// ID do plugin proprietário do comando.
    pub plugin_id: String,
}

impl UiCommand {
    pub fn new(id: impl Into<String>, title: impl Into<String>, plugin_id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            hotkey: None,
            icon: None,
            plugin_id: plugin_id.into(),
        }
    }

    pub fn with_hotkey(mut self, hotkey: impl Into<String>) -> Self {
        self.hotkey = Some(hotkey.into());
        self
    }

    pub fn with_icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = Some(icon.into());
        self
    }
}

/// Botão de ação rápida injetado na barra de ferramentas.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolbarItem {
    pub id: String,
    pub icon: String,
    pub tooltip: String,
    pub plugin_id: String,
    pub order: i32,
}

impl ToolbarItem {
    pub fn new(id: impl Into<String>, icon: impl Into<String>, tooltip: impl Into<String>, plugin_id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            icon: icon.into(),
            tooltip: tooltip.into(),
            plugin_id: plugin_id.into(),
            order: 100,
        }
    }
}

/// Widget informativo dinâmico exibido na barra de status inferior.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatusBarItem {
    pub id: String,
    pub text: String,
    pub tooltip: Option<String>,
    pub plugin_id: String,
    pub alignment: StatusBarAlignment,
    pub order: i32,
}

impl StatusBarItem {
    pub fn new(id: impl Into<String>, text: impl Into<String>, plugin_id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            tooltip: None,
            plugin_id: plugin_id.into(),
            alignment: StatusBarAlignment::Right,
            order: 100,
        }
    }

    pub fn with_tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    pub fn with_alignment(mut self, alignment: StatusBarAlignment) -> Self {
        self.alignment = alignment;
        self
    }
}

/// Registro central thread-safe de extensões de interface gráfica.
#[derive(Debug, Clone, Default)]
pub struct UiRegistry {
    commands: Arc<RwLock<HashMap<String, UiCommand>>>,
    toolbar_items: Arc<RwLock<HashMap<String, ToolbarItem>>>,
    status_bar_items: Arc<RwLock<HashMap<String, StatusBarItem>>>,
}

impl UiRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registra um comando na paleta se o plugin tiver a permissão [`PluginPermission::UiCommands`].
    pub async fn register_command(&self, command: UiCommand, permissions: &[PluginPermission]) -> Result<()> {
        if !permissions.contains(&PluginPermission::UiCommands) {
            return Err(PluginError::PermissionDenied(
                PluginPermission::UiCommands.to_string(),
                command.plugin_id,
            ));
        }

        let mut map = self.commands.write().await;
        let _ = map.insert(command.id.clone(), command);
        Ok(())
    }

    /// Registra um botão na barra de ferramentas se o plugin tiver a permissão [`PluginPermission::UiWidgets`].
    pub async fn register_toolbar_item(&self, item: ToolbarItem, permissions: &[PluginPermission]) -> Result<()> {
        if !permissions.contains(&PluginPermission::UiWidgets) {
            return Err(PluginError::PermissionDenied(
                PluginPermission::UiWidgets.to_string(),
                item.plugin_id,
            ));
        }

        let mut map = self.toolbar_items.write().await;
        let _ = map.insert(item.id.clone(), item);
        Ok(())
    }

    /// Registra um widget na barra de status se o plugin tiver a permissão [`PluginPermission::UiWidgets`].
    pub async fn register_status_bar_item(&self, item: StatusBarItem, permissions: &[PluginPermission]) -> Result<()> {
        if !permissions.contains(&PluginPermission::UiWidgets) {
            return Err(PluginError::PermissionDenied(
                PluginPermission::UiWidgets.to_string(),
                item.plugin_id,
            ));
        }

        let mut map = self.status_bar_items.write().await;
        let _ = map.insert(item.id.clone(), item);
        Ok(())
    }

    /// Atualiza o texto de um widget da barra de status em tempo real.
    pub async fn update_status_bar_item(&self, id: &str, new_text: &str) -> bool {
        let mut map = self.status_bar_items.write().await;
        if let Some(item) = map.get_mut(id) {
            item.text = new_text.to_string();
            true
        } else {
            false
        }
    }

    /// Remove todos os elementos de UI registrados por um plugin (chamado no unload).
    pub async fn unregister_all_for_plugin(&self, plugin_id: &str) {
        {
            let mut cmds = self.commands.write().await;
            cmds.retain(|_, c| c.plugin_id != plugin_id);
        }
        {
            let mut toolbar = self.toolbar_items.write().await;
            toolbar.retain(|_, t| t.plugin_id != plugin_id);
        }
        {
            let mut status = self.status_bar_items.write().await;
            status.retain(|_, s| s.plugin_id != plugin_id);
        }
    }

    /// Retorna todos os comandos registrados ordenados alfabeticamente.
    pub async fn all_commands(&self) -> Vec<UiCommand> {
        let map = self.commands.read().await;
        let mut list: Vec<UiCommand> = map.values().cloned().collect();
        list.sort_by(|a, b| a.title.cmp(&b.title));
        list
    }

    /// Retorna todos os itens da barra de ferramentas ordenados.
    pub async fn all_toolbar_items(&self) -> Vec<ToolbarItem> {
        let map = self.toolbar_items.read().await;
        let mut list: Vec<ToolbarItem> = map.values().cloned().collect();
        list.sort_by_key(|i| i.order);
        list
    }

    /// Retorna todos os widgets da barra de status ordenados.
    pub async fn all_status_bar_items(&self) -> Vec<StatusBarItem> {
        let map = self.status_bar_items.read().await;
        let mut list: Vec<StatusBarItem> = map.values().cloned().collect();
        list.sort_by_key(|i| i.order);
        list
    }
}

/// Plugin de demonstração para validação da API de extensão da interface gráfica.
///
/// Registra comando na paleta (`Ctrl+O`), botão na barra de ferramentas e
/// widget de status dinâmico na barra inferior, atualizando a contagem de palavras
/// mediante eventos de abertura de notas.
#[derive(Debug, Clone)]
pub struct WordCounterDemoPlugin {
    manifest: PluginManifest,
    ui_registry: Option<Arc<UiRegistry>>,
    active_words: usize,
}

impl Default for WordCounterDemoPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl WordCounterDemoPlugin {
    pub fn new() -> Self {
        let manifest = PluginManifest::new(
            "word-counter-demo",
            "Contador de Palavras",
            "1.0.0",
            "Plugin demonstrativo que adiciona comandos, botão na toolbar e status bar.",
            "Gnosis Core Team",
            "main.js",
        )
        .with_permissions(vec![
            PluginPermission::UiCommands,
            PluginPermission::UiWidgets,
            PluginPermission::ReadNotes,
        ]);

        Self {
            manifest,
            ui_registry: None,
            active_words: 0,
        }
    }

    pub fn active_words(&self) -> usize {
        self.active_words
    }
}

#[async_trait]
impl crate::lifecycle::Plugin for WordCounterDemoPlugin {
    fn manifest(&self) -> &PluginManifest {
        &self.manifest
    }

    async fn on_load(&mut self, context: &crate::lifecycle::PluginContext) -> Result<()> {
        let ui = context.ui_registry.clone();
        self.ui_registry = Some(ui.clone());

        // 1. Registra comando na Paleta de Comandos (Quick Switcher)
        let cmd = UiCommand::new(
            "word-counter-demo:stats",
            "Exibir Estatísticas Detalhadas do Texto",
            &self.manifest.id,
        )
        .with_hotkey("Ctrl+Shift+C")
        .with_icon("📝");
        ui.register_command(cmd, &context.permissions).await?;

        // 2. Registra botão de ação na barra de ferramentas
        let toolbar = ToolbarItem::new(
            "word-counter-demo:toolbar-btn",
            "📊",
            "Estatísticas de Palavras",
            &self.manifest.id,
        );
        ui.register_toolbar_item(toolbar, &context.permissions).await?;

        // 3. Registra widget dinâmico na barra de status inferior
        let status = StatusBarItem::new(
            "word-counter-demo:status-widget",
            "0 palavras",
            &self.manifest.id,
        )
        .with_tooltip("Contagem de palavras da nota ativa")
        .with_alignment(StatusBarAlignment::Right);
        ui.register_status_bar_item(status, &context.permissions).await?;

        Ok(())
    }

    async fn on_unload(&mut self) -> Result<()> {
        if let Some(ui) = self.ui_registry.take() {
            ui.unregister_all_for_plugin(&self.manifest.id).await;
        }
        self.active_words = 0;
        Ok(())
    }

    async fn on_event(&mut self, event: &crate::events::PluginEvent) -> Result<()> {
        if let crate::events::PluginEvent::NoteOpened { title, .. } = event {
            // Contagem dinâmica baseada no título da nota para a demonstração
            let count = title.split_whitespace().count().max(42);
            self.active_words = count;

            if let Some(ui) = &self.ui_registry {
                let text = format!("{} palavras", count);
                let _ = ui.update_status_bar_item("word-counter-demo:status-widget", &text).await;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{EventBus, PluginEvent};
    use crate::manager::PluginManager;
    use gnosis_core::path::VaultPath;

    #[tokio::test]
    async fn test_ui_registry_permissions_and_lifecycle() -> Result<()> {
        let registry = UiRegistry::new();

        // 1. Tentar registrar comando sem permissão -> deve falhar
        let cmd = UiCommand::new("plugin-a:run", "Executar Ação", "plugin-a");
        let fail = registry.register_command(cmd.clone(), &[]).await;
        assert!(fail.is_err());
        assert_eq!(
            fail,
            Err(PluginError::PermissionDenied("ui_commands".to_string(), "plugin-a".to_string()))
        );

        // 2. Registrar comando com permissão UiCommands -> Sucesso
        registry.register_command(cmd, &[PluginPermission::UiCommands]).await?;
        assert_eq!(registry.all_commands().await.len(), 1);

        // 3. Registrar botão na toolbar e status bar com permissão UiWidgets
        let tb = ToolbarItem::new("plugin-a:btn", "⚡", "Ação Rápida", "plugin-a");
        let sb = StatusBarItem::new("plugin-a:words", "0 palavras", "plugin-a");

        registry.register_toolbar_item(tb, &[PluginPermission::UiWidgets]).await?;
        registry.register_status_bar_item(sb, &[PluginPermission::UiWidgets]).await?;

        assert_eq!(registry.all_toolbar_items().await.len(), 1);
        assert_eq!(registry.all_status_bar_items().await.len(), 1);

        // 4. Atualizar status bar dinamicamente
        let updated = registry.update_status_bar_item("plugin-a:words", "1.250 palavras").await;
        assert!(updated);
        assert_eq!(registry.all_status_bar_items().await[0].text, "1.250 palavras");

        // 5. Unload do plugin remove todos os seus elementos da interface
        registry.unregister_all_for_plugin("plugin-a").await;
        assert_eq!(registry.all_commands().await.len(), 0);
        assert_eq!(registry.all_toolbar_items().await.len(), 0);
        assert_eq!(registry.all_status_bar_items().await.len(), 0);

        Ok(())
    }

    #[tokio::test]
    async fn test_demo_word_counter_plugin_with_manager() -> Result<()> {
        let bus = Arc::new(EventBus::new(16));
        let manager = PluginManager::new(bus);

        let demo_plugin = Box::new(WordCounterDemoPlugin::new());
        manager.register_plugin(demo_plugin).await?;

        // Antes do load, a UI está vazia
        assert_eq!(manager.ui_registry().all_commands().await.len(), 0);
        assert_eq!(manager.ui_registry().all_toolbar_items().await.len(), 0);
        assert_eq!(manager.ui_registry().all_status_bar_items().await.len(), 0);

        // Ativa o plugin
        manager.load_plugin("word-counter-demo").await?;

        // 1. Verifica comando registrado na paleta
        let commands = manager.ui_registry().all_commands().await;
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].id, "word-counter-demo:stats");
        assert_eq!(commands[0].title, "Exibir Estatísticas Detalhadas do Texto");
        assert_eq!(commands[0].hotkey, Some("Ctrl+Shift+C".to_string()));
        assert_eq!(commands[0].icon, Some("📝".to_string()));

        // 2. Verifica botão registrado na barra de ferramentas
        let toolbar = manager.ui_registry().all_toolbar_items().await;
        assert_eq!(toolbar.len(), 1);
        assert_eq!(toolbar[0].id, "word-counter-demo:toolbar-btn");
        assert_eq!(toolbar[0].icon, "📊");

        // 3. Verifica widget registrado na barra de status
        let status_items = manager.ui_registry().all_status_bar_items().await;
        assert_eq!(status_items.len(), 1);
        assert_eq!(status_items[0].id, "word-counter-demo:status-widget");
        assert_eq!(status_items[0].text, "0 palavras");

        // 4. Despacha evento de abertura de nota -> status bar deve ser atualizado
        let path = VaultPath::parse("Notas/Ideia.md").expect("valid path");
        manager.dispatch_event(PluginEvent::NoteOpened {
            path,
            title: "Ideia Genial para o Gnosis".to_string(),
        }).await;

        let status_after = manager.ui_registry().all_status_bar_items().await;
        assert_eq!(status_after.len(), 1);
        assert_eq!(status_after[0].text, "42 palavras");

        // 5. Descarrega o plugin -> todos os elementos de interface são limpos
        manager.unload_plugin("word-counter-demo").await?;
        assert_eq!(manager.ui_registry().all_commands().await.len(), 0);
        assert_eq!(manager.ui_registry().all_toolbar_items().await.len(), 0);
        assert_eq!(manager.ui_registry().all_status_bar_items().await.len(), 0);

        Ok(())
    }
}
