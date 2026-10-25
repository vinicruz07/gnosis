//! Implementação nativa de [`FileSystemProvider`] para disco local (Desktop & Portable).
//!
//! Recursos Críticos:
//! - Escrita atômica em 3 etapas (`.tmp` -> `sync_all` -> `rename`) para tolerância zero a corrupção.
//! - Confinamento de segurança garantindo que nenhuma operação escape da raiz do Vault.
//! - Suporte assíncrono sobre [`tokio::fs`].

use async_trait::async_trait;
use gnosis_core::{CoreError, FileMetadata, FileSystemProvider, Result as CoreResult, VaultPath};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;
use tokio::io::AsyncWriteExt;

/// Provedor VFS para o sistema de arquivos local do sistema operacional.
#[derive(Debug, Clone)]
pub struct NativeFileSystem {
    root: PathBuf,
}

impl NativeFileSystem {
    /// Inicializa o provedor nativo apontando para o diretório raiz do Vault.
    ///
    /// # Erros
    /// Retorna `CoreError::NotFound` se o caminho não existir ou não for um diretório.
    pub fn new<P: AsRef<Path>>(root: P) -> CoreResult<Self> {
        let path = root.as_ref().to_path_buf();
        if !path.exists() {
            return Err(CoreError::NotFound(format!(
                "Diretório raiz do Vault não existe: {}",
                path.display()
            )));
        }
        if !path.is_dir() {
            return Err(CoreError::NotDirectory(format!(
                "O caminho especificado para o Vault não é um diretório: {}",
                path.display()
            )));
        }

        let canonical_root = match path.canonicalize() {
            Ok(p) => p,
            Err(e) => return Err(CoreError::Io(format!("Falha ao resolver caminho canônico: {}", e))),
        };

        Ok(Self {
            root: canonical_root,
        })
    }

    /// Retorna o caminho absoluto do diretório raiz do Vault.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Mapeia um [`VaultPath`] seguro para um [`PathBuf`] físico no disco,
    /// garantindo que o caminho final resida estritamente dentro da raiz do Vault.
    fn resolve_path(&self, vault_path: &VaultPath) -> CoreResult<PathBuf> {
        let relative = vault_path.as_str();
        let target = self.root.join(relative);

        // Se o arquivo já existir, valida se seu caminho canônico reside sob a raiz
        if target.exists() {
            if let Ok(canonical_target) = target.canonicalize() {
                if !canonical_target.starts_with(&self.root) {
                    return Err(CoreError::PathTraversal(format!(
                        "Tentativa de fuga do Vault detectada para: {}",
                        vault_path.as_str()
                    )));
                }
            }
        }

        Ok(target)
    }
}

#[async_trait]
impl FileSystemProvider for NativeFileSystem {
    async fn read_file(&self, path: &VaultPath) -> CoreResult<Vec<u8>> {
        let full_path = self.resolve_path(path)?;

        match tokio::fs::read(&full_path).await {
            Ok(bytes) => Ok(bytes),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Err(CoreError::NotFound(path.to_string()))
            }
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                Err(CoreError::PermissionDenied(path.to_string()))
            }
            Err(e) => Err(CoreError::Io(format!(
                "Falha ao ler arquivo '{}': {}",
                path.as_str(),
                e
            ))),
        }
    }

    async fn write_file_atomic(&self, path: &VaultPath, data: &[u8]) -> CoreResult<()> {
        let full_path = self.resolve_path(path)?;

        // 1. Garante que os diretórios pais existam
        if let Some(parent) = full_path.parent() {
            if !parent.exists() {
                if let Err(e) = tokio::fs::create_dir_all(parent).await {
                    return Err(CoreError::Io(format!(
                        "Falha ao criar diretórios pais para '{}': {}",
                        path.as_str(),
                        e
                    )));
                }
            }
        }

        // 2. Cria o arquivo temporário no mesmo diretório (garante mesmo volume para rename atômico)
        let file_stem = path.file_stem().unwrap_or("tmp");
        let temp_filename = format!("{}.{}.tmp", file_stem, std::process::id());
        let temp_path = match full_path.parent() {
            Some(p) => p.join(temp_filename),
            None => self.root.join(temp_filename),
        };

        // 3. Grava os dados no arquivo temporário
        let mut file = match tokio::fs::File::create(&temp_path).await {
            Ok(f) => f,
            Err(e) => {
                return Err(CoreError::Io(format!(
                    "Falha ao criar arquivo temporário para escrita atômica: {}",
                    e
                )))
            }
        };

        if let Err(e) = file.write_all(data).await {
            let _ = tokio::fs::remove_file(&temp_path).await;
            return Err(CoreError::Io(format!(
                "Falha ao gravar dados temporários: {}",
                e
            )));
        }

        // 4. Força o flush físico dos buffers do SO para o disco rígido (sync_all)
        if let Err(e) = file.sync_all().await {
            let _ = tokio::fs::remove_file(&temp_path).await;
            return Err(CoreError::Io(format!(
                "Falha ao sincronizar dados com o disco: {}",
                e
            )));
        }

        // Fecha o arquivo explicitamente liberando o descritor (necessário no Windows antes de renomear)
        drop(file);

        // 5. Renomeação atômica do SO
        if let Err(e) = tokio::fs::rename(&temp_path, &full_path).await {
            let _ = tokio::fs::remove_file(&temp_path).await;
            return Err(CoreError::Io(format!(
                "Falha na renomeação atômica do arquivo: {}",
                e
            )));
        }

        Ok(())
    }

    async fn exists(&self, path: &VaultPath) -> CoreResult<bool> {
        let full_path = self.resolve_path(path)?;
        match tokio::fs::try_exists(&full_path).await {
            Ok(exists) => Ok(exists),
            Err(e) => Err(CoreError::Io(format!(
                "Falha ao checar existência de '{}': {}",
                path.as_str(),
                e
            ))),
        }
    }

    async fn remove_file(&self, path: &VaultPath) -> CoreResult<()> {
        let full_path = self.resolve_path(path)?;

        match tokio::fs::remove_file(&full_path).await {
            Ok(_) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Err(CoreError::NotFound(path.to_string()))
            }
            Err(e) => Err(CoreError::Io(format!(
                "Falha ao remover arquivo '{}': {}",
                path.as_str(),
                e
            ))),
        }
    }

    async fn create_dir_all(&self, path: &VaultPath) -> CoreResult<()> {
        let full_path = self.resolve_path(path)?;

        match tokio::fs::create_dir_all(&full_path).await {
            Ok(_) => Ok(()),
            Err(e) => Err(CoreError::Io(format!(
                "Falha ao criar diretório '{}': {}",
                path.as_str(),
                e
            ))),
        }
    }

    async fn list_dir(&self, dir_path: &VaultPath) -> CoreResult<Vec<VaultPath>> {
        let full_path = self.resolve_path(dir_path)?;

        let mut read_dir = match tokio::fs::read_dir(&full_path).await {
            Ok(rd) => rd,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(CoreError::NotFound(dir_path.to_string()));
            }
            Err(e) => {
                return Err(CoreError::Io(format!(
                    "Falha ao listar diretório '{}': {}",
                    dir_path.as_str(),
                    e
                )))
            }
        };

        let mut results = Vec::new();

        while let Ok(Some(entry)) = read_dir.next_entry().await {
            let entry_path = entry.path();
            if let Ok(relative) = entry_path.strip_prefix(&self.root) {
                let relative_str = relative.to_string_lossy();
                if let Ok(vault_path) = VaultPath::parse(relative_str.as_ref()) {
                    results.push(vault_path);
                }
            }
        }

        Ok(results)
    }

    async fn metadata(&self, path: &VaultPath) -> CoreResult<FileMetadata> {
        let full_path = self.resolve_path(path)?;

        match tokio::fs::metadata(&full_path).await {
            Ok(meta) => {
                let modified_timestamp_millis = meta
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                    .map(|d| d.as_millis() as u64);

                Ok(FileMetadata {
                    is_file: meta.is_file(),
                    is_dir: meta.is_dir(),
                    size_bytes: meta.len(),
                    modified_timestamp_millis,
                })
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Err(CoreError::NotFound(path.to_string()))
            }
            Err(e) => Err(CoreError::Io(format!(
                "Falha ao obter metadados de '{}': {}",
                path.as_str(),
                e
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_native_vfs_atomic_lifecycle() -> CoreResult<()> {
        let temp_dir = std::env::temp_dir().join(format!("gnosis_vfs_test_{}", std::process::id()));
        let _ = tokio::fs::create_dir_all(&temp_dir).await;

        let vfs = NativeFileSystem::new(&temp_dir)?;
        let path = VaultPath::parse("diario/nota_atomica.md")?;

        // 1. Inicialmente não existe
        assert!(!vfs.exists(&path).await?);

        // 2. Gravação atômica com criação de pastas
        let dados = b"# Teste de Escrita At\xc3\xb4mica";
        vfs.write_file_atomic(&path, dados).await?;
        assert!(vfs.exists(&path).await?);

        // 3. Leitura e integridade
        let lido = vfs.read_file(&path).await?;
        assert_eq!(lido, dados);

        // 4. Metadados
        let meta = vfs.metadata(&path).await?;
        assert!(meta.is_file);
        assert!(!meta.is_dir);
        assert_eq!(meta.size_bytes, dados.len() as u64);

        // 5. Listagem de diretório
        let dir = VaultPath::parse("diario")?;
        let itens = vfs.list_dir(&dir).await?;
        assert_eq!(itens.len(), 1);
        assert_eq!(itens[0].as_str(), "diario/nota_atomica.md");

        // 6. Remoção
        vfs.remove_file(&path).await?;
        assert!(!vfs.exists(&path).await?);

        // Limpeza final do diretório de teste
        let _ = tokio::fs::remove_dir_all(&temp_dir).await;

        Ok(())
    }
}
