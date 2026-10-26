//! Modelagem do Grafo de Conhecimento em memória com `petgraph` (`KnowledgeGraph`).
//!
//! Recursos:
//! - Grafo direcionado tipado (`NodeType`: `Note`, `Tag`, `Attachment`, `Unresolved`).
//! - Suporte a "Notas Fantasmas" (links para notas que ainda não foram criadas no Vault).
//! - Consultas instantâneas em $O(1)$: Backlinks, Forward links, notas órfãs e centralidade de nós.
//! - Construção e atualização reativa a partir das notas do `VaultIndex`.

use crate::error::{GraphError, Result};
use gnosis_core::{VaultNote, VaultPath};
use petgraph::graph::{DiGraph, EdgeIndex, NodeIndex};
use petgraph::visit::EdgeRef;
use petgraph::Direction;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Tipos de nós representáveis no grafo de conhecimento.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NodeType {
    /// Nota Markdown existente fisicamente no Vault.
    Note,
    /// Tag temática (ex: `#rust`, `#filosofia`).
    Tag,
    /// Arquivo multimídia ou documento anexo (ex: `.png`, `.pdf`).
    Attachment,
    /// Nota fantasma / link não resolvido (`[[Nota Ainda Nao Criada]]`).
    Unresolved,
}

/// Metadados de um nó no grafo de conhecimento.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphNode {
    /// Identificador canônico único (ex: caminho da nota ou `#tag`).
    pub id: String,
    /// Título ou rótulo de exibição no grafo.
    pub label: String,
    /// Categoria do nó.
    pub node_type: NodeType,
    /// Caminho relativo seguro no Vault (se for arquivo físico).
    pub path: Option<VaultPath>,
}

/// Tipos de arestas/conexões entre os nós.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EdgeType {
    /// Wikilink direcionado explícito (`[[target]]`).
    WikiLink,
    /// Associação de nota com uma tag (`#tag`).
    TagRelation,
    /// Inclusão multimídia / embed (`![[imagem.png]]`).
    Embed,
}

/// Metadados de uma aresta direcionada no grafo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphEdge {
    pub edge_type: EdgeType,
    pub label: Option<String>,
    pub weight: f32,
}

/// Grafo de conhecimento em memória representando todas as interconexões do Vault.
pub struct KnowledgeGraph {
    graph: DiGraph<GraphNode, GraphEdge>,
    node_indices: HashMap<String, NodeIndex>,
}

impl Default for KnowledgeGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl KnowledgeGraph {
    /// Inicializa um grafo de conhecimento vazio.
    pub fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            node_indices: HashMap::new(),
        }
    }

    /// Retorna a contagem total de nós no grafo.
    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    /// Retorna a contagem total de arestas no grafo.
    pub fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }

    /// Adiciona ou obtém um nó existente no grafo.
    ///
    /// Se o nó já existia como `Unresolved` e agora está sendo adicionado como `Note`,
    /// o tipo é promovido automaticamente.
    pub fn add_or_update_node(
        &mut self,
        id: &str,
        label: &str,
        node_type: NodeType,
        path: Option<VaultPath>,
    ) -> NodeIndex {
        if let Some(&idx) = self.node_indices.get(id) {
            if let Some(node) = self.graph.node_weight_mut(idx) {
                if node.node_type == NodeType::Unresolved && node_type == NodeType::Note {
                    node.node_type = NodeType::Note;
                    node.label = label.to_string();
                    node.path = path;
                }
            }
            return idx;
        }

        let node = GraphNode {
            id: id.to_string(),
            label: label.to_string(),
            node_type,
            path,
        };

        let idx = self.graph.add_node(node);
        self.node_indices.insert(id.to_string(), idx);
        idx
    }

    /// Conecta dois nós através de uma aresta direcionada.
    ///
    /// Se o nó de destino não existir no grafo, ele é criado automaticamente como `Unresolved`
    /// (suporte nativo a notas fantasmas linkadas).
    pub fn add_edge(
        &mut self,
        source_id: &str,
        target_id: &str,
        edge_type: EdgeType,
        label: Option<String>,
        weight: f32,
    ) -> Result<EdgeIndex> {
        let source_idx = match self.node_indices.get(source_id) {
            Some(&idx) => idx,
            None => {
                return Err(GraphError::NodeNotFound(source_id.to_string()));
            }
        };

        let target_idx = match self.node_indices.get(target_id) {
            Some(&idx) => idx,
            None => {
                // Cria nota fantasma não resolvida
                self.add_or_update_node(target_id, target_id, NodeType::Unresolved, None)
            }
        };

        // Evita arestas duplicadas do mesmo tipo entre os mesmos nós
        let mut edges = self.graph.edges_connecting(source_idx, target_idx);
        if let Some(existing) = edges.find(|e| e.weight().edge_type == edge_type) {
            return Ok(existing.id());
        }

        let edge = GraphEdge {
            edge_type,
            label,
            weight,
        };

        Ok(self.graph.add_edge(source_idx, target_idx, edge))
    }

    /// Popula o grafo a partir de uma coleção de notas do Vault.
    pub fn populate_from_notes(&mut self, notes: &[VaultNote]) {
        // 1. Cadastra todas as notas conhecidas
        for note in notes {
            let id = note.path().as_str();
            self.add_or_update_node(id, note.title(), NodeType::Note, Some(note.path().clone()));
        }

        // 2. Conecta arestas de Wikilinks e Tags
        for note in notes {
            let source_id = note.path().as_str();

            // Wikilinks
            for link in note.links() {
                let edge_type = if link.is_embed {
                    EdgeType::Embed
                } else {
                    EdgeType::WikiLink
                };

                let _ = self.add_edge(
                    source_id,
                    &link.target,
                    edge_type,
                    link.alias.clone(),
                    1.0,
                );
            }

            // Tags
            for tag in note.tags() {
                let tag_id = format!("#{}", tag);
                self.add_or_update_node(&tag_id, &tag_id, NodeType::Tag, None);
                let _ = self.add_edge(
                    source_id,
                    &tag_id,
                    EdgeType::TagRelation,
                    None,
                    0.5,
                );
            }
        }
    }

    /// Retorna todos os Backlinks de um nó ("quem aponta para esta nota?").
    pub fn backlinks(&self, node_id: &str) -> Vec<&GraphNode> {
        let Some(&idx) = self.node_indices.get(node_id) else {
            return Vec::new();
        };

        self.graph
            .neighbors_directed(idx, Direction::Incoming)
            .filter_map(|neighbor_idx| self.graph.node_weight(neighbor_idx))
            .collect()
    }

    /// Retorna todos os Forward Links de um nó ("para quem esta nota aponta?").
    pub fn forward_links(&self, node_id: &str) -> Vec<&GraphNode> {
        let Some(&idx) = self.node_indices.get(node_id) else {
            return Vec::new();
        };

        self.graph
            .neighbors_directed(idx, Direction::Outgoing)
            .filter_map(|neighbor_idx| self.graph.node_weight(neighbor_idx))
            .collect()
    }

    /// Retorna todas as notas órfãs do Vault (grau total de conexões igual a zero).
    pub fn orphan_notes(&self) -> Vec<&GraphNode> {
        self.graph
            .node_indices()
            .filter_map(|idx| {
                let node = self.graph.node_weight(idx)?;
                if node.node_type == NodeType::Note {
                    let in_degree = self.graph.neighbors_directed(idx, Direction::Incoming).count();
                    let out_degree = self.graph.neighbors_directed(idx, Direction::Outgoing).count();
                    if in_degree == 0 && out_degree == 0 {
                        Some(node)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect()
    }

    /// Retorna todas as notas fantasmas linkadas que ainda não foram criadas no disco.
    pub fn unresolved_notes(&self) -> Vec<&GraphNode> {
        self.graph
            .node_indices()
            .filter_map(|idx| {
                let node = self.graph.node_weight(idx)?;
                if node.node_type == NodeType::Unresolved {
                    Some(node)
                } else {
                    None
                }
            })
            .collect()
    }

    /// Retorna o grau total de conectividade de um nó.
    pub fn node_degree(&self, node_id: &str) -> usize {
        let Some(&idx) = self.node_indices.get(node_id) else {
            return 0;
        };

        let in_deg = self.graph.neighbors_directed(idx, Direction::Incoming).count();
        let out_deg = self.graph.neighbors_directed(idx, Direction::Outgoing).count();
        in_deg + out_deg
    }

    /// Retorna os nós mais densos/conectados do Vault ordenados por grau decrescente.
    pub fn densest_nodes(&self, limit: usize) -> Vec<(&GraphNode, usize)> {
        let mut nodes: Vec<(&GraphNode, usize)> = self
            .graph
            .node_indices()
            .filter_map(|idx| {
                let node = self.graph.node_weight(idx)?;
                let deg = self.graph.neighbors_directed(idx, Direction::Incoming).count()
                    + self.graph.neighbors_directed(idx, Direction::Outgoing).count();
                Some((node, deg))
            })
            .collect();

        nodes.sort_by(|a, b| b.1.cmp(&a.1));
        nodes.truncate(limit);
        nodes
    }

    /// Obtém um nó pelo seu identificador.
    pub fn get_node(&self, node_id: &str) -> Option<&GraphNode> {
        let idx = self.node_indices.get(node_id)?;
        self.graph.node_weight(*idx)
    }

    /// Retorna a referência interna ao grafo `petgraph`.
    pub fn inner_graph(&self) -> &DiGraph<GraphNode, GraphEdge> {
        &self.graph
    }

    /// Retorna todos os nós cadastrados no grafo.
    pub fn all_nodes(&self) -> Vec<&GraphNode> {
        self.graph.node_weights().collect()
    }

    /// Retorna todas as arestas do grafo como tuplas (origem_id, destino_id, &GraphEdge).
    pub fn all_edges(&self) -> Vec<(&str, &str, &GraphEdge)> {
        use petgraph::visit::EdgeRef;
        self.graph
            .edge_references()
            .filter_map(|e| {
                let source = self.graph.node_weight(e.source())?;
                let target = self.graph.node_weight(e.target())?;
                Some((source.id.as_str(), target.id.as_str(), e.weight()))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_graph_backlinks_and_unresolved_links() -> Result<()> {
        let mut graph = KnowledgeGraph::new();

        let path_a = VaultPath::parse("Notas/Rust.md")?;
        let path_b = VaultPath::parse("Notas/Gnosis.md")?;

        let raw_a = r#"---
title: Linguagem Rust
tags: [programacao, rust]
---
O [[Notas/Gnosis.md]] é feito em Rust. Também recomendo ver [[Ideia Inexistente]]."#;

        let raw_b = r#"---
title: Projeto Gnosis
tags: [rust]
---
O Gnosis usa [[Notas/Rust.md]]."#;

        let note_a = VaultNote::from_raw_markdown(path_a, raw_a);
        let note_b = VaultNote::from_raw_markdown(path_b, raw_b);

        graph.populate_from_notes(&[note_a, note_b]);

        assert_eq!(graph.node_count(), 5); // Rust.md, Gnosis.md, #programacao, #rust, Ideia Inexistente (Unresolved)

        // 1. Backlinks: Quem referencia Gnosis.md? (deve ser Rust.md)
        let backlinks_gnosis = graph.backlinks("Notas/Gnosis.md");
        assert_eq!(backlinks_gnosis.len(), 1);
        assert_eq!(backlinks_gnosis[0].id, "Notas/Rust.md");

        // 2. Unresolved Links (Notas Fantasmas):
        let unresolved = graph.unresolved_notes();
        assert_eq!(unresolved.len(), 1);
        assert_eq!(unresolved[0].id, "Ideia Inexistente");
        assert_eq!(unresolved[0].node_type, NodeType::Unresolved);

        // 3. Centralidade e densidade
        let densest = graph.densest_nodes(2);
        assert!(!densest.is_empty());

        Ok(())
    }

    #[test]
    fn test_orphan_notes_detection() -> Result<()> {
        let mut graph = KnowledgeGraph::new();

        let path_isolated = VaultPath::parse("Solitaria.md")?;
        let path_connected_a = VaultPath::parse("A.md")?;
        let path_connected_b = VaultPath::parse("B.md")?;

        let note_isolated = VaultNote::from_raw_markdown(path_isolated, "Apenas texto sem nenhum link.");
        let note_a = VaultNote::from_raw_markdown(path_connected_a, "Link para [[B.md]]");
        let note_b = VaultNote::from_raw_markdown(path_connected_b, "Nota B.");

        graph.populate_from_notes(&[note_isolated, note_a, note_b]);

        let orphans = graph.orphan_notes();
        assert_eq!(orphans.len(), 1);
        assert_eq!(orphans[0].id, "Solitaria.md");

        Ok(())
    }

    #[test]
    fn test_benchmark_5k_nodes_backlinks() -> Result<()> {
        let mut graph = KnowledgeGraph::new();

        // Cria 5.000 nós no grafo em estrutura sintética
        for i in 0..5_000 {
            let id = format!("nota_{:04}.md", i);
            graph.add_or_update_node(&id, &id, NodeType::Note, None);
        }

        // Conecta arestas em anel + estrela central (nó 0 conecta com muitos)
        for i in 1..5_000 {
            let source = format!("nota_{:04}.md", i);
            let target = format!("nota_{:04}.md", (i + 1) % 5_000);
            let _ = graph.add_edge(&source, &target, EdgeType::WikiLink, None, 1.0)?;

            // Cada nó também referencia o nó central "nota_0000.md"
            let _ = graph.add_edge(&source, "nota_0000.md", EdgeType::WikiLink, None, 1.0)?;
        }

        assert_eq!(graph.node_count(), 5_000);

        // Mede tempo de consulta de backlinks no nó central (que tem 4.999 conexões incoming)
        let start = std::time::Instant::now();
        let central_backlinks = graph.backlinks("nota_0000.md");
        let elapsed = start.elapsed();

        assert_eq!(central_backlinks.len(), 4_999);
        assert!(elapsed.as_micros() < 2_000, "Consulta de backlinks levou {:?}", elapsed);

        Ok(())
    }
}
