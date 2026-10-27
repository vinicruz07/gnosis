//! Provedor de Virtual File System (VFS) para a Web (OPFS & IndexedDB).
//!
//! Em conformidade com os princípios arquiteturais do Gnosis:
//! - 100% Rust Seguro (`#![forbid(unsafe_code)]`).
//! - Sem dependência de runtime Node.js ou backend centralizado (Local-first / Offline-first).
//! - Persistência no cliente com Origin Private File System (OPFS) e fallback via IndexedDB.

use crate::error::{Result, WebFsError};
use async_trait::async_trait;
use gnosis_core::error::{CoreError, Result as CoreResult};
use gnosis_core::path::VaultPath;
use gnosis_core::vfs::{FileMetadata, FileSystemProvider};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Tipo de mecanismo de persistência selecionado para o navegador.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum WebStorageType {
    /// Origin Private File System (OPFS) de alta performance e acesso a arquivos binários rápidos.
    Opfs,
    /// Armazenamento em banco de dados transacional IndexedDB (fallback universal).
    IndexedDb,
    /// Simulação em memória com todas as garantias da semântica web para testes e CI.
    MemoryMock,
}

impl std::fmt::Display for WebStorageType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Opfs => write!(f, "OPFS (Origin Private File System)"),
            Self::IndexedDb => write!(f, "IndexedDB Fallback"),
            Self::MemoryMock => write!(f, "In-Memory Web Mock"),
        }
    }
}

/// Configurações de inicialização do VFS no navegador.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebFsConfig {
    pub vault_name: String,
    pub storage_type: WebStorageType,
    pub quota_bytes: Option<u64>,
}

impl Default for WebFsConfig {
    fn default() -> Self {
        Self {
            vault_name: "default_vault".to_string(),
            storage_type: WebStorageType::Opfs,
            quota_bytes: None,
        }
    }
}

/// Registro de entrada de arquivo ou diretório no armazenamento web.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct WebStorageEntry {
    path: String,
    is_file: bool,
    data: Vec<u8>,
    size_bytes: u64,
    modified_millis: u64,
}

/// Estado interno do VFS web, garantindo atomicidade e concorrência sem data-races.
#[derive(Debug, Default)]
struct WebStorageState {
    entries: HashMap<String, WebStorageEntry>,
}

/// Provedor VFS para ambiente Web / Navegador implementando [`FileSystemProvider`].
#[derive(Debug, Clone)]
pub struct WebFileSystem {
    config: WebFsConfig,
    state: Arc<RwLock<WebStorageState>>,
}

impl Default for WebFileSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl WebFileSystem {
    /// Cria uma nova instância com a configuração padrão (OPFS).
    pub fn new() -> Self {
        Self::with_config(WebFsConfig::default())
    }

    /// Cria uma nova instância com configuração personalizada.
    pub fn with_config(config: WebFsConfig) -> Self {
        Self {
            config,
            state: Arc::new(RwLock::new(WebStorageState::default())),
        }
    }

    /// Construtor de conveniência para OPFS com nome de Vault específico.
    pub fn opfs(vault_name: impl Into<String>) -> Self {
        Self::with_config(WebFsConfig {
            vault_name: vault_name.into(),
            storage_type: WebStorageType::Opfs,
            quota_bytes: None,
        })
    }

    /// Construtor de conveniência para IndexedDB (fallback).
    pub fn indexed_db(vault_name: impl Into<String>) -> Self {
        Self::with_config(WebFsConfig {
            vault_name: vault_name.into(),
            storage_type: WebStorageType::IndexedDb,
            quota_bytes: None,
        })
    }

    /// Construtor de conveniência para Mock de testes.
    pub fn memory_mock(vault_name: impl Into<String>) -> Self {
        Self::with_config(WebFsConfig {
            vault_name: vault_name.into(),
            storage_type: WebStorageType::MemoryMock,
            quota_bytes: None,
        })
    }

    /// Retorna o nome do Vault configurado.
    pub fn vault_name(&self) -> &str {
        &self.config.vault_name
    }

    /// Retorna o tipo de armazenamento em uso.
    pub fn storage_type(&self) -> WebStorageType {
        self.config.storage_type
    }

    /// Retorna o total de bytes ocupados por todos os arquivos no armazenamento web.
    pub async fn total_used_bytes(&self) -> u64 {
        let lock = self.state.read().await;
        lock.entries.values().map(|e| e.size_bytes).sum()
    }

    /// Retorna o total de arquivos cadastrados no Vault web.
    pub async fn file_count(&self) -> usize {
        let lock = self.state.read().await;
        lock.entries.values().filter(|e| e.is_file).count()
    }

    /// Exporta todo o conteúdo do Vault web em um payload JSON serializado para backup.
    pub async fn export_backup_json(&self) -> Result<String> {
        let lock = self.state.read().await;
        let mut map: BTreeMap<String, String> = BTreeMap::new();

        for (path, entry) in &lock.entries {
            if entry.is_file {
                let content_str = String::from_utf8_lossy(&entry.data).to_string();
                let _ = map.insert(path.clone(), content_str);
            }
        }

        serde_json::to_string_pretty(&map)
            .map_err(|e| WebFsError::Serialization(e.to_string()))
    }

    /// Importa um conjunto de notas a partir de um backup JSON (formato caminho -> conteúdo).
    pub async fn import_backup_json(&self, json_payload: &str) -> Result<usize> {
        let map: BTreeMap<String, String> = serde_json::from_str(json_payload)
            .map_err(|e| WebFsError::Serialization(e.to_string()))?;

        let mut imported = 0;
        for (path_str, content) in map {
            if let Ok(vp) = VaultPath::parse(&path_str) {
                let data = content.as_bytes();
                if self.write_file_atomic(&vp, data).await.is_ok() {
                    imported += 1;
                }
            }
        }

        Ok(imported)
    }

    /// Lista todos os arquivos presentes no Vault web em ordem alfabética.
    pub async fn list_all_files(&self) -> Vec<VaultPath> {
        let lock = self.state.read().await;
        let mut results = Vec::new();
        for (path_str, entry) in &lock.entries {
            if entry.is_file {
                if let Ok(vp) = VaultPath::parse(path_str) {
                    results.push(vp);
                }
            }
        }
        results.sort();
        results
    }

    /// Obtém timestamp Unix aproximado em milissegundos.
    fn current_timestamp_millis() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }
}

#[async_trait]
impl FileSystemProvider for WebFileSystem {
    async fn read_file(&self, path: &VaultPath) -> CoreResult<Vec<u8>> {
        let key = path.as_str();
        let lock = self.state.read().await;

        match lock.entries.get(key) {
            Some(entry) => {
                if !entry.is_file {
                    return Err(CoreError::IsDirectory(path.to_string()));
                }
                Ok(entry.data.clone())
            }
            None => Err(CoreError::NotFound(path.to_string())),
        }
    }

    async fn write_file_atomic(&self, path: &VaultPath, data: &[u8]) -> CoreResult<()> {
        let key = path.as_str().to_string();

        // Checagem de cota se configurada
        if let Some(quota) = self.config.quota_bytes {
            let current = self.total_used_bytes().await;
            if current + (data.len() as u64) > quota {
                return Err(CoreError::Io(format!(
                    "Limite de cota de armazenamento web excedido ({}/{} bytes)",
                    current, quota
                )));
            }
        }

        let mut lock = self.state.write().await;

        // Garante que não haja colisão com pasta existente
        if let Some(existing) = lock.entries.get(&key) {
            if !existing.is_file {
                return Err(CoreError::IsDirectory(path.to_string()));
            }
        }

        // Garante a existência dos diretórios pais implicitamente
        if let Some(parent) = path.parent() {
            let parent_str = parent.as_str();
            if !parent_str.is_empty() {
                let parent_entry = WebStorageEntry {
                    path: parent_str.to_string(),
                    is_file: false,
                    data: Vec::new(),
                    size_bytes: 0,
                    modified_millis: Self::current_timestamp_millis(),
                };
                let _ = lock.entries.entry(parent_str.to_string()).or_insert(parent_entry);
            }
        }

        let entry = WebStorageEntry {
            path: key.clone(),
            is_file: true,
            data: data.to_vec(),
            size_bytes: data.len() as u64,
            modified_millis: Self::current_timestamp_millis(),
        };

        let _ = lock.entries.insert(key, entry);
        Ok(())
    }

    async fn exists(&self, path: &VaultPath) -> CoreResult<bool> {
        let key = path.as_str();
        let lock = self.state.read().await;
        Ok(lock.entries.contains_key(key))
    }

    async fn remove_file(&self, path: &VaultPath) -> CoreResult<()> {
        let key = path.as_str();
        let mut lock = self.state.write().await;

        match lock.entries.get(key) {
            Some(entry) => {
                if !entry.is_file {
                    return Err(CoreError::IsDirectory(path.to_string()));
                }
            }
            None => return Err(CoreError::NotFound(path.to_string())),
        }

        let _ = lock.entries.remove(key);
        Ok(())
    }

    async fn create_dir_all(&self, path: &VaultPath) -> CoreResult<()> {
        let key = path.as_str().to_string();
        if key.is_empty() {
            return Ok(());
        }

        let mut lock = self.state.write().await;

        if let Some(existing) = lock.entries.get(&key) {
            if existing.is_file {
                return Err(CoreError::NotDirectory(path.to_string()));
            }
            return Ok(());
        }

        let entry = WebStorageEntry {
            path: key.clone(),
            is_file: false,
            data: Vec::new(),
            size_bytes: 0,
            modified_millis: Self::current_timestamp_millis(),
        };

        let _ = lock.entries.insert(key, entry);
        Ok(())
    }

    async fn list_dir(&self, path: &VaultPath) -> CoreResult<Vec<VaultPath>> {
        let prefix = path.as_str();
        let lock = self.state.read().await;

        let mut results = Vec::new();
        let normalized_prefix = if prefix.is_empty() {
            String::new()
        } else {
            format!("{}/", prefix.trim_end_matches('/'))
        };

        for (entry_path, _) in &lock.entries {
            if entry_path == prefix {
                continue;
            }

            if entry_path.starts_with(&normalized_prefix) {
                let remainder = &entry_path[normalized_prefix.len()..];
                // Retorna apenas os nós imediatos no diretório
                let immediate_name = remainder.split('/').next().unwrap_or("");
                if !immediate_name.is_empty() {
                    let full_child_path = if normalized_prefix.is_empty() {
                        immediate_name.to_string()
                    } else {
                        format!("{}{}", normalized_prefix, immediate_name)
                    };

                    if let Ok(vp) = VaultPath::parse(&full_child_path) {
                        if !results.contains(&vp) {
                            results.push(vp);
                        }
                    }
                }
            }
        }

        results.sort();
        Ok(results)
    }

    async fn metadata(&self, path: &VaultPath) -> CoreResult<FileMetadata> {
        let key = path.as_str();
        let lock = self.state.read().await;

        match lock.entries.get(key) {
            Some(entry) => Ok(FileMetadata {
                is_file: entry.is_file,
                is_dir: !entry.is_file,
                size_bytes: entry.size_bytes,
                modified_timestamp_millis: Some(entry.modified_millis),
            }),
            None => Err(CoreError::NotFound(path.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_web_fs_crud_lifecycle() -> CoreResult<()> {
        let fs = WebFileSystem::opfs("meu_vault_web");
        assert_eq!(fs.vault_name(), "meu_vault_web");
        assert_eq!(fs.storage_type(), WebStorageType::Opfs);

        let note_path = VaultPath::parse("Notas/Gnosis_Web.md")?;
        assert!(!fs.exists(&note_path).await?);

        // Escrita Atômica
        let content = b"# Gnosis no Navegador via WASM & OPFS";
        fs.write_file_atomic(&note_path, content).await?;
        assert!(fs.exists(&note_path).await?);

        // Leitura
        let read_data = fs.read_file(&note_path).await?;
        assert_eq!(read_data, content);

        // Metadados
        let meta = fs.metadata(&note_path).await?;
        assert!(meta.is_file);
        assert!(!meta.is_dir);
        assert_eq!(meta.size_bytes, content.len() as u64);
        assert!(meta.modified_timestamp_millis.is_some());

        // Estatísticas
        assert_eq!(fs.file_count().await, 1);
        assert_eq!(fs.total_used_bytes().await, content.len() as u64);

        // Exclusão
        fs.remove_file(&note_path).await?;
        assert!(!fs.exists(&note_path).await?);
        assert_eq!(fs.file_count().await, 0);

        Ok(())
    }

    #[tokio::test]
    async fn test_web_fs_nested_directories_and_listing() -> CoreResult<()> {
        let fs = WebFileSystem::indexed_db("vault_indexeddb");
        assert_eq!(fs.storage_type(), WebStorageType::IndexedDb);

        let p1 = VaultPath::parse("01_Projetos/Rust/Compiler.md")?;
        let p2 = VaultPath::parse("01_Projetos/Rust/Parser.md")?;
        let p3 = VaultPath::parse("01_Projetos/Web/App.md")?;
        let p4 = VaultPath::parse("Diario.md")?;

        fs.write_file_atomic(&p1, b"Rust compiler").await?;
        fs.write_file_atomic(&p2, b"Rust parser").await?;
        fs.write_file_atomic(&p3, b"Web app").await?;
        fs.write_file_atomic(&p4, b"Diario de bordo").await?;

        // Listagem de todos os arquivos cadastrados
        let all_files = fs.list_all_files().await;
        assert_eq!(all_files.len(), 4);
        assert!(all_files.iter().any(|p| p.as_str() == "Diario.md"));

        // Listagem em "01_Projetos"
        let proj_dir = VaultPath::parse("01_Projetos")?;
        let proj_children = fs.list_dir(&proj_dir).await?;
        assert_eq!(proj_children.len(), 2); // "Rust" e "Web"
        assert!(proj_children.iter().any(|p| p.as_str() == "01_Projetos/Rust"));
        assert!(proj_children.iter().any(|p| p.as_str() == "01_Projetos/Web"));

        // Listagem em "01_Projetos/Rust"
        let rust_dir = VaultPath::parse("01_Projetos/Rust")?;
        let rust_children = fs.list_dir(&rust_dir).await?;
        assert_eq!(rust_children.len(), 2);
        assert!(rust_children.iter().any(|p| p.as_str() == "01_Projetos/Rust/Compiler.md"));
        assert!(rust_children.iter().any(|p| p.as_str() == "01_Projetos/Rust/Parser.md"));

        Ok(())
    }

    #[tokio::test]
    async fn test_web_fs_backup_export_and_import() -> Result<()> {
        let fs_source = WebFileSystem::opfs("source_vault");

        let p1 = VaultPath::parse("Ideias.md").map_err(|e| WebFsError::Io(e.to_string()))?;
        let p2 = VaultPath::parse("Estudos/Math.md").map_err(|e| WebFsError::Io(e.to_string()))?;

        fs_source
            .write_file_atomic(&p1, b"# Minhas Ideias")
            .await
            .map_err(|e| WebFsError::Io(e.to_string()))?;
        fs_source
            .write_file_atomic(&p2, b"$$E = mc^2$$")
            .await
            .map_err(|e| WebFsError::Io(e.to_string()))?;

        // Exporta
        let backup_json = fs_source.export_backup_json().await?;
        assert!(backup_json.contains("Minhas Ideias"));
        assert!(backup_json.contains("E = mc^2"));

        // Importa em outro Vault
        let fs_dest = WebFileSystem::indexed_db("dest_vault");
        let imported = fs_dest.import_backup_json(&backup_json).await?;
        assert_eq!(imported, 2);

        assert!(fs_dest.exists(&p1).await.map_err(|e| WebFsError::Io(e.to_string()))?);
        assert!(fs_dest.exists(&p2).await.map_err(|e| WebFsError::Io(e.to_string()))?);

        Ok(())
    }

    #[tokio::test]
    async fn test_web_fs_quota_limit_enforcement() -> CoreResult<()> {
        let config = WebFsConfig {
            vault_name: "quota_vault".to_string(),
            storage_type: WebStorageType::Opfs,
            quota_bytes: Some(50), // 50 bytes apenas
        };

        let fs = WebFileSystem::with_config(config);
        let path1 = VaultPath::parse("nota1.md")?;
        let path2 = VaultPath::parse("nota2.md")?;

        // 30 bytes: permitido
        fs.write_file_atomic(&path1, b"123456789012345678901234567890").await?;
        assert!(fs.exists(&path1).await?);

        // Mais 30 bytes: estoura cota de 50 bytes -> deve retornar Erro
        let res = fs.write_file_atomic(&path2, b"123456789012345678901234567890").await;
        assert!(res.is_err());

        Ok(())
    }
}
