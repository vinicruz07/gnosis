//! Controlador de visualização do Grafo de Conhecimento 2D com física e Auto-Sleep na interface.

use gnosis_graph::model::{EdgeType, KnowledgeGraph, NodeType};
use gnosis_graph::physics::{GraphPhysicsSimulator, PhysicsConfig, Point2D};
use gnosis_graph::subgraph::{GraphFilterConfig, SubgraphManager};
use serde::{Deserialize, Serialize};

/// Dados de um nó do grafo para renderização na interface Slint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphNodeVisual {
    pub id: String,
    pub label: String,
    pub x: f32,
    pub y: f32,
    pub radius: f32,
    pub color_hex: String,
    pub is_pinned: bool,
    pub degree: usize,
}

/// Dados de uma aresta do grafo para renderização visual.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphEdgeVisual {
    pub source_x: f32,
    pub source_y: f32,
    pub target_x: f32,
    pub target_y: f32,
    pub color_hex: String,
}

/// Estado do motor de física do grafo na UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PhysicsStatus {
    Simulating,
    Sleeping,
}

/// Orquestrador de visualização do grafo na UI, ligando modelo, filtros e simulação física 2D.
pub struct GraphViewController {
    pub full_graph: KnowledgeGraph,
    pub active_graph: KnowledgeGraph,
    pub simulator: GraphPhysicsSimulator,
    pub subgraph_manager: SubgraphManager,
    pub filter_config: GraphFilterConfig,
    pub pan_x: f32,
    pub pan_y: f32,
    pub zoom: f32,
}

impl GraphViewController {
    /// Inicializa a visualização do grafo com a física calibrada.
    pub fn new(graph: KnowledgeGraph) -> Self {
        let subgraph_mgr = SubgraphManager::new();
        let filter = GraphFilterConfig::default();
        let filtered = subgraph_mgr.apply_filter(&graph, &filter);

        let config = PhysicsConfig {
            repulsion_constant: 12000.0,
            spring_length: 80.0,
            spring_stiffness: 0.04,
            center_gravity: 0.01,
            damping: 0.85,
            max_velocity: 12.0,
            sleep_energy_threshold: 0.05,
            time_step: 0.5,
        };

        let sim = GraphPhysicsSimulator::from_knowledge_graph(&filtered, config);

        Self {
            full_graph: graph,
            active_graph: filtered,
            simulator: sim,
            subgraph_manager: subgraph_mgr,
            filter_config: filter,
            pan_x: 0.0,
            pan_y: 0.0,
            zoom: 1.0,
        }
    }

    /// Executa um passo da simulação de física 2D.
    /// Retorna `true` se o sistema ainda estiver em movimento, ou `false` se atingiu o Auto-Sleep (0% CPU).
    pub fn step(&mut self) -> bool {
        self.simulator.step()
    }

    /// Retorna se o motor de física atingiu o repouso absoluto (0% de CPU).
    pub fn is_sleeping(&self) -> bool {
        self.simulator.is_sleeping()
    }

    /// Força o despertar da simulação física (ex: após arraste ou adição de nós).
    pub fn wake_up(&mut self) {
        self.simulator.wake_up();
    }

    /// Fixa um nó em coordenadas específicas da tela (ex: clique e arraste com o mouse).
    pub fn pin_node(&mut self, id: &str, screen_x: f32, screen_y: f32) -> bool {
        // Converte coordenadas da tela para o espaço do grafo levando em conta Pan e Zoom
        let graph_x = (screen_x - self.pan_x) / self.zoom.max(0.1);
        let graph_y = (screen_y - self.pan_y) / self.zoom.max(0.1);

        self.simulator.pin_node(id, Point2D::new(graph_x, graph_y))
    }

    /// Libera o nó fixado de volta à dinâmica física livre.
    pub fn unpin_node(&mut self, id: &str) -> bool {
        self.simulator.unpin_node(id)
    }

    /// Aplica novas regras de filtragem (ocultar órfãos, links não resolvidos, etc.) e reconstrói o simulador.
    pub fn update_filters(&mut self, filter: GraphFilterConfig) {
        self.filter_config = filter;
        self.active_graph = self
            .subgraph_manager
            .apply_filter(&self.full_graph, &self.filter_config);

        // Atualiza a física preservando posições dos nós persistentes
        let old_positions: std::collections::HashMap<String, Point2D> = self
            .simulator
            .get_all_positions()
            .into_iter()
            .map(|(id, pos)| (id.to_string(), pos))
            .collect();

        self.simulator = GraphPhysicsSimulator::from_knowledge_graph(
            &self.active_graph,
            self.simulator.config().clone(),
        );

        for (id, pos) in old_positions {
            let _ = self.simulator.set_position(&id, pos);
        }

        self.wake_up();
    }

    /// Gera a lista de nós visuais transformados para exibição no Slint.
    pub fn get_visual_nodes(&self, center_offset_x: f32, center_offset_y: f32) -> Vec<GraphNodeVisual> {
        let mut visual_nodes = Vec::new();

        for node in self.active_graph.all_nodes() {
            let pos = self
                .simulator
                .get_position(&node.id)
                .unwrap_or(Point2D::ZERO);

            // Transforma coordenadas mundiais para coordenadas de tela centralizadas
            let screen_x = center_offset_x + self.pan_x + pos.x * self.zoom;
            let screen_y = center_offset_y + self.pan_y + pos.y * self.zoom;

            let degree = self.active_graph.node_degree(&node.id);
            // Raio do nó cresce logaritmicamente com o número de conexões
            let radius = (6.0 + (degree as f32).min(20.0) * 1.5) * self.zoom.clamp(0.4, 2.0);

            // Cor baseada no tipo de nó
            let color_hex = match node.node_type {
                NodeType::Note => "#89b4fa".to_string(),       // Azul Obsidian
                NodeType::Tag => "#fab387".to_string(),        // Laranja
                NodeType::Attachment => "#a6e3a1".to_string(), // Verde
                NodeType::Unresolved => "#6c7086".to_string(), // Cinza fantasma
            };

            visual_nodes.push(GraphNodeVisual {
                id: node.id.clone(),
                label: node.label.clone(),
                x: screen_x,
                y: screen_y,
                radius,
                color_hex,
                is_pinned: false,
                degree,
            });
        }

        visual_nodes
    }

    /// Gera a lista de arestas visuais com coordenadas na tela.
    pub fn get_visual_edges(
        &self,
        center_offset_x: f32,
        center_offset_y: f32,
    ) -> Vec<GraphEdgeVisual> {
        let mut visual_edges = Vec::new();

        for (src_id, tgt_id, edge) in self.active_graph.all_edges() {
            let p1 = match self.simulator.get_position(src_id) {
                Some(p) => p,
                None => continue,
            };
            let p2 = match self.simulator.get_position(tgt_id) {
                Some(p) => p,
                None => continue,
            };

            let src_x = center_offset_x + self.pan_x + p1.x * self.zoom;
            let src_y = center_offset_y + self.pan_y + p1.y * self.zoom;
            let tgt_x = center_offset_x + self.pan_x + p2.x * self.zoom;
            let tgt_y = center_offset_y + self.pan_y + p2.y * self.zoom;

            let color_hex = match edge.edge_type {
                EdgeType::WikiLink => "#45475a".to_string(),
                EdgeType::TagRelation => "#585b70".to_string(),
                EdgeType::Embed => "#89b4fa".to_string(),
            };

            visual_edges.push(GraphEdgeVisual {
                source_x: src_x,
                source_y: src_y,
                target_x: tgt_x,
                target_y: tgt_y,
                color_hex,
            });
        }

        visual_edges
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_test_graph() -> KnowledgeGraph {
        let mut g = KnowledgeGraph::new();
        g.add_or_update_node("A", "Nota A", NodeType::Note, None);
        g.add_or_update_node("B", "Nota B", NodeType::Note, None);
        g.add_or_update_node("C", "Nota C", NodeType::Note, None);
        g.add_or_update_node("Orphan", "Isolada", NodeType::Note, None);

        let _ = g.add_edge("A", "B", EdgeType::WikiLink, None, 1.0);
        let _ = g.add_edge("B", "C", EdgeType::WikiLink, None, 1.0);
        g
    }

    #[test]
    fn test_graph_view_controller_simulation_and_auto_sleep() {
        let graph = build_test_graph();
        let mut controller = GraphViewController::new(graph);

        assert!(!controller.is_sleeping());

        // Roda a física até atingir equilíbrio e entrar em repouso
        let mut steps = 0;
        while steps < 300 && !controller.is_sleeping() {
            controller.step();
            steps += 1;
        }

        assert!(controller.is_sleeping(), "Deveria ter entrado em Auto-Sleep após {} passos", steps);

        // Acordar e fixar nó com mouse
        let pinned = controller.pin_node("A", 150.0, 200.0);
        assert!(pinned);
        assert!(!controller.is_sleeping());

        controller.unpin_node("A");
    }

    #[test]
    fn test_graph_view_filters_and_visual_mapping() {
        let graph = build_test_graph();
        let mut controller = GraphViewController::new(graph);

        let visual_nodes_before = controller.get_visual_nodes(400.0, 300.0);
        assert_eq!(visual_nodes_before.len(), 4);

        // Aplica filtro para ocultar notas órfãs
        let filter = GraphFilterConfig {
            hide_orphans: true,
            ..Default::default()
        };
        controller.update_filters(filter);

        let visual_nodes_after = controller.get_visual_nodes(400.0, 300.0);
        assert_eq!(visual_nodes_after.len(), 3);
        assert!(!visual_nodes_after.iter().any(|n| n.id == "Orphan"));

        let edges = controller.get_visual_edges(400.0, 300.0);
        assert_eq!(edges.len(), 2);
    }
}
