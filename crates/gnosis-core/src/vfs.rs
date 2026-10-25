//! Virtual File System (VFS) Abstrato para o Gnosis.
//!
//! Esta trait desacopla o Core da implementação física do armazenamento, permitindo
//! que a mesma lógica gerencie Vaults no disco nativo (Desktop/Portable) ou no
//! navegador (Web via OPFS/WASM).

use crate::error::{CoreError, Result};
use crate::path::VaultPath;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Metadados básicos de um arquivo ou diretório no Vault.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileMetadata {
    pub is_file: bool,
    pub is_dir: bool,
    pub size_bytes: u64,
    /// Timestamp Unix em milissegundos da última modificação, se disponível.
    pub modified_timestamp_millis: Option<u64>,
}

/// Interface abstrata para operações de sistema de arquivos dentro do Vault.
///
/// Todas as operações utilizam caminhos seguros [`VaultPath`], garantindo
/// que nenhum provedor consiga acessar ou modificar dados fora da raiz do Vault.
#[async_trait]
pub trait FileSystemProvider: Send + Sync {
    /// Lê todo o conteúdo de um arquivo em bytes de forma assíncrona.
    async fn read_file(&self, path: &VaultPath) -> Result<Vec<u8>>;

    /// Escreve dados em um arquivo de forma atômica (evita corrupção de arquivos).
    async fn write_file_atomic(&self, path: &VaultPath, data: &[u8]) -> Result<()>;

    /// Verifica se um arquivo ou diretório existe no caminho especificado.
    async fn exists(&self, path: &VaultPath) -> Result<bool>;

    /// Remove um arquivo do Vault.
    async fn remove_file(&self, path: &VaultPath) -> Result<()>;

    /// Cria uma pasta e todos os seus diretórios pais intermediários.
    async fn create_dir_all(&self, path: &VaultPath) -> Result<()>;

    /// Lista os caminhos imediatos de arquivos e subdiretórios contidos na pasta especificada.
    async fn list_dir(&self, path: &VaultPath) -> Result<Vec<VaultPath>>;

    /// Retorna os metadados do recurso (tamanho, tipo, data de modificação).
    async fn metadata(&self, path: &VaultPath) -> Result<FileMetadata>;
}

/// Provedor VFS 100% em memória para testes unitários e ambientes voláteis sem disco.
#[derive(Default, Clone)]
pub struct MemoryFileSystem {
    files: std::sync::Arc<tokio::sync::RwLock<std::collections::HashMap<VaultPath, Vec<u8>>>>,
}

impl MemoryFileSystem {
    pub fn new() -> Self {
        Self {
            files: std::sync::Arc::new(tokio::sync::RwLock::new(std::collections::HashMap::new())),
        }
    }
}

#[async_trait]
impl FileSystemProvider for MemoryFileSystem {
    async fn read_file(&self, path: &VaultPath) -> Result<Vec<u8>> {
        let map = self.files.read().await;
        match map.get(path) {
            Some(data) => Ok(data.clone()),
            None => Err(CoreError::NotFound(path.to_string())),
        }
    }

    async fn write_file_atomic(&self, path: &VaultPath, data: &[u8]) -> Result<()> {
        let mut map = self.files.write().await;
        let _ = map.insert(path.clone(), data.to_vec());
        Ok(())
    }

    async fn exists(&self, path: &VaultPath) -> Result<bool> {
        let map = self.files.read().await;
        Ok(map.contains_key(path))
    }

    async fn remove_file(&self, path: &VaultPath) -> Result<()> {
        let mut map = self.files.write().await;
        match map.remove(path) {
            Some(_) => Ok(()),
            None => Err(CoreError::NotFound(path.to_string())),
        }
    }

    async fn create_dir_all(&self, _path: &VaultPath) -> Result<()> {
        // No provedor em memória, pastas são implícitas pelos caminhos dos arquivos
        Ok(())
    }

    async fn list_dir(&self, prefix: &VaultPath) -> Result<Vec<VaultPath>> {
        let map = self.files.read().await;
        let prefix_str = format!("{}/", prefix.as_str());

        let mut results = Vec::new();
        for path in map.keys() {
            if path.as_str().starts_with(&prefix_str) {
                results.push(path.clone());
            }
        }
        Ok(results)
    }

    async fn metadata(&self, path: &VaultPath) -> Result<FileMetadata> {
        let map = self.files.read().await;
        match map.get(path) {
            Some(data) => Ok(FileMetadata {
                is_file: true,
                is_dir: false,
                size_bytes: data.len() as u64,
                modified_timestamp_millis: None,
            }),
            None => Err(CoreError::NotFound(path.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_memory_vfs_lifecycle() -> std::result::Result<(), CoreError> {
        let vfs = MemoryFileSystem::new();
        let path = VaultPath::parse("diario/2026-10-25.md")?;

        assert!(!vfs.exists(&path).await?);

        vfs.write_file_atomic(&path, b"# Meu Dia em Rust").await?;
        assert!(vfs.exists(&path).await?);

        let content = vfs.read_file(&path).await?;
        assert_eq!(content, b"# Meu Dia em Rust");

        let meta = vfs.metadata(&path).await?;
        assert!(meta.is_file);
        assert_eq!(meta.size_bytes, 17);

        vfs.remove_file(&path).await?;
        assert!(!vfs.exists(&path).await?);

        Ok(())
    }
}
