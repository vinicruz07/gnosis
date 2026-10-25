//! Mecanismo de exclusão segura de arquivos e notas do Vault (`VaultTrash`).
//!
//! Em vez de remover arquivos definitivamente com perda irreversível, o Gnosis
//! move itens deletados para a pasta protegida `.trash/` do Vault, mantendo um manifesto
//! atômico de metadados para permitir auditoria, listagem e restauração instantânea.

use crate::error::{NativeFsError, Result};
use gnosis_core::VaultPath;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

/// Nome padrão da pasta de lixeira interna do Vault.
pub const TRASH_FOLDER_NAME: &str = ".trash";

/// Nome do arquivo de manifesto de itens na lixeira.
pub const TRASH_MANIFEST_FILE: &str = "manifest.json";

/// Registro de um item movido para a lixeira do Vault.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrashedItem {
    /// Identificador único da exclusão.
    pub id: String,
    /// Caminho original seguro no momento da exclusão.
    pub original_path: VaultPath,
    /// Nome do arquivo físico armazenado dentro de `.trash/`.
    pub trashed_filename: String,
    /// Timestamp Unix do momento da exclusão.
    pub trashed_epoch_secs: u64,
    /// Tamanho do arquivo em bytes.
    pub size_bytes: u64,
}

/// Manifesto que indexa todos os arquivos atualmente armazenados em `.trash/`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TrashManifest {
    pub items: Vec<TrashedItem>,
}

/// Gerenciador de lixeira segura do Vault.
pub struct VaultTrash;

impl VaultTrash {
    /// Retorna o caminho absoluto do diretório `.trash` do Vault.
    pub fn trash_dir(vault_root: &Path) -> PathBuf {
        vault_root.join(TRASH_FOLDER_NAME)
    }

    /// Move um arquivo do Vault com segurança para a lixeira `.trash/`.
    ///
    /// Preserva metadados e garante que nomes duplicados não se sobreponham através de IDs únicos.
    pub async fn move_to_trash(vault_root: &Path, path: &VaultPath) -> Result<TrashedItem> {
        let source_full = vault_root.join(path.as_str());

        if !source_full.exists() {
            return Err(NativeFsError::NotFound(source_full));
        }

        let metadata = tokio::fs::metadata(&source_full).await?;
        if metadata.is_dir() {
            return Err(NativeFsError::Trash("Exclusão de diretórios inteiros via lixeira ainda não é suportada".to_string()));
        }

        let trash_dir = Self::trash_dir(vault_root);
        tokio::fs::create_dir_all(&trash_dir).await?;

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let file_stem = path.file_stem().unwrap_or("item");
        let ext = path.extension().map(|e| format!(".{}", e)).unwrap_or_default();
        let unique_id = format!("{}_{:x}", now, rand_simple_id());
        let trashed_filename = format!("{}_{}{}", unique_id, file_stem, ext);
        let dest_full = trash_dir.join(&trashed_filename);

        // Move o arquivo fisicamente para dentro da lixeira
        tokio::fs::rename(&source_full, &dest_full).await?;

        let item = TrashedItem {
            id: unique_id,
            original_path: path.clone(),
            trashed_filename,
            trashed_epoch_secs: now,
            size_bytes: metadata.len(),
        };

        // Atualiza manifesto
        let mut manifest = Self::load_manifest(vault_root).await?;
        manifest.items.push(item.clone());
        Self::save_manifest(vault_root, &manifest).await?;

        Ok(item)
    }

    /// Restaura um item da lixeira de volta para o seu caminho original dentro do Vault.
    pub async fn restore(vault_root: &Path, item_id: &str) -> Result<VaultPath> {
        let mut manifest = Self::load_manifest(vault_root).await?;
        let pos = match manifest.items.iter().position(|i| i.id == item_id) {
            Some(p) => p,
            None => return Err(NativeFsError::Trash(format!("Item com ID '{}' não encontrado na lixeira", item_id))),
        };

        let item = manifest.items.remove(pos);
        let trash_full = Self::trash_dir(vault_root).join(&item.trashed_filename);
        let original_full = vault_root.join(item.original_path.as_str());

        if original_full.exists() {
            return Err(NativeFsError::Trash(format!(
                "Não é possível restaurar: já existe um arquivo em '{}'",
                item.original_path
            )));
        }

        if let Some(parent) = original_full.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        tokio::fs::rename(&trash_full, &original_full).await?;
        Self::save_manifest(vault_root, &manifest).await?;

        Ok(item.original_path)
    }

    /// Retorna a lista de itens atualmente na lixeira do Vault.
    pub async fn list_trashed(vault_root: &Path) -> Result<Vec<TrashedItem>> {
        let manifest = Self::load_manifest(vault_root).await?;
        Ok(manifest.items)
    }

    /// Esvazia completamente a lixeira do Vault, deletando arquivos e o manifesto de forma irreversível.
    pub async fn empty_trash(vault_root: &Path) -> Result<usize> {
        let trash_dir = Self::trash_dir(vault_root);
        if !trash_dir.exists() {
            return Ok(0);
        }

        let manifest = Self::load_manifest(vault_root).await?;
        let count = manifest.items.len();

        tokio::fs::remove_dir_all(&trash_dir).await?;
        Ok(count)
    }

    async fn load_manifest(vault_root: &Path) -> Result<TrashManifest> {
        let path = Self::trash_dir(vault_root).join(TRASH_MANIFEST_FILE);
        if !path.exists() {
            return Ok(TrashManifest::default());
        }

        let bytes = tokio::fs::read(&path).await?;
        let manifest: TrashManifest = serde_json::from_slice(&bytes)?;
        Ok(manifest)
    }

    async fn save_manifest(vault_root: &Path, manifest: &TrashManifest) -> Result<()> {
        let trash_dir = Self::trash_dir(vault_root);
        tokio::fs::create_dir_all(&trash_dir).await?;

        let final_path = trash_dir.join(TRASH_MANIFEST_FILE);
        let tmp_path = trash_dir.join(format!("{}.tmp", TRASH_MANIFEST_FILE));

        let json_bytes = serde_json::to_vec_pretty(manifest)?;

        let mut file = tokio::fs::File::create(&tmp_path).await?;
        tokio::io::AsyncWriteExt::write_all(&mut file, &json_bytes).await?;
        file.sync_all().await?;

        tokio::fs::rename(&tmp_path, &final_path).await?;
        Ok(())
    }
}

fn rand_simple_id() -> u32 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(12345);
    nanos ^ 0x5bf03635
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_vault_trash_safe_delete_and_restore() -> Result<()> {
        let temp_dir = std::env::temp_dir().join(format!("gnosis_trash_test_{}", std::process::id()));
        let _ = tokio::fs::create_dir_all(&temp_dir).await;

        let note_path = VaultPath::parse("Pastas/NotaImportante.md")?;
        let full_note_path = temp_dir.join(note_path.as_str());

        if let Some(p) = full_note_path.parent() {
            tokio::fs::create_dir_all(p).await?;
        }
        tokio::fs::write(&full_note_path, "# Conteúdo Precioso").await?;

        // 1. Mover para a lixeira
        let item = VaultTrash::move_to_trash(&temp_dir, &note_path).await?;
        assert_eq!(item.original_path, note_path);
        assert!(!full_note_path.exists());

        let trashed_list = VaultTrash::list_trashed(&temp_dir).await?;
        assert_eq!(trashed_list.len(), 1);
        assert_eq!(trashed_list[0].id, item.id);

        // 2. Restaurar da lixeira
        let restored_path = VaultTrash::restore(&temp_dir, &item.id).await?;
        assert_eq!(restored_path, note_path);
        assert!(full_note_path.exists());

        let empty_list = VaultTrash::list_trashed(&temp_dir).await?;
        assert!(empty_list.is_empty());

        // 3. Esvaziar lixeira após novo descarte
        let _ = VaultTrash::move_to_trash(&temp_dir, &note_path).await?;
        let emptied = VaultTrash::empty_trash(&temp_dir).await?;
        assert_eq!(emptied, 1);

        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
        Ok(())
    }
}
