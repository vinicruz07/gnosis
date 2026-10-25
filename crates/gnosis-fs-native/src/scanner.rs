//! Scanner de diretório multithreaded para inicialização e varredura de Vaults.
//!
//! Recursos:
//! - Varredura rápida com [`walkdir::WalkDir`], ignorando pastas ocultas (`.git`, `.obsidian`, `.trash`).
//! - Classificação entre notas Markdown (`.md`) e anexos multimídia (imagens, PDFs, áudio).
//! - Concorrência limitada ao paralelismo real da CPU para evitar travamentos em hardware de 2015-.
//! - Geração de [`ScanReport`] detalhado com métricas de tempo e contagem de itens.

use crate::error::Result;
use gnosis_core::{CoreError, VaultIndex, VaultNote, VaultPath};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::Semaphore;
use walkdir::{DirEntry, WalkDir};

/// Categoria do anexo detectado no Vault.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttachmentKind {
    Image,
    Document,
    Audio,
    Video,
    Canvas,
    Other,
}

impl AttachmentKind {
    /// Identifica o tipo de anexo com base na extensão do arquivo.
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_ascii_lowercase().as_str() {
            "png" | "jpg" | "jpeg" | "gif" | "bmp" | "svg" | "webp" => Self::Image,
            "pdf" | "doc" | "docx" | "txt" => Self::Document,
            "mp3" | "wav" | "ogg" | "m4a" | "flac" => Self::Audio,
            "mp4" | "mov" | "webm" | "mkv" => Self::Video,
            "canvas" => Self::Canvas,
            _ => Self::Other,
        }
    }
}

/// Metadados de um anexo registrado no Vault.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentInfo {
    pub path: VaultPath,
    pub file_name: String,
    pub size_bytes: u64,
    pub kind: AttachmentKind,
}

/// Relatório de métricas produzido após a conclusão da varredura do Vault.
#[derive(Debug, Clone, Default)]
pub struct ScanReport {
    pub notes_indexed: usize,
    pub attachments_indexed: usize,
    pub errors_count: usize,
    pub duration_millis: u64,
}

/// Motor de varredura e indexação de arquivos de Vault.
pub struct VaultScanner;

impl VaultScanner {
    /// Executa a varredura do diretório raiz, povoando o [`VaultIndex`] com notas
    /// e retornando o mapa de anexos e o relatório de desempenho.
    pub async fn scan_vault(
        root_path: &Path,
        index: &mut VaultIndex,
    ) -> Result<(HashMap<VaultPath, AttachmentInfo>, ScanReport)> {
        let start_time = Instant::now();
        let canonical_root = match root_path.canonicalize() {
            Ok(p) => p,
            Err(_) => {
                return Err(crate::error::NativeFsError::NotFound(
                    root_path.to_path_buf(),
                ))
            }
        };

        // 1. Coleta síncrona dos arquivos válidos via WalkDir filtrando pastas ocultas
        let mut markdown_files: Vec<(PathBuf, VaultPath)> = Vec::new();
        let mut attachment_files: Vec<(PathBuf, VaultPath, u64)> = Vec::new();

        let walker = WalkDir::new(&canonical_root)
            .follow_links(false)
            .into_iter()
            .filter_entry(|e| !is_hidden_or_ignored(e));

        for entry_res in walker {
            let entry = match entry_res {
                Ok(e) => e,
                Err(_) => continue,
            };

            if entry.file_type().is_file() {
                let file_path = entry.path();
                if let Ok(relative) = file_path.strip_prefix(&canonical_root) {
                    let relative_str = relative.to_string_lossy();
                    if let Ok(vault_path) = VaultPath::parse(relative_str.as_ref()) {
                        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);

                        if vault_path.is_markdown() {
                            markdown_files.push((file_path.to_path_buf(), vault_path));
                        } else {
                            attachment_files.push((file_path.to_path_buf(), vault_path, size));
                        }
                    }
                }
            }
        }

        // 2. Mapeamento dos anexos
        let mut attachments = HashMap::new();
        for (_, vault_path, size) in attachment_files {
            let file_name = vault_path.file_name().unwrap_or("").to_string();
            let ext = vault_path.extension().unwrap_or("");
            let kind = AttachmentKind::from_extension(ext);

            let info = AttachmentInfo {
                path: vault_path.clone(),
                file_name,
                size_bytes: size,
                kind,
            };
            attachments.insert(vault_path, info);
        }

        // 3. Leitura e indexação concorrente de notas Markdown calibrada para o hardware
        let concurrency = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(2)
            .max(1)
            .min(8); // Limite prudente para computadores mais modestos de 2015-

        let semaphore = Arc::new(Semaphore::new(concurrency));
        let mut tasks = Vec::with_capacity(markdown_files.len());

        for (file_path, vault_path) in markdown_files {
            let sem = Arc::clone(&semaphore);

            tasks.push(tokio::spawn(async move {
                let _permit = sem.acquire().await;
                match tokio::fs::read_to_string(&file_path).await {
                    Ok(content) => {
                        let note = VaultNote::from_raw_markdown(vault_path, &content);
                        Ok(note)
                    }
                    Err(e) => Err(CoreError::Io(format!(
                        "Erro ao ler arquivo '{}': {}",
                        file_path.display(),
                        e
                    ))),
                }
            }));
        }

        let mut notes_count = 0;
        let mut errors_count = 0;

        for task in tasks {
            if let Ok(result) = task.await {
                match result {
                    Ok(note) => {
                        index.insert(note);
                        notes_count += 1;
                    }
                    Err(_) => {
                        errors_count += 1;
                    }
                }
            } else {
                errors_count += 1;
            }
        }

        let duration_millis = start_time.elapsed().as_millis() as u64;

        let report = ScanReport {
            notes_indexed: notes_count,
            attachments_indexed: attachments.len(),
            errors_count,
            duration_millis,
        };

        Ok((attachments, report))
    }
}

/// Verifica se um arquivo ou diretório deve ser ignorado na varredura inicial.
fn is_hidden_or_ignored(entry: &DirEntry) -> bool {
    let file_name = entry.file_name().to_string_lossy();

    // Ignora pastas e arquivos ocultos do sistema e do ecossistema Obsidian
    if file_name.starts_with('.') {
        return true;
    }

    match file_name.as_ref() {
        "target" | "node_modules" | "$RECYCLE.BIN" => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_scanner_with_synthetic_vault() -> CoreResult<()> {
        let temp_dir = std::env::temp_dir().join(format!("gnosis_scan_test_{}", std::process::id()));
        let _ = tokio::fs::create_dir_all(&temp_dir).await;

        // Cria estrutura de pastas e notas sintéticas
        let subpasta = temp_dir.join("Projetos");
        let _ = tokio::fs::create_dir_all(&subpasta).await;

        let pasta_oculta = temp_dir.join(".git");
        let _ = tokio::fs::create_dir_all(&pasta_oculta).await;

        // 1. Nota válida com frontmatter
        let nota1 = r#"---
title: Projeto Rust
aliases: [gnosis-rust]
tags: [dev]
---
# Conteúdo do projeto"#;
        tokio::fs::write(subpasta.join("gnosis.md"), nota1)
            .await
            .map_err(|e| CoreError::Io(e.to_string()))?;

        // 2. Anexo simulado
        tokio::fs::write(subpasta.join("diagrama.png"), b"FAKE_PNG_BYTES")
            .await
            .map_err(|e| CoreError::Io(e.to_string()))?;

        // 3. Arquivo em pasta oculta (deve ser ignorado)
        tokio::fs::write(pasta_oculta.join("ignored.md"), "# Ignorado")
            .await
            .map_err(|e| CoreError::Io(e.to_string()))?;

        let mut index = VaultIndex::new();
        let (attachments, report) = match VaultScanner::scan_vault(&temp_dir, &mut index).await {
            Ok(res) => res,
            Err(e) => return Err(CoreError::Io(format!("Erro no scanner: {}", e))),
        };

        // Validações
        assert_eq!(report.notes_indexed, 1);
        assert_eq!(report.attachments_indexed, 1);
        assert_eq!(report.errors_count, 0);

        // Verifica se a nota foi devidamente indexada no VaultIndex
        assert!(index.resolve_wikilink("gnosis-rust").is_some());
        assert!(index.resolve_wikilink("Projeto Rust").is_some());

        // Verifica o anexo
        let anexo_path = VaultPath::parse("Projetos/diagrama.png")?;
        if let Some(info) = attachments.get(&anexo_path) {
            assert_eq!(info.kind, AttachmentKind::Image);
            assert_eq!(info.size_bytes, 14);
        } else {
            return Err(CoreError::NotFound(anexo_path.to_string()));
        }

        // Limpeza
        let _ = tokio::fs::remove_dir_all(&temp_dir).await;

        Ok(())
    }
}
