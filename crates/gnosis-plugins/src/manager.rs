//! Gerenciador central de plugins (`PluginManager`).
//!
//! Coordena a descoberta automática de manifestos na pasta `.gnosis/plugins/`,
//! o registro em tempo de execução, controle de estado (`PluginState`) e distribuição
//! de eventos através do [`EventBus`].

use crate::error::{PluginError, Result};
use crate::events::{EventBus, PluginEvent};
use crate::lifecycle::{Plugin, PluginContext, PluginState};
use crate::manifest::PluginManifest;
use gnosis_core::path::VaultPath;
use gnosis_core::vfs::FileSystemProvider;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

/// Gerenciador thread-safe de plugins do Gnosis.
#[derive(Clone)]
pub struct PluginManager {
    event_bus: Arc<EventBus>,
    manifests: Arc<RwLock<HashMap<String, PluginManifest>>>,
    instances: Arc<RwLock<HashMap<String, Arc<Mutex<Box<dyn Plugin>>>>>>,
    states: Arc<RwLock<HashMap<String, PluginState>>>,
}

impl PluginManager {
    /// Inicializa um novo gerenciador vinculado ao barramento de eventos.
    pub fn new(event_bus: Arc<EventBus>) -> Self {
        Self {
            event_bus,
            manifests: Arc::new(RwLock::new(HashMap::new())),
            instances: Arc::new(RwLock::new(HashMap::new())),
            states: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Retorna uma referência ao barramento de eventos compartilhado.
    pub fn event_bus(&self) -> &Arc<EventBus> {
        &self.event_bus
    }

    /// Registra uma nova instância de plugin no gerenciador em estado `Unloaded`.
    pub async fn register_plugin(&self, plugin: Box<dyn Plugin>) -> Result<()> {
        let manifest = plugin.manifest().clone();
        let plugin_id = manifest.id.clone();

        let mut manifests = self.manifests.write().await;
        if manifests.contains_key(&plugin_id) {
            return Err(PluginError::DuplicatePlugin(plugin_id));
        }

        let mut instances = self.instances.write().await;
        let mut states = self.states.write().await;

        let _ = manifests.insert(plugin_id.clone(), manifest);
        let _ = instances.insert(plugin_id.clone(), Arc::new(Mutex::new(plugin)));
        let _ = states.insert(plugin_id, PluginState::Unloaded);

        Ok(())
    }

    /// Ativa e carrega um plugin registrado invocando seu método `on_load`.
    pub async fn load_plugin(&self, plugin_id: &str) -> Result<()> {
        let manifest = {
            let manifests = self.manifests.read().await;
            manifests.get(plugin_id).cloned().ok_or_else(|| PluginError::PluginNotFound(plugin_id.to_string()))?
        };

        {
            let states = self.states.read().await;
            if let Some(state) = states.get(plugin_id) {
                if *state == PluginState::Active || *state == PluginState::Loaded {
                    return Err(PluginError::AlreadyLoaded(plugin_id.to_string()));
                }
            }
        }

        let instance_arc = {
            let instances = self.instances.read().await;
            instances.get(plugin_id).cloned().ok_or_else(|| PluginError::PluginNotFound(plugin_id.to_string()))?
        };

        let context = PluginContext::new(plugin_id, self.event_bus.clone(), manifest.permissions.clone());

        let mut instance = instance_arc.lock().await;
        match instance.on_load(&context).await {
            Ok(()) => {
                let mut states = self.states.write().await;
                let _ = states.insert(plugin_id.to_string(), PluginState::Active);
                Ok(())
            }
            Err(e) => {
                let mut states = self.states.write().await;
                let _ = states.insert(plugin_id.to_string(), PluginState::Error(e.to_string()));
                Err(PluginError::LifecycleError(plugin_id.to_string(), e.to_string()))
            }
        }
    }

    /// Desativa um plugin ativo invocando seu método `on_unload`.
    pub async fn unload_plugin(&self, plugin_id: &str) -> Result<()> {
        let instance_arc = {
            let instances = self.instances.read().await;
            instances.get(plugin_id).cloned().ok_or_else(|| PluginError::PluginNotFound(plugin_id.to_string()))?
        };

        {
            let states = self.states.read().await;
            if let Some(state) = states.get(plugin_id) {
                if *state == PluginState::Unloaded {
                    return Err(PluginError::NotLoaded(plugin_id.to_string()));
                }
            }
        }

        let mut instance = instance_arc.lock().await;
        match instance.on_unload().await {
            Ok(()) => {
                let mut states = self.states.write().await;
                let _ = states.insert(plugin_id.to_string(), PluginState::Unloaded);
                self.event_bus.unsubscribe_plugin(plugin_id).await;
                Ok(())
            }
            Err(e) => {
                let mut states = self.states.write().await;
                let _ = states.insert(plugin_id.to_string(), PluginState::Error(e.to_string()));
                Err(PluginError::LifecycleError(plugin_id.to_string(), e.to_string()))
            }
        }
    }

    /// Retorna o estado atual de um plugin cadastrado.
    pub async fn get_plugin_state(&self, plugin_id: &str) -> Option<PluginState> {
        let states = self.states.read().await;
        states.get(plugin_id).cloned()
    }

    /// Retorna todos os manifestos de plugins cadastrados.
    pub async fn list_manifests(&self) -> Vec<PluginManifest> {
        let manifests = self.manifests.read().await;
        let mut list: Vec<PluginManifest> = manifests.values().cloned().collect();
        list.sort_by(|a, b| a.id.cmp(&b.id));
        list
    }

    /// Despacha um evento para o barramento assíncrono e para todas as instâncias de plugins ativas.
    pub async fn dispatch_event(&self, event: PluginEvent) -> usize {
        // Publica no barramento para ouvintes assíncronos externos
        let delivered = self.event_bus.publish(event.clone());

        // Despacha diretamente para as instâncias de plugins ativas
        let instances = {
            let inst_guard = self.instances.read().await;
            let states_guard = self.states.read().await;

            let mut active_list = Vec::new();
            for (id, inst) in inst_guard.iter() {
                if let Some(state) = states_guard.get(id) {
                    if *state == PluginState::Active {
                        active_list.push(inst.clone());
                    }
                }
            }
            active_list
        };

        for inst_arc in instances {
            let mut inst = inst_arc.lock().await;
            let _ = inst.on_event(&event).await;
        }

        delivered
    }

    /// Descoberta automática de plugins através do sistema de arquivos virtual [`FileSystemProvider`].
    ///
    /// Varre a pasta indicada (normalmente `.gnosis/plugins/`) em busca de subpastas
    /// contendo um arquivo `plugin.json` válido, registrando os manifestos encontrados.
    pub async fn discover_plugins_in_vfs<P: FileSystemProvider>(
        &self,
        fs: &P,
        plugins_dir: &VaultPath,
    ) -> Result<Vec<PluginManifest>> {
        let mut discovered = Vec::new();
        let entries = match fs.list_dir(plugins_dir).await {
            Ok(list) => list,
            Err(_) => return Ok(discovered),
        };

        for entry_path in entries {
            let entry_str = entry_path.as_str();

            // Caso 1: O provedor retornou diretamente o arquivo plugin.json (ex: MemoryFileSystem)
            if entry_str.ends_with("/plugin.json") {
                if let Ok(bytes) = fs.read_file(&entry_path).await {
                    let json_str = String::from_utf8_lossy(&bytes);
                    if let Ok(manifest) = PluginManifest::from_json_str(&json_str) {
                        let plugin_id = manifest.id.clone();
                        let mut manifests = self.manifests.write().await;
                        let mut states = self.states.write().await;

                        let _ = manifests.insert(plugin_id.clone(), manifest.clone());
                        states.entry(plugin_id).or_insert(PluginState::Unloaded);
                        discovered.push(manifest);
                    }
                }
                continue;
            }

            // Caso 2: O provedor retornou uma subpasta de plugin (ex: NativeFileSystem / WebFileSystem)
            let manifest_path_str = format!("{}/plugin.json", entry_str.trim_end_matches('/'));
            if let Ok(manifest_path) = VaultPath::parse(&manifest_path_str) {
                if let Ok(true) = fs.exists(&manifest_path).await {
                    if let Ok(bytes) = fs.read_file(&manifest_path).await {
                        let json_str = String::from_utf8_lossy(&bytes);
                        if let Ok(manifest) = PluginManifest::from_json_str(&json_str) {
                            let plugin_id = manifest.id.clone();
                            let mut manifests = self.manifests.write().await;
                            let mut states = self.states.write().await;

                            let _ = manifests.insert(plugin_id.clone(), manifest.clone());
                            states.entry(plugin_id).or_insert(PluginState::Unloaded);
                            discovered.push(manifest);
                        }
                    }
                }
            }
        }

        discovered.sort_by(|a, b| a.id.cmp(&b.id));
        discovered.dedup_by(|a, b| a.id == b.id);
        Ok(discovered)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lifecycle::MockPlugin;
    use gnosis_core::vfs::MemoryFileSystem;

    #[tokio::test]
    async fn test_plugin_manager_lifecycle_and_dispatch() -> Result<()> {
        let bus = Arc::new(EventBus::new(16));
        let manager = PluginManager::new(bus);

        let plugin = Box::new(MockPlugin::new("zen-writer", "Zen Writer"));
        manager.register_plugin(plugin).await?;

        assert_eq!(manager.get_plugin_state("zen-writer").await, Some(PluginState::Unloaded));

        // Ativa o plugin
        manager.load_plugin("zen-writer").await?;
        assert_eq!(manager.get_plugin_state("zen-writer").await, Some(PluginState::Active));

        // Despacha evento de abertura de nota
        let note_path = VaultPath::parse("Livro/Capitulo1.md").expect("valid path");
        manager.dispatch_event(PluginEvent::NoteOpened {
            path: note_path.clone(),
            title: "Capítulo 1".to_string(),
        }).await;

        // Desativa o plugin
        manager.unload_plugin("zen-writer").await?;
        assert_eq!(manager.get_plugin_state("zen-writer").await, Some(PluginState::Unloaded));

        Ok(())
    }

    #[tokio::test]
    async fn test_discover_plugins_in_memory_vfs() -> Result<()> {
        let fs = MemoryFileSystem::new();
        let bus = Arc::new(EventBus::new(16));
        let manager = PluginManager::new(bus);

        // Simula criação da pasta .gnosis/plugins/word-count/plugin.json
        let plugins_root = VaultPath::parse(".gnosis/plugins").expect("valid path");
        let plugin_folder = VaultPath::parse(".gnosis/plugins/word-count").expect("valid path");
        let manifest_path = VaultPath::parse(".gnosis/plugins/word-count/plugin.json").expect("valid path");

        fs.create_dir_all(&plugins_root).await?;
        fs.create_dir_all(&plugin_folder).await?;

        let manifest_json = r#"{
            "id": "word-count",
            "name": "Contador de Palavras",
            "version": "1.0.0",
            "description": "Exibe contagem em tempo real na barra de status",
            "author": "Gnosis Community",
            "entrypoint": "main.js",
            "permissions": ["ui_widgets", "read_notes"]
        }"#;

        fs.write_file_atomic(&manifest_path, manifest_json.as_bytes()).await?;

        let discovered = manager.discover_plugins_in_vfs(&fs, &plugins_root).await?;
        assert_eq!(discovered.len(), 1);
        assert_eq!(discovered[0].id, "word-count");
        assert_eq!(discovered[0].name, "Contador de Palavras");

        let manifests = manager.list_manifests().await;
        assert_eq!(manifests.len(), 1);
        assert_eq!(manifests[0].id, "word-count");

        Ok(())
    }
}
