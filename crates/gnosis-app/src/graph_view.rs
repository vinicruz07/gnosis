//! Controlador de visualização do Grafo de Conhecimento 2D com física e Auto-Sleep na interface.

use gnosis_graph::model::{EdgeType, KnowledgeGraph, NodeType};
use gnosis_graph::physics::{GraphPhysicsSimulator, PhysicsConfig, Point2D};
use gnosis_graph::subgraph::{GraphFilterConfig, SubgraphManager};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Metadados de um nó serializado no arquivo `.grafo`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GrafoNodeData {
    pub id: String,
    pub label: String,
    pub node_type: NodeType,
    pub x: f32,
    pub y: f32,
    pub radius: Option<f32>,
    pub color: Option<String>,
    pub is_pinned: bool,
}

/// Metadados de uma aresta serializada no arquivo `.grafo`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GrafoEdgeData {
    pub source: String,
    pub target: String,
    pub edge_type: EdgeType,
    pub label: Option<String>,
}

/// Estrutura de persistência do arquivo de Grafo (`.grafo`) no Grimório.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GrafoFile {
    pub version: u32,
    pub pan_x: f32,
    pub pan_y: f32,
    pub zoom: f32,
    pub nodes: Vec<GrafoNodeData>,
    pub edges: Vec<GrafoEdgeData>,
}

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
    pub custom_radius: HashMap<String, f32>,
    pub custom_colors: HashMap<String, String>,
    pub pan_x: f32,
    pub pan_y: f32,
    pub zoom: f32,
    pub dragging_node_id: Option<String>,
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
            custom_radius: HashMap::new(),
            custom_colors: HashMap::new(),
            pan_x: 0.0,
            pan_y: 0.0,
            zoom: 1.0,
            dragging_node_id: None,
        }
    }

    /// Localiza o nó visual sob as coordenadas do mouse (com raio de tolerância confortável para clique/toque).
    pub fn find_node_at(&self, mouse_x: f32, mouse_y: f32, screen_w: f32, screen_h: f32) -> Option<String> {
        let visual_nodes = self.get_visual_nodes(screen_w, screen_h);
        for node in visual_nodes.iter().rev() {
            let dx = mouse_x - node.x;
            let dy = mouse_y - node.y;
            let hit_radius = (node.radius + 6.0).max(16.0);
            if dx * dx + dy * dy <= hit_radius * hit_radius {
                return Some(node.id.clone());
            }
        }
        None
    }

    /// Processa o evento de mouse down no canvas: se atingir um nó, inicia o arrasto dele; caso contrário, prepara para pan.
    pub fn handle_pointer_down(&mut self, mouse_x: f32, mouse_y: f32, screen_w: f32, screen_h: f32) -> Option<String> {
        let node_id = self.find_node_at(mouse_x, mouse_y, screen_w, screen_h);
        self.dragging_node_id = node_id.clone();
        node_id
    }

    /// Processa o movimento de arrasto no canvas: se um nó estiver sendo arrastado, move o nó e fixa; se não, move a câmera (pan).
    /// Retorna `true` se um nó foi movido (necessitando atualização de nós na UI), ou `false` se foi pan.
    pub fn handle_pointer_move(&mut self, dx: f32, dy: f32) -> bool {
        if let Some(id) = self.dragging_node_id.clone() {
            self.drag_node(&id, dx, dy);
            self.wake_up();
            self.step();
            true
        } else {
            self.pan(dx, dy);
            false
        }
    }

    /// Processa o término do clique/arrasto no canvas.
    pub fn handle_pointer_up(&mut self) -> Option<String> {
        let released = self.dragging_node_id.take();
        self.wake_up();
        released
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

    /// Move um nó aplicando um deslocamento de tela (delta) e o fixa.
    pub fn drag_node(&mut self, id: &str, delta_screen_x: f32, delta_screen_y: f32) -> bool {
        let zoom = self.zoom.max(0.1);
        let world_dx = delta_screen_x / zoom;
        let world_dy = delta_screen_y / zoom;

        if let Some(current_pos) = self.simulator.get_position(id) {
            let new_pos = Point2D::new(current_pos.x + world_dx, current_pos.y + world_dy);
            self.simulator.pin_node(id, new_pos)
        } else {
            false
        }
    }

    /// Aplica deslocamento relativo (pan) na câmera do grafo.
    pub fn pan(&mut self, dx: f32, dy: f32) {
        self.pan_x += dx;
        self.pan_y += dy;
    }

    /// Aplica fator de zoom progressivo na câmera do grafo limitando entre 0.2x e 3.0x.
    pub fn zoom_by(&mut self, factor: f32) {
        self.zoom = (self.zoom * factor).clamp(0.2, 3.0);
    }

    /// Aumenta o zoom do grafo em 15%.
    pub fn zoom_in(&mut self) {
        self.zoom_by(1.15);
    }

    /// Diminui o zoom do grafo em 15%.
    pub fn zoom_out(&mut self) {
        self.zoom_by(0.85);
    }

    /// Restaura o zoom (1.0) e o pan (0, 0) da câmera do grafo.
    pub fn zoom_reset(&mut self) {
        self.zoom = 1.0;
        self.pan_x = 0.0;
        self.pan_y = 0.0;
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

    /// Adiciona um novo nó conceitual no grafo ativo e desperta a simulação.
    pub fn add_custom_node(&mut self, id: &str, label: &str, node_type: NodeType) -> bool {
        self.full_graph.add_or_update_node(id, label, node_type, None);
        self.active_graph = self
            .subgraph_manager
            .apply_filter(&self.full_graph, &self.filter_config);
        self.simulator.sync_with_graph(&self.active_graph);
        true
    }

    /// Remove um nó do grafo e todas as suas arestas.
    pub fn remove_node(&mut self, id: &str) -> bool {
        let removed = self.full_graph.remove_node(id);
        self.custom_radius.remove(id);
        self.custom_colors.remove(id);
        self.active_graph = self
            .subgraph_manager
            .apply_filter(&self.full_graph, &self.filter_config);
        self.simulator.sync_with_graph(&self.active_graph);
        removed
    }

    /// Cria uma nova aresta direcionada entre dois nós.
    pub fn connect_nodes(&mut self, source_id: &str, target_id: &str) -> bool {
        if self
            .full_graph
            .add_edge(source_id, target_id, EdgeType::WikiLink, None, 1.0)
            .is_ok()
        {
            self.active_graph = self
                .subgraph_manager
                .apply_filter(&self.full_graph, &self.filter_config);
            self.simulator.sync_with_graph(&self.active_graph);
            true
        } else {
            false
        }
    }

    /// Remove a aresta de conexão entre dois nós (em ambos os sentidos se existirem).
    pub fn disconnect_nodes(&mut self, source_id: &str, target_id: &str) -> bool {
        let r1 = self.full_graph.remove_edge(source_id, target_id);
        let r2 = self.full_graph.remove_edge(target_id, source_id);
        let removed = r1 || r2;
        if removed {
            self.active_graph = self
                .subgraph_manager
                .apply_filter(&self.full_graph, &self.filter_config);
            self.simulator.sync_with_graph(&self.active_graph);
        }
        removed
    }

    /// Altera a cor temática do disco de um nó.
    pub fn set_node_color(&mut self, id: &str, color_hex: &str) {
        self.custom_colors.insert(id.to_string(), color_hex.to_string());
    }

    /// Ajusta o raio do disco de um nó somando ou subtraindo delta.
    pub fn adjust_node_radius(&mut self, id: &str, delta: f32) {
        let current = self.custom_radius.get(id).copied().unwrap_or(12.0);
        let new_radius = (current + delta).clamp(4.0, 48.0);
        self.custom_radius.insert(id.to_string(), new_radius);
    }

    /// Define diretamente o raio de um nó.
    pub fn set_node_radius(&mut self, id: &str, radius: f32) {
        self.custom_radius.insert(id.to_string(), radius.clamp(4.0, 48.0));
    }

    /// Exporta o estado atual completo do grafo para a estrutura serializável `GrafoFile`.
    pub fn export_grafo(&self) -> GrafoFile {
        let mut nodes = Vec::new();
        for node in self.full_graph.all_nodes() {
            let pos = self
                .simulator
                .get_position(&node.id)
                .unwrap_or(Point2D::ZERO);
            nodes.push(GrafoNodeData {
                id: node.id.clone(),
                label: node.label.clone(),
                node_type: node.node_type,
                x: pos.x,
                y: pos.y,
                radius: self.custom_radius.get(&node.id).copied(),
                color: self.custom_colors.get(&node.id).cloned(),
                is_pinned: self.simulator.is_pinned(&node.id),
            });
        }

        let mut edges = Vec::new();
        for (src, tgt, edge) in self.full_graph.all_edges() {
            edges.push(GrafoEdgeData {
                source: src.to_string(),
                target: tgt.to_string(),
                edge_type: edge.edge_type,
                label: edge.label.clone(),
            });
        }

        GrafoFile {
            version: 1,
            pan_x: self.pan_x,
            pan_y: self.pan_y,
            zoom: self.zoom,
            nodes,
            edges,
        }
    }

    /// Importa um `GrafoFile` restaurando nós, arestas, posições, zoom, raios e cores.
    pub fn import_grafo(&mut self, file: GrafoFile) {
        let mut new_graph = KnowledgeGraph::new();
        for node in &file.nodes {
            new_graph.add_or_update_node(&node.id, &node.label, node.node_type, None);
        }
        for edge in &file.edges {
            let _ = new_graph.add_edge(
                &edge.source,
                &edge.target,
                edge.edge_type,
                edge.label.clone(),
                1.0,
            );
        }

        self.full_graph = new_graph;
        self.pan_x = file.pan_x;
        self.pan_y = file.pan_y;
        self.zoom = file.zoom;
        self.custom_radius.clear();
        self.custom_colors.clear();

        self.active_graph = self
            .subgraph_manager
            .apply_filter(&self.full_graph, &self.filter_config);
        self.simulator = GraphPhysicsSimulator::from_knowledge_graph(
            &self.active_graph,
            self.simulator.config().clone(),
        );

        for node in file.nodes {
            let _ = self
                .simulator
                .set_position(&node.id, Point2D::new(node.x, node.y));
            if node.is_pinned {
                let _ = self
                    .simulator
                    .pin_node(&node.id, Point2D::new(node.x, node.y));
            }
            if let Some(r) = node.radius {
                self.custom_radius.insert(node.id.clone(), r);
            }
            if let Some(c) = node.color {
                self.custom_colors.insert(node.id, c);
            }
        }
        self.wake_up();
    }

    /// Salva o grafo no disco como um arquivo `.grafo` em formato JSON formatado.
    pub fn save_to_grafo_file(&self, path: &std::path::Path) -> std::io::Result<()> {
        let file_data = self.export_grafo();
        let json_string = serde_json::to_string_pretty(&file_data)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, json_string)
    }

    /// Carrega o grafo a partir de um arquivo `.grafo` salvo no disco.
    pub fn load_from_grafo_file(&mut self, path: &std::path::Path) -> std::io::Result<()> {
        let content = std::fs::read_to_string(path)?;
        let file_data: GrafoFile = serde_json::from_str(&content)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        self.import_grafo(file_data);
        Ok(())
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
            // Raio do nó: se tiver custom_radius usa-o, senão calcula proporcionalmente às conexões
            let base_radius = (6.0 + (degree as f32).min(20.0) * 1.5) * self.zoom.clamp(0.4, 2.0);
            let radius = self
                .custom_radius
                .get(&node.id)
                .copied()
                .unwrap_or(base_radius);

            // Cor baseada no tipo de nó ou customizada pelo usuário
            let color_hex = if let Some(custom) = self.custom_colors.get(&node.id) {
                custom.clone()
            } else {
                match node.node_type {
                    NodeType::Note => "#89b4fa".to_string(),       // Azul Obsidian
                    NodeType::Tag => "#fab387".to_string(),        // Laranja
                    NodeType::Attachment => "#a6e3a1".to_string(), // Verde
                    NodeType::Unresolved => "#6c7086".to_string(), // Cinza fantasma
                }
            };

            let is_pinned = self.simulator.is_pinned(&node.id);

            visual_nodes.push(GraphNodeVisual {
                id: node.id.clone(),
                label: node.label.clone(),
                x: screen_x,
                y: screen_y,
                radius,
                color_hex,
                is_pinned,
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

    #[test]
    fn test_graph_view_node_and_edge_operations() {
        let graph = build_test_graph();
        let mut controller = GraphViewController::new(graph);

        // Adiciona um nó personalizado
        let added = controller.add_custom_node("CustomNode", "Conceito Novo", NodeType::Note);
        assert!(added);
        assert_eq!(controller.full_graph.node_count(), 5);

        // Conecta CustomNode a A
        let connected = controller.connect_nodes("CustomNode", "A");
        assert!(connected);

        // Remove nó B (deve remover arestas com A e C)
        let removed = controller.remove_node("B");
        assert!(removed);
        assert_eq!(controller.full_graph.node_count(), 4);

        // Desconecta CustomNode de A
        let disconnected = controller.disconnect_nodes("CustomNode", "A");
        assert!(disconnected);
    }

    #[test]
    fn test_graph_view_custom_styling() {
        let graph = build_test_graph();
        let mut controller = GraphViewController::new(graph);

        // Altera cor e raio do nó A
        controller.set_node_color("A", "#f38ba8");
        controller.adjust_node_radius("A", 10.0);

        let visual_nodes = controller.get_visual_nodes(400.0, 300.0);
        let node_a = visual_nodes.iter().find(|n| n.id == "A").unwrap();
        assert_eq!(node_a.color_hex, "#f38ba8");
        assert_eq!(node_a.radius, 22.0); // 12.0 default + 10.0 delta
    }

    #[test]
    fn test_graph_view_file_persistence_export_import() {
        let graph = build_test_graph();
        let mut controller = GraphViewController::new(graph);

        controller.pan(50.0, -30.0);
        controller.zoom_by(1.5);
        controller.set_node_color("A", "#a6e3a1");
        controller.set_node_radius("A", 25.0);
        controller.pin_node("A", 100.0, 150.0);

        // Exporta para GrafoFile
        let exported = controller.export_grafo();
        assert_eq!(exported.nodes.len(), 4);
        assert_eq!(exported.edges.len(), 2);
        assert_eq!(exported.pan_x, 50.0);
        assert_eq!(exported.pan_y, -30.0);
        assert_eq!(exported.zoom, 1.5);

        // Novo controlador vazio importando o arquivo
        let mut new_controller = GraphViewController::new(KnowledgeGraph::new());
        new_controller.import_grafo(exported);

        assert_eq!(new_controller.full_graph.node_count(), 4);
        assert_eq!(new_controller.pan_x, 50.0);
        assert_eq!(new_controller.zoom, 1.5);

        let visual = new_controller.get_visual_nodes(400.0, 300.0);
        let imported_a = visual.iter().find(|n| n.id == "A").unwrap();
        assert_eq!(imported_a.color_hex, "#a6e3a1");
        assert_eq!(imported_a.radius, 25.0);
    }

    #[test]
    fn test_graph_view_hit_testing_and_unified_drag() {
        let graph = build_test_graph();
        let mut controller = GraphViewController::new(graph);

        // Obter posição do nó "A"
        let nodes = controller.get_visual_nodes(400.0, 300.0);
        let node_a = nodes.iter().find(|n| n.id == "A").unwrap();
        let ax = node_a.x;
        let ay = node_a.y;

        // 1. Hit-testing: clique exatamente no nó A deve detectá-lo
        let hit = controller.find_node_at(ax, ay, 400.0, 300.0);
        assert_eq!(hit, Some("A".to_string()));

        // 2. Hit-testing: clique bem longe (ex: 10, 10) deve retornar None
        let miss = controller.find_node_at(10.0, 10.0, 400.0, 300.0);
        assert_eq!(miss, None);

        // 3. Pointer Down no nó A
        let down_target = controller.handle_pointer_down(ax, ay, 400.0, 300.0);
        assert_eq!(down_target, Some("A".to_string()));
        assert_eq!(controller.dragging_node_id, Some("A".to_string()));

        // 4. Pointer Move deve mover o nó A (retornando true para step de física)
        let moved = controller.handle_pointer_move(15.0, 20.0);
        assert!(moved);

        // 5. Pointer Up deve liberar o nó
        let released = controller.handle_pointer_up();
        assert_eq!(released, Some("A".to_string()));
        assert_eq!(controller.dragging_node_id, None);

        // 6. Pointer Down no vazio deve preparar pan (retornando None)
        let pan_target = controller.handle_pointer_down(5.0, 5.0, 400.0, 300.0);
        assert_eq!(pan_target, None);
        assert_eq!(controller.dragging_node_id, None);

        let initial_pan_x = controller.pan_x;
        let panned = controller.handle_pointer_move(10.0, 5.0);
        assert!(!panned); // false indica que foi pan de câmera
        assert_eq!(controller.pan_x, initial_pan_x + 10.0);
    }
}
