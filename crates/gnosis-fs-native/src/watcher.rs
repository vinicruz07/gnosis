//! Monitor de alterações de arquivos do Vault em tempo real baseado em [`notify`].
//!
//! Recursos:
//! - Monitoramento recursivo em segundo plano com desacoplamento via canais assíncronos.
//! - Filtragem automática de arquivos temporários (`.tmp`), arquivos de trava e diretórios de sistema.
//! - Tradução de eventos de baixo nível do sistema operacional para eventos tipados [`VaultChangeEvent`].
//! - Atualização automática e atômica do [`VaultIndex`] em memória.

use crate::error::{NativeFsError, Result};
use gnosis_core::{CoreError, VaultIndex, VaultNote, VaultPath};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

/// Evento de domínio de alteração ocorrida dentro do Vault.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VaultChangeEvent {
    Created(VaultPath),
    Modified(VaultPath),
    Deleted(VaultPath),
    Renamed { from: VaultPath, to: VaultPath },
}

/// Gerenciador do monitor de arquivos em tempo real.
pub struct VaultFileWatcher {
    _watcher: RecommendedWatcher,
    root: PathBuf,
}

impl VaultFileWatcher {
    /// Inicia o monitoramento recursivo do diretório raiz do Vault.
    ///
    /// Retorna a instância do `VaultFileWatcher` e um receptor assíncrono de eventos [`VaultChangeEvent`].
    pub fn watch(root: &Path) -> Result<(Self, UnboundedReceiver<VaultChangeEvent>)> {
        let canonical_root = match root.canonicalize() {
            Ok(p) => p,
            Err(_) => return Err(NativeFsError::NotFound(root.to_path_buf())),
        };

        let (tx, rx) = unbounded_channel();
        let root_for_callback = Arc::new(canonical_root.clone());
        let sender = tx.clone();

        let event_handler = move |res: std::result::Result<Event, notify::Error>| {
            if let Ok(event) = res {
                Self::process_notify_event(&root_for_callback, &sender, event);
            }
        };

        let mut watcher = match RecommendedWatcher::new(event_handler, notify::Config::default()) {
            Ok(w) => w,
            Err(e) => return Err(NativeFsError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))),
        };

        if let Err(e) = watcher.watch(&canonical_root, RecursiveMode::Recursive) {
            return Err(NativeFsError::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())));
        }

        Ok((
            Self {
                _watcher: watcher,
                root: canonical_root,
            },
            rx,
        ))
    }

    /// Retorna a raiz monitorada pelo watcher.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Converte um evento bruto do `notify` em [`VaultChangeEvent`] filtrado e seguro.
    fn process_notify_event(
        root: &Path,
        sender: &UnboundedSender<VaultChangeEvent>,
        event: Event,
    ) {
        let mut valid_paths = Vec::new();

        for os_path in event.paths {
            // Ignora arquivos temporários (como .tmp das nossas gravações atômicas)
            if let Some(ext) = os_path.extension() {
                if ext == "tmp" {
                    continue;
                }
            }

            // Ignora arquivos e pastas ocultas (.git, .obsidian, etc.)
            let is_ignored = os_path.components().any(|c| {
                c.as_os_str().to_string_lossy().starts_with('.')
            });
            if is_ignored {
                continue;
            }

            if let Ok(relative) = os_path.strip_prefix(root) {
                let relative_str = relative.to_string_lossy();
                if let Ok(vault_path) = VaultPath::parse(relative_str.as_ref()) {
                    valid_paths.push(vault_path);
                }
            }
        }

        if valid_paths.is_empty() {
            return;
        }

        match event.kind {
            EventKind::Create(_) => {
                for path in valid_paths {
                    let _ = sender.send(VaultChangeEvent::Created(path));
                }
            }
            EventKind::Modify(_) => {
                for path in valid_paths {
                    let _ = sender.send(VaultChangeEvent::Modified(path));
                }
            }
            EventKind::Remove(_) => {
                for path in valid_paths {
                    let _ = sender.send(VaultChangeEvent::Deleted(path));
                }
            }
            _ => {}
        }
    }

    /// Aplica um evento de alteração diretamente ao [`VaultIndex`], mantendo a memória sincronizada com o disco.
    pub async fn apply_change_to_index(
        root: &Path,
        index: &mut VaultIndex,
        event: &VaultChangeEvent,
    ) -> std::result::Result<(), CoreError> {
        match event {
            VaultChangeEvent::Created(path) | VaultChangeEvent::Modified(path) => {
                if path.is_markdown() {
                    let full_path = root.join(path.as_str());
                    if full_path.exists() {
                        if let Ok(content) = tokio::fs::read_to_string(&full_path).await {
                            let note = VaultNote::from_raw_markdown(path.clone(), &content);
                            index.insert(note);
                        }
                    }
                }
            }
            VaultChangeEvent::Deleted(path) => {
                let _ = index.remove(path);
            }
            VaultChangeEvent::Renamed { from, to } => {
                let _ = index.remove(from);
                if to.is_markdown() {
                    let full_path = root.join(to.as_str());
                    if full_path.exists() {
                        if let Ok(content) = tokio::fs::read_to_string(&full_path).await {
                            let note = VaultNote::from_raw_markdown(to.clone(), &content);
                            index.insert(note);
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn test_watcher_detection_lifecycle() -> std::result::Result<(), CoreError> {
        let temp_dir = std::env::temp_dir().join(format!("gnosis_watch_test_{}", std::process::id()));
        let _ = tokio::fs::create_dir_all(&temp_dir).await;

        let (_watcher, mut rx) = match VaultFileWatcher::watch(&temp_dir) {
            Ok(res) => res,
            Err(e) => return Err(CoreError::Io(format!("Erro ao iniciar watcher: {}", e))),
        };

        let mut index = VaultIndex::new();

        // 1. Criação de arquivo externo
        let note_path = temp_dir.join("nota_externa.md");
        let content = r#"---
title: Nota do Watcher
aliases: [watcher-note]
tags: [tempo-real]
---
# Conteúdo Criado Externamente"#;

        tokio::fs::write(&note_path, content)
            .await
            .map_err(|e| CoreError::Io(e.to_string()))?;

        // Aguarda evento chegar via canal
        let timeout = Duration::from_millis(1500);
        let start = std::time::Instant::now();
        let mut received = false;

        while start.elapsed() < timeout {
            if let Ok(Some(event)) = tokio::time::timeout(Duration::from_millis(200), rx.recv()).await {
                let _ = VaultFileWatcher::apply_change_to_index(&temp_dir, &mut index, &event).await;
                received = true;
                break;
            }
        }

        assert!(received);
        // Verifica se a nota externa foi indexada automaticamente
        assert!(index.resolve_wikilink("watcher-note").is_some());
        assert!(index.resolve_wikilink("Nota do Watcher").is_some());

        // 2. Remoção do arquivo externo
        let _ = tokio::fs::remove_file(&note_path).await;

        let start_del = std::time::Instant::now();
        while start_del.elapsed() < timeout {
            if let Ok(Some(event)) = tokio::time::timeout(Duration::from_millis(200), rx.recv()).await {
                if matches!(event, VaultChangeEvent::Deleted(_)) {
                    let _ = VaultFileWatcher::apply_change_to_index(&temp_dir, &mut index, &event).await;
                    break;
                }
            }
        }

        // Limpeza
        let _ = tokio::fs::remove_dir_all(&temp_dir).await;

        Ok(())
    }
}
