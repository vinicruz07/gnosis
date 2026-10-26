//! Gerenciador global de múltiplos Vaults conhecidos e preferências da aplicação (`app.json`).
//!
//! Recursos:
//! - Registro persistente de Vaults locais (`known_vaults`), com identificador estável, nome e caminho.
//! - Rastreamento de último Vault ativo e ordenação inteligente (favoritos no topo, depois cronológico).
//! - Gravação atômica em disco para proteção contra arquivos corrompidos.
//! - Suporte completo ao modo Desktop (`%USERPROFILE%/.gnosis`) e Portable (`./config/app.json`).

use crate::error::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Nome padrão do arquivo de configuração global.
pub const GLOBAL_APP_CONFIG_FILE: &str = "app.json";

/// Metadados de um Vault conhecido pelo usuário na máquina.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VaultEntry {
    /// Identificador único gerado a partir do caminho.
    pub id: String,
    /// Nome amigável de exibição do Vault.
    pub name: String,
    /// Caminho absoluto da pasta do Vault no disco.
    pub path: PathBuf,
    /// Timestamp Unix do último acesso (segundos).
    pub last_opened_epoch_secs: u64,
    /// Indicador se o usuário marcou este Vault como favorito.
    pub is_favorite: bool,
}

/// Configuração global da aplicação Gnosis.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlobalAppConfig {
    /// Versão do schema de configuração.
    pub version: u32,
    /// Lista de todos os Vaults registrados na máquina.
    pub known_vaults: Vec<VaultEntry>,
    /// ID do último Vault ativo aberto pelo usuário.
    pub last_active_vault_id: Option<String>,
    /// Se deve reabrir automaticamente o último Vault ao iniciar.
    pub auto_open_last_vault: bool,
}

impl Default for GlobalAppConfig {
    fn default() -> Self {
        Self {
            version: 1,
            known_vaults: Vec::new(),
            last_active_vault_id: None,
            auto_open_last_vault: true,
        }
    }
}

impl GlobalAppConfig {
    /// Cria uma nova instância com valores padrão.
    pub fn new() -> Self {
        Self::default()
    }

    /// Adiciona ou atualiza um Vault no registro global.
    ///
    /// Se o Vault já existir (comparando caminhos canônicos), atualiza o timestamp de acesso
    /// e o nome (se fornecido). Retorna o identificador único do Vault.
    pub fn add_or_update_vault(&mut self, raw_path: PathBuf, custom_name: Option<String>) -> String {
        let canonical_path = raw_path.canonicalize().unwrap_or(raw_path.clone());
        let id = Self::generate_id(&canonical_path);
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        if let Some(entry) = self.known_vaults.iter_mut().find(|v| v.id == id || v.path == canonical_path) {
            entry.last_opened_epoch_secs = now;
            if let Some(name) = custom_name {
                entry.name = name;
            }
            self.last_active_vault_id = Some(id.clone());
            return id;
        }

        let name = custom_name.unwrap_or_else(|| {
            canonical_path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "Meu Vault".to_string())
        });

        self.known_vaults.push(VaultEntry {
            id: id.clone(),
            name,
            path: canonical_path,
            last_opened_epoch_secs: now,
            is_favorite: false,
        });

        self.last_active_vault_id = Some(id.clone());
        id
    }

    /// Remove um Vault do registro pelo identificador.
    pub fn remove_vault(&mut self, id: &str) -> bool {
        let initial_len = self.known_vaults.len();
        self.known_vaults.retain(|v| v.id != id);

        if self.last_active_vault_id.as_deref() == Some(id) {
            self.last_active_vault_id = self.known_vaults.first().map(|v| v.id.clone());
        }

        self.known_vaults.len() < initial_len
    }

    /// Alterna o estado de favorito de um Vault. Retorna `true` se encontrado.
    pub fn toggle_favorite(&mut self, id: &str) -> bool {
        if let Some(entry) = self.known_vaults.iter_mut().find(|v| v.id == id) {
            entry.is_favorite = !entry.is_favorite;
            true
        } else {
            false
        }
    }

    /// Retorna um Vault pelo identificador.
    pub fn get_vault(&self, id: &str) -> Option<&VaultEntry> {
        self.known_vaults.iter().find(|v| v.id == id)
    }

    /// Retorna o último Vault ativo, se houver.
    pub fn get_last_active_vault(&self) -> Option<&VaultEntry> {
        let id = self.last_active_vault_id.as_ref()?;
        self.get_vault(id)
    }

    /// Retorna a lista de Vaults ordenados: favoritos primeiro, depois mais recentes.
    pub fn list_sorted(&self) -> Vec<&VaultEntry> {
        let mut list: Vec<&VaultEntry> = self.known_vaults.iter().collect();
        list.sort_by(|a, b| {
            b.is_favorite
                .cmp(&a.is_favorite)
                .then_with(|| b.last_opened_epoch_secs.cmp(&a.last_opened_epoch_secs))
        });
        list
    }

    /// Salva atomicamente a configuração em `config_dir/app.json`.
    pub async fn save_to_dir(&self, config_dir: &Path) -> Result<()> {
        tokio::fs::create_dir_all(config_dir).await?;

        let final_path = config_dir.join(GLOBAL_APP_CONFIG_FILE);
        let tmp_path = config_dir.join(format!("{}.tmp", GLOBAL_APP_CONFIG_FILE));

        let json_bytes = serde_json::to_vec_pretty(self)?;

        let mut file = tokio::fs::File::create(&tmp_path).await?;
        tokio::io::AsyncWriteExt::write_all(&mut file, &json_bytes).await?;
        file.sync_all().await?;

        tokio::fs::rename(&tmp_path, &final_path).await?;
        Ok(())
    }

    /// Carrega a configuração global a partir de `config_dir/app.json`.
    ///
    /// Se o arquivo não existir, retorna `Ok(GlobalAppConfig::default())`.
    pub async fn load_from_dir(config_dir: &Path) -> Result<Self> {
        let path = config_dir.join(GLOBAL_APP_CONFIG_FILE);

        if !path.exists() {
            return Ok(Self::default());
        }

        let bytes = tokio::fs::read(&path).await?;
        let config: Self = serde_json::from_slice(&bytes)?;
        Ok(config)
    }

    fn generate_id(path: &Path) -> String {
        let s = path.to_string_lossy();
        let mut hash: u64 = 5381;
        for b in s.bytes() {
            hash = ((hash << 5).wrapping_add(hash)).wrapping_add(b as u64);
        }
        format!("vault_{:016x}", hash)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_global_app_config_lifecycle() -> Result<()> {
        let temp_dir = std::env::temp_dir().join(format!("gnosis_app_config_test_{}", std::process::id()));
        let _ = tokio::fs::create_dir_all(&temp_dir).await;

        let mut config = GlobalAppConfig::new();

        let vault_a = temp_dir.join("MeuCerebro");
        let vault_b = temp_dir.join("Trabalho");
        let _ = tokio::fs::create_dir_all(&vault_a).await;
        let _ = tokio::fs::create_dir_all(&vault_b).await;

        let id_a = config.add_or_update_vault(vault_a.clone(), Some("Meu Cérebro".to_string()));
        let id_b = config.add_or_update_vault(vault_b.clone(), Some("Notas de Trabalho".to_string()));

        assert_eq!(config.known_vaults.len(), 2);
        assert_eq!(config.last_active_vault_id, Some(id_b.clone()));

        // Favoritar A
        let fav = config.toggle_favorite(&id_a);
        assert!(fav);

        // Listagem ordenada: favorito deve vir primeiro
        let sorted = config.list_sorted();
        assert_eq!(sorted.first().map(|v| v.id.as_str()), Some(id_a.as_str()));

        // Salvar e recarregar
        config.save_to_dir(&temp_dir).await?;
        let loaded = GlobalAppConfig::load_from_dir(&temp_dir).await?;

        assert_eq!(loaded.known_vaults.len(), 2);
        assert_eq!(loaded.get_vault(&id_a).map(|v| v.is_favorite), Some(true));

        // Remover B
        let mut loaded_mut = loaded;
        let removed = loaded_mut.remove_vault(&id_b);
        assert!(removed);
        assert_eq!(loaded_mut.known_vaults.len(), 1);
        assert_eq!(loaded_mut.last_active_vault_id, Some(id_a));

        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
        Ok(())
    }
}
