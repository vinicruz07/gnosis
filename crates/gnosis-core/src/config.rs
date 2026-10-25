//! Gerenciador de persistência de configurações locais e estado do Vault (`.gnosis/config.json`).
//!
//! Recursos:
//! - Serialização e desserialização atômica de preferências e estado do workspace (última nota, abas abertas, histórico).
//! - Gerenciamento de abas abertas e histórico recente com desduplicação e limite de capacidade.
//! - Importação e compatibilidade transparente com arquivos de workspace do Obsidian (`.obsidian/workspace.json`).
//! - Métodos assíncronos para salvar e carregar via [`FileSystemProvider`].

use crate::error::{CoreError, Result};
use crate::path::VaultPath;
use crate::vfs::FileSystemProvider;
use serde::{Deserialize, Serialize};

/// Caminho padrão relativo do arquivo de configuração do Gnosis dentro do Vault.
pub const GNOSIS_CONFIG_PATH: &str = ".gnosis/config.json";

/// Caminho do arquivo de configuração de workspace do Obsidian para migração.
pub const OBSIDIAN_WORKSPACE_PATH: &str = ".obsidian/workspace.json";

/// Limite máximo de histórico de notas recentes mantidas em memória/disco.
const MAX_RECENT_NOTES: usize = 30;

fn default_theme() -> String {
    "gnosis-dark".to_string()
}

fn default_font_size() -> u16 {
    15
}

fn default_true() -> bool {
    true
}

/// Estado persistido do workspace e preferências do usuário para o Vault.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultConfig {
    /// Versão do schema de configuração.
    pub version: u32,

    /// Nome amigável do Vault (opcional).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub vault_name: Option<String>,

    /// Caminho da última nota ativa/aberta.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub last_opened_note: Option<VaultPath>,

    /// Lista de caminhos de notas abertas em abas.
    #[serde(default)]
    pub open_tabs: Vec<VaultPath>,

    /// Índice da aba ativa atualmente selecionada.
    #[serde(default)]
    pub active_tab_index: usize,

    /// Histórico de notas visitadas recentemente (ordem cronológica reversa).
    #[serde(default)]
    pub recent_notes: Vec<VaultPath>,

    /// Pasta padrão para armazenamento de anexos (ex: "attachments").
    pub attachments_folder: VaultPath,

    /// Pasta padrão para criação de novas notas (None = raiz do Vault).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub new_note_folder: Option<VaultPath>,

    /// Tema visual ativo (ex: "gnosis-dark", "gnosis-light", "dracula", "nord").
    #[serde(default = "default_theme")]
    pub active_theme: String,

    /// Tamanho da fonte do editor/leitor.
    #[serde(default = "default_font_size")]
    pub font_size: u16,

    /// Exibir numeração de linhas no editor.
    #[serde(default = "default_true")]
    pub line_numbers: bool,

    /// Limitar comprimento de linha para leitura confortável.
    #[serde(default = "default_true")]
    pub readable_line_length: bool,
}

impl Default for VaultConfig {
    fn default() -> Self {
        Self {
            version: 1,
            vault_name: None,
            last_opened_note: None,
            open_tabs: Vec::new(),
            active_tab_index: 0,
            recent_notes: Vec::new(),
            attachments_folder: VaultPath::default_attachments(),
            new_note_folder: None,
            active_theme: default_theme(),
            font_size: default_font_size(),
            line_numbers: default_true(),
            readable_line_length: default_true(),
        }
    }
}

impl VaultConfig {
    /// Cria uma nova instância padrão de configuração.
    pub fn new() -> Self {
        Self::default()
    }

    /// Serializa a configuração em JSON formatado com indentação.
    pub fn to_json_pretty(&self) -> Result<String> {
        serde_json::to_string_pretty(self).map_err(|e| CoreError::Serialization(e.to_string()))
    }

    /// Serializa a configuração em JSON compacto.
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string(self).map_err(|e| CoreError::Serialization(e.to_string()))
    }

    /// Desserializa a configuração a partir de uma string JSON.
    pub fn from_json(json_str: &str) -> Result<Self> {
        serde_json::from_str(json_str).map_err(|e| CoreError::Serialization(e.to_string()))
    }

    /// Registra uma nota como aberta: adiciona às abas se não existir, seleciona a aba e atualiza o histórico recente.
    pub fn record_opened_note(&mut self, path: VaultPath) {
        self.last_opened_note = Some(path.clone());

        // Atualiza abas abertas
        if let Some(pos) = self.open_tabs.iter().position(|p| p == &path) {
            self.active_tab_index = pos;
        } else {
            self.open_tabs.push(path.clone());
            self.active_tab_index = self.open_tabs.len().saturating_sub(1);
        }

        // Atualiza histórico recente (move para o topo, desduplica)
        self.recent_notes.retain(|p| p != &path);
        self.recent_notes.insert(0, path);

        if self.recent_notes.len() > MAX_RECENT_NOTES {
            self.recent_notes.truncate(MAX_RECENT_NOTES);
        }
    }

    /// Fecha uma aba aberta. Se a aba for a ativa, seleciona a aba adjacente mais próxima.
    pub fn close_tab(&mut self, path: &VaultPath) -> bool {
        let Some(pos) = self.open_tabs.iter().position(|p| p == path) else {
            return false;
        };

        self.open_tabs.remove(pos);

        if self.open_tabs.is_empty() {
            self.active_tab_index = 0;
            self.last_opened_note = None;
        } else {
            if self.active_tab_index >= self.open_tabs.len() {
                self.active_tab_index = self.open_tabs.len().saturating_sub(1);
            }
            if let Some(active_path) = self.open_tabs.get(self.active_tab_index) {
                self.last_opened_note = Some(active_path.clone());
            }
        }

        true
    }

    /// Altera a aba ativa pelo índice. Retorna `true` se o índice for válido.
    pub fn switch_tab(&mut self, index: usize) -> bool {
        if index < self.open_tabs.len() {
            self.active_tab_index = index;
            if let Some(active_path) = self.open_tabs.get(index) {
                self.last_opened_note = Some(active_path.clone());
            }
            true
        } else {
            false
        }
    }

    /// Salva a configuração atual no VFS em `.gnosis/config.json`.
    pub async fn save_to_vfs<F: FileSystemProvider>(&self, vfs: &F) -> Result<()> {
        let json_bytes = self.to_json_pretty()?.into_bytes();
        let path = VaultPath::parse(GNOSIS_CONFIG_PATH)?;
        vfs.write_file(&path, &json_bytes).await
    }

    /// Carrega a configuração do Vault a partir do VFS.
    ///
    /// Se `.gnosis/config.json` existir, faz o parse e retorna.
    /// Se não existir, tenta carregar `.obsidian/workspace.json` para importar abas e estado prévio.
    /// Caso nenhum dos dois exista, retorna a configuração padrão [`VaultConfig::default()`].
    pub async fn load_from_vfs<F: FileSystemProvider>(vfs: &F) -> Result<Self> {
        let gnosis_path = VaultPath::parse(GNOSIS_CONFIG_PATH)?;

        if vfs.exists(&gnosis_path).await {
            let data = vfs.read_file(&gnosis_path).await?;
            let json_str = String::from_utf8(data).map_err(|e| CoreError::Utf8Error(e.to_string()))?;
            return Self::from_json(&json_str);
        }

        let obsidian_path = match VaultPath::parse(OBSIDIAN_WORKSPACE_PATH) {
            Ok(p) => p,
            Err(_) => return Ok(Self::default()),
        };

        if vfs.exists(&obsidian_path).await {
            let data = vfs.read_file(&obsidian_path).await?;
            if let Ok(json_str) = String::from_utf8(data) {
                if let Ok(migrated) = Self::parse_obsidian_workspace(&json_str) {
                    return Ok(migrated);
                }
            }
        }

        Ok(Self::default())
    }

    /// Converte um arquivo de estado `.obsidian/workspace.json` em um [`VaultConfig`].
    pub fn parse_obsidian_workspace(json_str: &str) -> Result<Self> {
        let root: serde_json::Value =
            serde_json::from_str(json_str).map_err(|e| CoreError::Serialization(e.to_string()))?;

        let mut config = Self::default();
        let mut found_files = Vec::new();

        Self::extract_files_recursive(&root, &mut found_files);

        for file in found_files {
            if let Ok(vault_path) = VaultPath::parse(&file) {
                if !config.open_tabs.contains(&vault_path) {
                    config.open_tabs.push(vault_path);
                }
            }
        }

        if let Some(first) = config.open_tabs.first() {
            config.last_opened_note = Some(first.clone());
            config.active_tab_index = 0;
        }

        Ok(config)
    }

    fn extract_files_recursive(value: &serde_json::Value, out: &mut Vec<String>) {
        match value {
            serde_json::Value::Object(map) => {
                if let Some(file_val) = map.get("file") {
                    if let Some(file_str) = file_val.as_str() {
                        if !file_str.is_empty() {
                            out.push(file_str.to_string());
                        }
                    }
                }
                for (_, v) in map {
                    Self::extract_files_recursive(v, out);
                }
            }
            serde_json::Value::Array(arr) => {
                for item in arr {
                    Self::extract_files_recursive(item, out);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vfs::MemoryFileSystem;

    #[test]
    fn test_default_config_properties() {
        let config = VaultConfig::new();
        assert_eq!(config.version, 1);
        assert_eq!(config.attachments_folder.as_str(), "attachments");
        assert_eq!(config.active_theme, "gnosis-dark");
        assert_eq!(config.font_size, 15);
        assert!(config.line_numbers);
        assert!(config.readable_line_length);
        assert!(config.open_tabs.is_empty());
        assert!(config.last_opened_note.is_none());
    }

    #[test]
    fn test_serialization_and_deserialization() -> Result<()> {
        let mut config = VaultConfig::new();
        let note_a = VaultPath::parse("Notas/Ideia.md")?;
        let note_b = VaultPath::parse("Diario/Hoje.md")?;

        config.record_opened_note(note_a.clone());
        config.record_opened_note(note_b.clone());
        config.vault_name = Some("Meu Segundo Cerebro".to_string());
        config.active_theme = "dracula".to_string();

        let json = config.to_json_pretty()?;
        let restored = VaultConfig::from_json(&json)?;

        assert_eq!(restored.vault_name, Some("Meu Segundo Cerebro".to_string()));
        assert_eq!(restored.active_theme, "dracula");
        assert_eq!(restored.open_tabs.len(), 2);
        assert_eq!(restored.last_opened_note, Some(note_b));
        assert_eq!(restored.recent_notes.first(), Some(&VaultPath::parse("Diario/Hoje.md")?));

        Ok(())
    }

    #[test]
    fn test_tab_management_and_history() -> Result<()> {
        let mut config = VaultConfig::new();
        let note1 = VaultPath::parse("Nota1.md")?;
        let note2 = VaultPath::parse("Nota2.md")?;
        let note3 = VaultPath::parse("Nota3.md")?;

        config.record_opened_note(note1.clone());
        config.record_opened_note(note2.clone());
        config.record_opened_note(note3.clone());

        assert_eq!(config.open_tabs.len(), 3);
        assert_eq!(config.active_tab_index, 2);
        assert_eq!(config.last_opened_note, Some(note3.clone()));

        // Alterna para aba 0 (Nota1.md)
        let switched = config.switch_tab(0);
        assert!(switched);
        assert_eq!(config.active_tab_index, 0);
        assert_eq!(config.last_opened_note, Some(note1.clone()));

        // Fecha aba ativa (Nota1.md)
        let closed = config.close_tab(&note1);
        assert!(closed);
        assert_eq!(config.open_tabs.len(), 2);
        assert_eq!(config.active_tab_index, 0);
        assert_eq!(config.last_opened_note, Some(note2.clone()));

        Ok(())
    }

    #[tokio::test]
    async fn test_vfs_save_and_load() -> Result<()> {
        let vfs = MemoryFileSystem::new();
        let mut config = VaultConfig::new();
        let note = VaultPath::parse("Artigos/Rust.md")?;
        config.record_opened_note(note.clone());
        config.active_theme = "nord".to_string();

        // Salva no VFS
        config.save_to_vfs(&vfs).await?;

        // Carrega do VFS
        let loaded = VaultConfig::load_from_vfs(&vfs).await?;
        assert_eq!(loaded.active_theme, "nord");
        assert_eq!(loaded.last_opened_note, Some(note));
        assert_eq!(loaded.open_tabs.len(), 1);

        Ok(())
    }

    #[test]
    fn test_obsidian_workspace_migration() -> Result<()> {
        let obsidian_json = r#"{
            "main": {
                "id": "root-split",
                "type": "split",
                "children": [
                    {
                        "id": "tabs-container",
                        "type": "tabs",
                        "children": [
                            {
                                "id": "leaf-1",
                                "type": "leaf",
                                "state": {
                                    "type": "markdown",
                                    "state": {
                                        "file": "Projetos/Gnosis.md"
                                    }
                                }
                            },
                            {
                                "id": "leaf-2",
                                "type": "leaf",
                                "state": {
                                    "type": "markdown",
                                    "state": {
                                        "file": "Inbox/Ideias.md"
                                    }
                                }
                            }
                        ]
                    }
                ]
            }
        }"#;

        let config = VaultConfig::parse_obsidian_workspace(obsidian_json)?;
        assert_eq!(config.open_tabs.len(), 2);
        assert_eq!(config.open_tabs[0].as_str(), "Projetos/Gnosis.md");
        assert_eq!(config.open_tabs[1].as_str(), "Inbox/Ideias.md");
        assert_eq!(config.last_opened_note, Some(VaultPath::parse("Projetos/Gnosis.md")?));

        Ok(())
    }
}
