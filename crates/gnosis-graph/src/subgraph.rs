//! Extração de subgrafo local de vizinhança (raio K) e filtros dinâmicos de nós/arestas.
//!
//! Recursos:
//! - **Local Graph (Raio K)**: Algoritmo de Busca em Largura (BFS) centrado na nota ativa,
//!   navegando por backlinks e conexões diretas até a profundidade K configurada.
//! - **Filtros Dinâmicos**: Ocultar notas órfãs, links não resolvidos (fantasmas), anexos,
//!   e filtrar nós por tags específicas ou prefixo de caminho/pasta.
//! - **Agrupamento Temático**: Atribuição de grupos ou paletas de cores para renderização.

use crate::model::{KnowledgeGraph, NodeType};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

/// Configuração de filtros dinâmicos para a visualização do grafo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct GraphFilterConfig {
    /// Oculta nós órfãos (nós sem nenhuma aresta de entrada ou saída).
    pub hide_orphans: bool,
    /// Oculta notas fantasmas/não resolvidas (links para notas que ainda não foram criadas).
    pub hide_unresolved: bool,
    /// Oculta arquivos de mídia e anexos do grafo.
    pub hide_attachments: bool,
    /// Conjunto de tags permitidas (se preenchido, exibe apenas nós associados a pelo menos uma dessas tags).
    pub include_tags: Option<HashSet<String>>,
    /// Conjunto de tags excluídas da visualização.
    pub exclude_tags: Option<HashSet<String>>,
    /// Filtra nós cujo caminho/ID comece com este prefixo de pasta (ex: "Projetos/").
    pub path_prefix: Option<String>,
}

/// Extrator e gerenciador de visualizações parciais do grafo.
#[derive(Debug, Default, Clone)]
pub struct SubgraphManager;

impl SubgraphManager {
    /// Cria uma nova instância do gerenciador de subgrafos.
    pub fn new() -> Self {
        Self
    }

    /// Extrai o **Grafo Local** (*Local Graph*) centrado em `center_id` até a profundidade `max_depth` (raio K).
    ///
    /// Navega de forma bidirecional (backlinks e conexões diretas) usando BFS (Breadth-First Search).
    /// Se `center_id` não existir no grafo, retorna um grafo vazio.
    pub fn extract_local(
        &self,
        graph: &KnowledgeGraph,
        center_id: &str,
        max_depth: usize,
    ) -> KnowledgeGraph {
        let mut sub = KnowledgeGraph::new();

        if graph.get_node(center_id).is_none() {
            return sub;
        }

        // Fila de BFS: (node_id, profundidade atual)
        let mut queue: VecDeque<(String, usize)> = VecDeque::new();
        let mut visited: HashSet<String> = HashSet::new();

        queue.push_back((center_id.to_string(), 0));
        visited.insert(center_id.to_string());

        while let Some((curr_id, depth)) = queue.pop_front() {
            if let Some(node) = graph.get_node(&curr_id) {
                sub.add_or_update_node(
                    &node.id,
                    &node.label,
                    node.node_type.clone(),
                    node.path.clone(),
                );
            }

            if depth < max_depth {
                // Vizinhos de saída (forward links)
                for fwd in graph.forward_links(&curr_id) {
                    if visited.insert(fwd.id.clone()) {
                        queue.push_back((fwd.id.clone(), depth + 1));
                    }
                }

                // Vizinhos de entrada (backlinks)
                for bwd in graph.backlinks(&curr_id) {
                    if visited.insert(bwd.id.clone()) {
                        queue.push_back((bwd.id.clone(), depth + 1));
                    }
                }
            }
        }

        // Reconecta as arestas induzidas entre os nós que entraram no subgrafo
        for (source_id, target_id, edge) in graph.all_edges() {
            if visited.contains(source_id) && visited.contains(target_id) {
                let _ = sub.add_edge(
                    source_id,
                    target_id,
                    edge.edge_type.clone(),
                    edge.label.clone(),
                    edge.weight,
                );
            }
        }

        sub
    }

    /// Aplica as regras de filtragem especificadas em `GraphFilterConfig` sobre o grafo original,
    /// gerando um novo `KnowledgeGraph` com os nós e arestas filtrados.
    pub fn apply_filter(
        &self,
        graph: &KnowledgeGraph,
        config: &GraphFilterConfig,
    ) -> KnowledgeGraph {
        let mut filtered = KnowledgeGraph::new();

        let orphans: HashSet<String> = if config.hide_orphans {
            graph.orphan_notes().into_iter().map(|n| n.id.clone()).collect()
        } else {
            HashSet::new()
        };

        // Identifica quais nós atendem aos critérios de filtragem
        let mut valid_node_ids: HashSet<String> = HashSet::new();

        for node in graph.all_nodes() {
            // Filtro: nós órfãos
            if config.hide_orphans && orphans.contains(&node.id) {
                continue;
            }

            // Filtro: nós não resolvidos (fantasmas)
            if config.hide_unresolved && node.node_type == NodeType::Unresolved {
                continue;
            }

            // Filtro: anexos
            if config.hide_attachments && node.node_type == NodeType::Attachment {
                continue;
            }

            // Filtro: prefixo de caminho / pasta
            if let Some(prefix) = &config.path_prefix {
                if let Some(path) = &node.path {
                    if !path.as_str().starts_with(prefix) {
                        continue;
                    }
                } else if !node.id.starts_with(prefix) {
                    continue;
                }
            }

            // Filtro: inclusão de tags
            if let Some(include_tags) = &config.include_tags {
                if node.node_type == NodeType::Tag {
                    let tag_name = node.id.trim_start_matches('#');
                    if !include_tags.contains(tag_name) && !include_tags.contains(&node.id) {
                        continue;
                    }
                } else {
                    // Para notas comuns, verifica se está conectada a alguma das tags incluídas
                    let connected_to_tag = graph.forward_links(&node.id).iter().any(|neighbor| {
                        neighbor.node_type == NodeType::Tag
                            && (include_tags.contains(&neighbor.id)
                                || include_tags.contains(neighbor.id.trim_start_matches('#')))
                    });
                    if !connected_to_tag {
                        continue;
                    }
                }
            }

            // Filtro: exclusão de tags
            if let Some(exclude_tags) = &config.exclude_tags {
                if node.node_type == NodeType::Tag {
                    let tag_name = node.id.trim_start_matches('#');
                    if exclude_tags.contains(tag_name) || exclude_tags.contains(&node.id) {
                        continue;
                    }
                } else {
                    let has_excluded_tag = graph.forward_links(&node.id).iter().any(|neighbor| {
                        neighbor.node_type == NodeType::Tag
                            && (exclude_tags.contains(&neighbor.id)
                                || exclude_tags.contains(neighbor.id.trim_start_matches('#')))
                    });
                    if has_excluded_tag {
                        continue;
                    }
                }
            }

            valid_node_ids.insert(node.id.clone());
            filtered.add_or_update_node(
                &node.id,
                &node.label,
                node.node_type.clone(),
                node.path.clone(),
            );
        }

        // Reconecta as arestas entre os nós filtrados válidos
        for (source_id, target_id, edge) in graph.all_edges() {
            if valid_node_ids.contains(source_id) && valid_node_ids.contains(target_id) {
                let _ = filtered.add_edge(
                    source_id,
                    target_id,
                    edge.edge_type.clone(),
                    edge.label.clone(),
                    edge.weight,
                );
            }
        }

        filtered
    }

    /// Atribui identificadores de cluster numérico para cada nó com base em sua tag dominante ou pasta raiz.
    ///
    /// Útil para a interface Slint aplicar esquemas de cores diferenciados por grupo temático.
    pub fn assign_color_groups(&self, graph: &KnowledgeGraph) -> HashMap<String, u32> {
        let mut groups = HashMap::new();
        let mut tag_group_map: HashMap<String, u32> = HashMap::new();
        let mut next_group_id: u32 = 1;

        for node in graph.all_nodes() {
            // Se for tag, ganha um grupo próprio
            if node.node_type == NodeType::Tag {
                let gid = *tag_group_map.entry(node.id.clone()).or_insert_with(|| {
                    let id = next_group_id;
                    next_group_id = next_group_id.saturating_add(1);
                    id
                });
                groups.insert(node.id.clone(), gid);
                continue;
            }

            // Se for nota, verifica se tem uma tag conectada de saída
            let primary_tag = graph
                .forward_links(&node.id)
                .into_iter()
                .find(|n| n.node_type == NodeType::Tag);

            if let Some(tag_node) = primary_tag {
                let gid = *tag_group_map.entry(tag_node.id.clone()).or_insert_with(|| {
                    let id = next_group_id;
                    next_group_id = next_group_id.saturating_add(1);
                    id
                });
                groups.insert(node.id.clone(), gid);
            } else if let Some(path) = &node.path {
                // Agrupa pela primeira pasta do caminho se houver
                let folder = path.as_str().split('/').next().unwrap_or("");
                if !folder.is_empty() {
                    let gid = *tag_group_map.entry(folder.to_string()).or_insert_with(|| {
                        let id = next_group_id;
                        next_group_id = next_group_id.saturating_add(1);
                        id
                    });
                    groups.insert(node.id.clone(), gid);
                } else {
                    groups.insert(node.id.clone(), 0);
                }
            } else {
                // Grupo padrão 0
                groups.insert(node.id.clone(), 0);
            }
        }

        groups
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::EdgeType;
    use gnosis_core::VaultPath;

    fn build_sample_graph() -> KnowledgeGraph {
        let mut graph = KnowledgeGraph::new();

        // Nós: A -> B -> C -> D
        graph.add_or_update_node("A", "Nota A", NodeType::Note, VaultPath::parse("Inbox/A.md").ok());
        graph.add_or_update_node("B", "Nota B", NodeType::Note, VaultPath::parse("Projetos/B.md").ok());
        graph.add_or_update_node("C", "Nota C", NodeType::Note, VaultPath::parse("Projetos/C.md").ok());
        graph.add_or_update_node("D", "Nota D", NodeType::Note, VaultPath::parse("Arquivo/D.md").ok());
        graph.add_or_update_node("Ghost", "Fantasma", NodeType::Unresolved, None);
        graph.add_or_update_node("Orphan", "Isolada", NodeType::Note, None);
        graph.add_or_update_node("Image.png", "Anexo", NodeType::Attachment, None);
        graph.add_or_update_node("#rust", "Rust Tag", NodeType::Tag, None);

        // Arestas
        let _ = graph.add_edge("A", "B", EdgeType::WikiLink, None, 1.0);
        let _ = graph.add_edge("B", "C", EdgeType::WikiLink, None, 1.0);
        let _ = graph.add_edge("C", "D", EdgeType::WikiLink, None, 1.0);
        let _ = graph.add_edge("A", "Ghost", EdgeType::WikiLink, None, 1.0);
        let _ = graph.add_edge("B", "#rust", EdgeType::TagRelation, None, 1.0);
        let _ = graph.add_edge("C", "Image.png", EdgeType::Embed, None, 1.0);

        graph
    }

    #[test]
    fn test_extract_local_subgraph_depth_1_and_2() {
        let graph = build_sample_graph();
        let manager = SubgraphManager::new();

        // Raio K = 1 centrado em B: deve pegar A (backlink), C (forward) e #rust (forward)
        let sub1 = manager.extract_local(&graph, "B", 1);
        assert_eq!(sub1.node_count(), 4);
        assert!(sub1.get_node("B").is_some());
        assert!(sub1.get_node("A").is_some());
        assert!(sub1.get_node("C").is_some());
        assert!(sub1.get_node("#rust").is_some());
        assert!(sub1.get_node("D").is_none()); // D está a 2 passos de B

        // Raio K = 2 centrado em B: deve agora alcançar D e Ghost (via A) e Image.png (via C)
        let sub2 = manager.extract_local(&graph, "B", 2);
        assert!(sub2.get_node("D").is_some());
        assert!(sub2.get_node("Ghost").is_some());
        assert!(sub2.get_node("Image.png").is_some());
        assert!(sub2.get_node("Orphan").is_none()); // Órfão nunca é alcançado
    }

    #[test]
    fn test_filtering_orphans_unresolved_attachments() {
        let graph = build_sample_graph();
        let manager = SubgraphManager::new();

        let config = GraphFilterConfig {
            hide_orphans: true,
            hide_unresolved: true,
            hide_attachments: true,
            ..Default::default()
        };

        let filtered = manager.apply_filter(&graph, &config);

        assert!(filtered.get_node("Orphan").is_none());
        assert!(filtered.get_node("Ghost").is_none());
        assert!(filtered.get_node("Image.png").is_none());
        assert!(filtered.get_node("A").is_some());
        assert!(filtered.get_node("B").is_some());
        assert!(filtered.get_node("C").is_some());
    }

    #[test]
    fn test_filter_by_path_prefix() {
        let graph = build_sample_graph();
        let manager = SubgraphManager::new();

        let config = GraphFilterConfig {
            path_prefix: Some("Projetos/".to_string()),
            ..Default::default()
        };

        let filtered = manager.apply_filter(&graph, &config);

        // Apenas B e C têm prefixo "Projetos/"
        assert!(filtered.get_node("B").is_some());
        assert!(filtered.get_node("C").is_some());
        assert!(filtered.get_node("A").is_none());
        assert!(filtered.get_node("D").is_none());
    }

    #[test]
    fn test_color_groups_assignment() {
        let graph = build_sample_graph();
        let manager = SubgraphManager::new();

        let groups = manager.assign_color_groups(&graph);

        // B está conectada a #rust, então B deve ter o mesmo grupo de #rust
        let rust_group = groups.get("#rust");
        let b_group = groups.get("B");
        assert!(rust_group.is_some());
        assert_eq!(rust_group, b_group);
    }
}
