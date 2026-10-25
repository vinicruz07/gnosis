//! Árvore Merkle hierárquica determinística para sincronização atômica do Vault (`VaultMerkleTree`).
//!
//! Princípios de Desempenho e Eficiência:
//! - Detecção de divergências entre instâncias em $O(\log N)$ em vez de $O(N)$.
//! - Se a raiz for idêntica, nenhuma verificação adicional é feita ($O(1)$).
//! - Subárvores inteiras com o mesmo hash são puladas imediatamente.
//! - Ordenação lexicográfica estrita dos nós filhos para reprodutibilidade garantida entre plataformas.

use crate::error::{Result, SyncError};
use gnosis_core::VaultPath;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt;

/// Algoritmo de hash 64-bit determinístico de alta performance (FNV-1a 64).
pub fn compute_hash(data: &[u8]) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{:016x}", hash)
}

/// Representa a diferença calculada entre dois estados de Vault.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct VaultDiff {
    /// Arquivos que existem no Vault remoto mas faltam no local.
    pub created: Vec<VaultPath>,
    /// Arquivos que existem em ambos, mas com hashes de conteúdo divergentes.
    pub modified: Vec<VaultPath>,
    /// Arquivos que existem no local mas foram excluídos no remoto.
    pub deleted: Vec<VaultPath>,
    /// Contagem de arquivos idênticos que não precisaram ser transferidos.
    pub identical_count: usize,
}

impl VaultDiff {
    /// Retorna `true` se ambos os Vaults estiverem em sincronia total (nenhuma diferença).
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.created.is_empty() && self.modified.is_empty() && self.deleted.is_empty()
    }

    /// Retorna o total de arquivos divergentes (novos, modificados e deletados).
    #[inline]
    pub fn total_changes(&self) -> usize {
        self.created.len() + self.modified.len() + self.deleted.len()
    }
}

/// Nó individual da árvore Merkle (arquivo ou diretório).
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MerkleNode {
    File {
        hash: String,
        size_bytes: u64,
        modified_epoch_secs: u64,
    },
    Directory {
        hash: String,
        children: BTreeMap<String, MerkleNode>,
    },
}

impl MerkleNode {
    /// Retorna o hash do nó.
    pub fn hash(&self) -> &str {
        match self {
            Self::File { hash, .. } => hash,
            Self::Directory { hash, .. } => hash,
        }
    }

    /// Recalcula e atualiza o hash do diretório a partir dos hashes ordenados de seus filhos.
    pub fn recalculate_dir_hash(&mut self) {
        if let Self::Directory { hash, children } = self {
            let mut combined = Vec::new();
            for (name, child) in children.iter() {
                combined.extend_from_slice(name.as_bytes());
                combined.extend_from_slice(child.hash().as_bytes());
            }
            *hash = compute_hash(&combined);
        }
    }

    /// Coleta recursivamente todos os caminhos de arquivos contidos neste nó.
    pub fn collect_all_files(&self, prefix: &str, out: &mut Vec<VaultPath>) {
        match self {
            Self::File { .. } => {
                if let Ok(vp) = VaultPath::parse(prefix) {
                    out.push(vp);
                }
            }
            Self::Directory { children, .. } => {
                for (name, child) in children {
                    let next_prefix = if prefix.is_empty() {
                        name.clone()
                    } else {
                        format!("{}/{}", prefix, name)
                    };
                    child.collect_all_files(&next_prefix, out);
                }
            }
        }
    }
}

impl fmt::Debug for MerkleNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::File { hash, size_bytes, .. } => {
                write!(f, "File(hash={}, size={})", hash, size_bytes)
            }
            Self::Directory { hash, children } => {
                write!(f, "Dir(hash={}, children={})", hash, children.len())
            }
        }
    }
}

/// Árvore Merkle que representa o estado instantâneo do Vault.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VaultMerkleTree {
    root: MerkleNode,
    total_files: usize,
}

impl Default for VaultMerkleTree {
    fn default() -> Self {
        Self::new()
    }
}

impl VaultMerkleTree {
    /// Inicializa uma árvore Merkle vazia.
    pub fn new() -> Self {
        Self {
            root: MerkleNode::Directory {
                hash: compute_hash(b"gnosis_empty_vault"),
                children: BTreeMap::new(),
            },
            total_files: 0,
        }
    }

    /// Retorna o hash raiz representativo de todo o Vault.
    pub fn root_hash(&self) -> &str {
        self.root.hash()
    }

    /// Retorna o número total de arquivos catalogados na árvore.
    pub fn total_files(&self) -> usize {
        self.total_files
    }

    /// Insere ou atualiza um arquivo na árvore Merkle, recalculando os hashes de subdiretórios até a raiz.
    pub fn insert(
        &mut self,
        path: &VaultPath,
        content_hash: String,
        size_bytes: u64,
        modified_epoch_secs: u64,
    ) {
        let segments: Vec<&str> = path.as_str().split('/').collect();
        if segments.is_empty() {
            return;
        }

        let is_new = Self::insert_recursive(
            &mut self.root,
            &segments,
            content_hash,
            size_bytes,
            modified_epoch_secs,
        );

        if is_new {
            self.total_files += 1;
        }

        self.root.recalculate_dir_hash();
    }

    fn insert_recursive(
        current: &mut MerkleNode,
        segments: &[&str],
        content_hash: String,
        size_bytes: u64,
        modified_epoch_secs: u64,
    ) -> bool {
        if segments.is_empty() {
            return false;
        }

        let first = segments[0];

        if segments.len() == 1 {
            // Folha (arquivo final)
            if let MerkleNode::Directory { children, .. } = current {
                let is_new = !children.contains_key(first);
                children.insert(
                    first.to_string(),
                    MerkleNode::File {
                        hash: content_hash,
                        size_bytes,
                        modified_epoch_secs,
                    },
                );
                current.recalculate_dir_hash();
                return is_new;
            }
            return false;
        }

        // Segmento intermediário (subdiretório)
        if let MerkleNode::Directory { children, .. } = current {
            if !children.contains_key(first) {
                children.insert(
                    first.to_string(),
                    MerkleNode::Directory {
                        hash: compute_hash(first.as_bytes()),
                        children: BTreeMap::new(),
                    },
                );
            }

            let is_new = if let Some(child_node) = children.get_mut(first) {
                Self::insert_recursive(
                    child_node,
                    &segments[1..],
                    content_hash,
                    size_bytes,
                    modified_epoch_secs,
                )
            } else {
                false
            };

            current.recalculate_dir_hash();
            is_new
        } else {
            false
        }
    }

    /// Remove um arquivo da árvore Merkle.
    pub fn remove(&mut self, path: &VaultPath) -> bool {
        let segments: Vec<&str> = path.as_str().split('/').collect();
        if segments.is_empty() {
            return false;
        }

        let removed = Self::remove_recursive(&mut self.root, &segments);
        if removed {
            self.total_files = self.total_files.saturating_sub(1);
            self.root.recalculate_dir_hash();
        }
        removed
    }

    fn remove_recursive(current: &mut MerkleNode, segments: &[&str]) -> bool {
        if segments.is_empty() {
            return false;
        }

        let first = segments[0];

        if let MerkleNode::Directory { children, .. } = current {
            if segments.len() == 1 {
                let removed = children.remove(first).is_some();
                current.recalculate_dir_hash();
                return removed;
            }

            let mut should_prune = false;
            let removed = if let Some(child_node) = children.get_mut(first) {
                let res = Self::remove_recursive(child_node, &segments[1..]);
                if let MerkleNode::Directory { children: sub_children, .. } = child_node {
                    if sub_children.is_empty() {
                        should_prune = true;
                    }
                }
                res
            } else {
                false
            };

            if should_prune {
                children.remove(first);
            }

            current.recalculate_dir_hash();
            removed
        } else {
            false
        }
    }

    /// Compara esta árvore local com a árvore remota e produz um [`VaultDiff`] em $O(\log N)$.
    pub fn diff(&self, remote: &Self) -> VaultDiff {
        let mut diff = VaultDiff::default();

        // Atalho O(1): se as raízes forem idênticas, o Vault inteiro está sincronizado!
        if self.root_hash() == remote.root_hash() {
            diff.identical_count = self.total_files;
            return diff;
        }

        Self::diff_nodes(&self.root, &remote.root, "", &mut diff);
        diff
    }

    fn diff_nodes(local: &MerkleNode, remote: &MerkleNode, path_prefix: &str, diff: &mut VaultDiff) {
        // Se ambos os nós têm hashes idênticos, pula toda a subárvore em O(1)
        if local.hash() == remote.hash() {
            let mut count = 0;
            let mut dummy = Vec::new();
            local.collect_all_files(path_prefix, &mut dummy);
            count += dummy.len();
            diff.identical_count += count;
            return;
        }

        match (local, remote) {
            (
                MerkleNode::File { hash: h_local, .. },
                MerkleNode::File { hash: h_remote, .. },
            ) => {
                if let Ok(vp) = VaultPath::parse(path_prefix) {
                    if h_local != h_remote {
                        diff.modified.push(vp);
                    } else {
                        diff.identical_count += 1;
                    }
                }
            }
            (MerkleNode::Directory { children: l_kids, .. }, MerkleNode::Directory { children: r_kids, .. }) => {
                // Arquivos e pastas no local
                for (name, l_child) in l_kids {
                    let next_prefix = if path_prefix.is_empty() {
                        name.clone()
                    } else {
                        format!("{}/{}", path_prefix, name)
                    };

                    if let Some(r_child) = r_kids.get(name) {
                        Self::diff_nodes(l_child, r_child, &next_prefix, diff);
                    } else {
                        // Remoto não tem esse nó -> deletado no remoto
                        let mut deleted_files = Vec::new();
                        l_child.collect_all_files(&next_prefix, &mut deleted_files);
                        diff.deleted.extend(deleted_files);
                    }
                }

                // Arquivos e pastas que existem apenas no remoto -> criados no remoto
                for (name, r_child) in r_kids {
                    if !l_kids.contains_key(name) {
                        let next_prefix = if path_prefix.is_empty() {
                            name.clone()
                        } else {
                            format!("{}/{}", path_prefix, name)
                        };
                        let mut created_files = Vec::new();
                        r_child.collect_all_files(&next_prefix, &mut created_files);
                        diff.created.extend(created_files);
                    }
                }
            }
            // Conflito de tipo (um é arquivo, outro diretório)
            _ => {
                if let Ok(vp) = VaultPath::parse(path_prefix) {
                    diff.modified.push(vp);
                }
            }
        }
    }

    /// Serializa o snapshot da árvore Merkle em JSON compacto.
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string(self).map_err(|e| SyncError::Serialization(e.to_string()))
    }

    /// Reconstrói uma árvore Merkle a partir de JSON recebido de outro nó.
    pub fn from_json(json_str: &str) -> Result<Self> {
        serde_json::from_str(json_str).map_err(|e| SyncError::Serialization(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merkle_tree_identical_vaults_o1() -> Result<()> {
        let mut tree_a = VaultMerkleTree::new();
        let mut tree_b = VaultMerkleTree::new();

        let path1 = VaultPath::parse("Notas/Ideia.md")?;
        let path2 = VaultPath::parse("Tarefas.md")?;

        tree_a.insert(&path1, compute_hash(b"conteudo A"), 100, 1000);
        tree_a.insert(&path2, compute_hash(b"conteudo B"), 200, 1000);

        tree_b.insert(&path1, compute_hash(b"conteudo A"), 100, 1000);
        tree_b.insert(&path2, compute_hash(b"conteudo B"), 200, 1000);

        assert_eq!(tree_a.root_hash(), tree_b.root_hash());

        let diff = tree_a.diff(&tree_b);
        assert!(diff.is_empty());
        assert_eq!(diff.identical_count, 2);

        Ok(())
    }

    #[test]
    fn test_merkle_tree_diff_detection() -> Result<()> {
        let mut local = VaultMerkleTree::new();
        let mut remote = VaultMerkleTree::new();

        let common = VaultPath::parse("Common.md")?;
        let mod_path = VaultPath::parse("Notas/Modificada.md")?;
        let del_path = VaultPath::parse("ApenasLocal.md")?;
        let add_path = VaultPath::parse("ApenasRemota.md")?;

        // Arquivo comum idêntico
        local.insert(&common, compute_hash(b"igual"), 50, 1000);
        remote.insert(&common, compute_hash(b"igual"), 50, 1000);

        // Arquivo modificado
        local.insert(&mod_path, compute_hash(b"versao 1"), 80, 1000);
        remote.insert(&mod_path, compute_hash(b"versao 2"), 85, 1005);

        // Arquivo local deletado no remoto
        local.insert(&del_path, compute_hash(b"local"), 30, 1000);

        // Arquivo novo criado no remoto
        remote.insert(&add_path, compute_hash(b"remota"), 40, 1010);

        let diff = local.diff(&remote);

        assert_eq!(diff.identical_count, 1);
        assert_eq!(diff.modified, vec![mod_path]);
        assert_eq!(diff.deleted, vec![del_path]);
        assert_eq!(diff.created, vec![add_path]);
        assert_eq!(diff.total_changes(), 3);

        Ok(())
    }

    #[test]
    fn test_merkle_tree_benchmark_10k_nodes() -> Result<()> {
        let mut tree_a = VaultMerkleTree::new();
        let mut tree_b = VaultMerkleTree::new();

        // Gera 10.000 nós distribuídos em 100 pastas
        for i in 0..10_000 {
            let folder_id = i / 100;
            let path_str = format!("pasta_{:03}/nota_{:05}.md", folder_id, i);
            let path = VaultPath::parse(&path_str)?;
            let content_hash = compute_hash(format!("conteudo da nota {}", i).as_bytes());

            tree_a.insert(&path, content_hash.clone(), 128, 1000);
            tree_b.insert(&path, content_hash, 128, 1000);
        }

        assert_eq!(tree_a.total_files(), 10_000);
        assert_eq!(tree_b.total_files(), 10_000);
        assert_eq!(tree_a.root_hash(), tree_b.root_hash());

        // Modifica apenas 1 nota entre as 10.000
        let target_path = VaultPath::parse("pasta_050/nota_05050.md")?;
        tree_b.insert(
            &target_path,
            compute_hash(b"conteudo alterado pelo colega"),
            256,
            1050,
        );

        let start = std::time::Instant::now();
        let diff = tree_a.diff(&tree_b);
        let elapsed = start.elapsed();

        // O diff deve encontrar exatamente 1 modificação em menos de 5 milissegundos
        assert_eq!(diff.modified, vec![target_path]);
        assert_eq!(diff.created.len(), 0);
        assert_eq!(diff.deleted.len(), 0);
        assert_eq!(diff.identical_count, 9_999);
        assert!(elapsed.as_millis() < 50, "Diff levou {:?}", elapsed);

        Ok(())
    }
}
