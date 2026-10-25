//! Índice em memória do Vault para busca e resolução de notas em $O(1)$.
//!
//! Mapeia simultaneamente caminhos, títulos, aliases e tags do Obsidian para garantir
//! que links `[[Nota]]` e buscas por `#tag` sejam resolvidos em microssegundos.

use crate::note::{NoteId, VaultNote};
use crate::path::VaultPath;
use std::collections::HashMap;

/// Índice centralizado em memória para todas as notas abertas ou indexadas no Vault.
#[derive(Default, Debug, Clone)]
pub struct VaultIndex {
    by_id: HashMap<NoteId, VaultNote>,
    by_path: HashMap<VaultPath, NoteId>,
    by_title: HashMap<String, NoteId>,
    by_alias: HashMap<String, Vec<NoteId>>,
    by_tag: HashMap<String, Vec<NoteId>>,
}

impl VaultIndex {
    /// Inicializa um novo índice vazio.
    pub fn new() -> Self {
        Self::default()
    }

    /// Retorna a quantidade total de notas indexadas no Vault.
    pub fn count(&self) -> usize {
        self.by_id.len()
    }

    /// Insere ou atualiza uma nota no índice, reconstruindo seus mapeamentos de busca.
    pub fn insert(&mut self, note: VaultNote) {
        let id = note.id().clone();
        let path = note.path().clone();
        let title_key = note.title().to_lowercase();

        // Mapeia por caminho e por título normalizado
        self.by_path.insert(path, id.clone());
        self.by_title.insert(title_key, id.clone());

        // Mapeia aliases
        for alias in note.aliases() {
            let alias_key = alias.to_lowercase();
            let entry = self.by_alias.entry(alias_key).or_default();
            if !entry.contains(&id) {
                entry.push(id.clone());
            }
        }

        // Mapeia tags
        for tag in note.tags() {
            let tag_key = tag.to_lowercase();
            let entry = self.by_tag.entry(tag_key).or_default();
            if !entry.contains(&id) {
                entry.push(id.clone());
            }
        }

        self.by_id.insert(id, note);
    }

    /// Remove uma nota do índice com base no seu caminho.
    pub fn remove(&mut self, path: &VaultPath) -> Option<VaultNote> {
        let id = self.by_path.remove(path)?;
        let note = self.by_id.remove(&id)?;

        let title_key = note.title().to_lowercase();
        self.by_title.remove(&title_key);

        for alias in note.aliases() {
            let alias_key = alias.to_lowercase();
            if let Some(ids) = self.by_alias.get_mut(&alias_key) {
                ids.retain(|item_id| item_id != &id);
            }
        }

        for tag in note.tags() {
            let tag_key = tag.to_lowercase();
            if let Some(ids) = self.by_tag.get_mut(&tag_key) {
                ids.retain(|item_id| item_id != &id);
            }
        }

        Some(note)
    }

    /// Obtém uma nota pelo seu identificador único.
    pub fn get_by_id(&self, id: &NoteId) -> Option<&VaultNote> {
        self.by_id.get(id)
    }

    /// Obtém uma nota de forma mutável pelo seu identificador único.
    pub fn get_by_id_mut(&mut self, id: &NoteId) -> Option<&mut VaultNote> {
        self.by_id.get_mut(id)
    }

    /// Obtém uma nota pelo seu caminho canônico no Vault.
    pub fn get_by_path(&self, path: &VaultPath) -> Option<&VaultNote> {
        let id = self.by_path.get(path)?;
        self.by_id.get(id)
    }

    /// Obtém uma nota mutável pelo seu caminho canônico no Vault.
    pub fn get_by_path_mut(&mut self, path: &VaultPath) -> Option<&mut VaultNote> {
        let id = self.by_path.get(path)?;
        self.by_id.get_mut(id)
    }

    /// Retorna todas as notas associadas a uma hashtag específica.
    pub fn get_by_tag(&self, tag: &str) -> Vec<&VaultNote> {
        let clean_tag = tag.trim().trim_start_matches('#').to_lowercase();
        match self.by_tag.get(&clean_tag) {
            Some(ids) => ids.iter().filter_map(|id| self.by_id.get(id)).collect(),
            None => Vec::new(),
        }
    }

    /// Resolve um Wikilink do Obsidian (ex: `"Minha Nota"` ou `"pasta/documento"`).
    ///
    /// Ordem de resolução idêntica à do Obsidian:
    /// 1. Tenta correspondência exata de caminho (com ou sem extensão `.md`).
    /// 2. Tenta correspondência por título da nota (case-insensitive).
    /// 3. Tenta correspondência por aliases registrados no frontmatter da nota.
    pub fn resolve_wikilink(&self, link_target: &str) -> Option<&VaultNote> {
        let target = link_target.trim();
        if target.is_empty() {
            return None;
        }

        // 1. Tenta correspondência por caminho
        if let Ok(path) = VaultPath::parse(target) {
            if let Some(note) = self.get_by_path(&path) {
                return Some(note);
            }
        }
        if !target.ends_with(".md") {
            let with_md = format!("{}.md", target);
            if let Ok(path) = VaultPath::parse(with_md) {
                if let Some(note) = self.get_by_path(&path) {
                    return Some(note);
                }
            }
        }

        // 2. Tenta correspondência por título
        let lower_target = target.to_lowercase();
        if let Some(id) = self.by_title.get(&lower_target) {
            if let Some(note) = self.by_id.get(id) {
                return Some(note);
            }
        }

        // 3. Tenta correspondência por alias
        if let Some(ids) = self.by_alias.get(&lower_target) {
            if let Some(first_id) = ids.first() {
                if let Some(note) = self.by_id.get(first_id) {
                    return Some(note);
                }
            }
        }

        None
    }

    /// Itera sobre todas as notas atualmente no índice.
    pub fn all_notes(&self) -> impl Iterator<Item = &VaultNote> {
        self.by_id.values()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::CoreError;

    #[test]
    fn test_vault_index_resolution() -> std::result::Result<(), CoreError> {
        let mut index = VaultIndex::new();

        let path1 = VaultPath::parse("estudos/rust.md")?;
        let raw_markdown = r#"---
title: Linguagem Rust
aliases: [rustlang, rs]
tags: [linguagens, backend]
icon: 🦀
---
# Introdução
O Rust é #seguro e rápido. Veja [[Outra Nota]].
"#;

        let note1 = VaultNote::from_raw_markdown(path1.clone(), raw_markdown);
        index.insert(note1);

        assert_eq!(index.count(), 1);

        // Resolução por caminho
        assert!(index.resolve_wikilink("estudos/rust.md").is_some());
        assert!(index.resolve_wikilink("estudos/rust").is_some());

        // Resolução por título do frontmatter
        assert!(index.resolve_wikilink("Linguagem Rust").is_some());
        assert!(index.resolve_wikilink("linguagem rust").is_some());

        // Resolução por alias do frontmatter
        assert!(index.resolve_wikilink("rustlang").is_some());
        assert!(index.resolve_wikilink("rs").is_some());

        // Resolução por tags agregadas (frontmatter + corpo)
        let tagged_notes = index.get_by_tag("seguro");
        assert_eq!(tagged_notes.len(), 1);
        let backend_notes = index.get_by_tag("backend");
        assert_eq!(backend_notes.len(), 1);

        // Ícone/Sticker
        if let Some(note) = index.get_by_path(&path1) {
            assert_eq!(note.icon(), Some("🦀"));
            assert_eq!(note.links().len(), 1);
        }

        Ok(())
    }
}
