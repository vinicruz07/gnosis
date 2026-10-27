//! Controlador de visualização e manipulação do Canvas 2D infinito no Slint.
//!
//! Fornece suporte a:
//! - Câmera virtual 2D: Pan suave e Zoom progressivo com projeção focal.
//! - Enquadramento automático (`fit_all`) baseado na bounding box dos nós.
//! - Manipulação direta: seleção, movimentação de nós por arraste, adição de cartões e criação de conexões.
//! - Conversão pura para modelos de dados de renderização do Slint.
//!
//! Diretrizes de Qualidade:
//! - 100% Rust Seguro (`#![deny(unsafe_code)]`).
//! - Zero pânicos em tempo de execução (`clippy::unwrap_used`, `clippy::expect_used`).

use std::collections::HashMap;
use gnosis_core::canvas::{
    CanvasData, CanvasEdge, CanvasNode, EdgeEnd, FileNode, GroupNode, LinkNode, NodeSide, TextNode,
};
use serde::{Deserialize, Serialize};

/// Dados visuais projetados de um nó para consumo pela interface.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanvasNodeVisual {
    pub id: String,
    pub node_type: String,
    pub screen_x: f32,
    pub screen_y: f32,
    pub screen_width: f32,
    pub screen_height: f32,
    pub color_hex: String,
    pub title: String,
    pub content: String,
    pub subpath: String,
    pub is_selected: bool,
}

/// Dados visuais projetados de uma aresta/conexão entre dois nós.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanvasEdgeVisual {
    pub id: String,
    pub from_node: String,
    pub from_x: f32,
    pub from_y: f32,
    pub to_node: String,
    pub to_x: f32,
    pub to_y: f32,
    pub color_hex: String,
    pub label: String,
    pub has_arrow: bool,
}

/// Orquestrador de visualização do Canvas 2D infinito.
pub struct CanvasViewController {
    pub canvas_data: CanvasData,
    pub camera_x: f32,
    pub camera_y: f32,
    pub zoom: f32,
    pub selected_node_id: Option<String>,
}

impl CanvasViewController {
    /// Inicializa o controlador com um canvas vazio ou existente.
    pub fn new(data: CanvasData) -> Self {
        Self {
            canvas_data: data,
            camera_x: 200.0,
            camera_y: 150.0,
            zoom: 1.0,
            selected_node_id: None,
        }
    }

    /// Cria um canvas com nós de demonstração introdutórios se vazio.
    pub fn with_default_demo() -> Self {
        let mut data = CanvasData::new();

        data.add_node(CanvasNode::Group(GroupNode {
            id: "group_intro".to_string(),
            x: -40,
            y: -40,
            width: 720,
            height: 380,
            label: Some("Mapa Mental do Projeto".to_string()),
            background: None,
            background_style: None,
            color: Some("4".to_string()),
            extra: HashMap::new(),
        }));

        data.add_node(CanvasNode::Text(TextNode {
            id: "node_welcome".to_string(),
            x: 0,
            y: 0,
            width: 280,
            height: 140,
            text: "# Bem-vindo ao Canvas!\nOrganize ideias visualmente com conexões espaciais e notas incorporadas.".to_string(),
            color: Some("1".to_string()),
            extra: HashMap::new(),
        }));

        data.add_node(CanvasNode::File(FileNode {
            id: "node_note_arch".to_string(),
            x: 360,
            y: 40,
            width: 280,
            height: 180,
            file: "Notas/Arquitetura.md".to_string(),
            subpath: Some("#Visao-Geral".to_string()),
            color: Some("2".to_string()),
            extra: HashMap::new(),
        }));

        data.add_edge(CanvasEdge {
            id: "edge_demo".to_string(),
            from_node: "node_welcome".to_string(),
            from_side: Some(NodeSide::Right),
            from_end: Some(EdgeEnd::None),
            to_node: "node_note_arch".to_string(),
            to_side: Some(NodeSide::Left),
            to_end: Some(EdgeEnd::Arrow),
            color: Some("#89b4fa".to_string()),
            label: Some("conduz a".to_string()),
            extra: HashMap::new(),
        });

        Self::new(data)
    }

    /// Desloca a câmera 2D (pan).
    pub fn pan(&mut self, dx: f32, dy: f32) {
        self.camera_x += dx;
        self.camera_y += dy;
    }

    /// Aplica zoom progressivo limitando os limites entre 0.2 e 3.0.
    pub fn zoom_by(&mut self, factor: f32) {
        let new_zoom = (self.zoom * factor).clamp(0.2, 3.0);
        self.zoom = new_zoom;
    }

    /// Aumenta o zoom em 15%.
    pub fn zoom_in(&mut self) {
        self.zoom_by(1.15);
    }

    /// Diminui o zoom em 15%.
    pub fn zoom_out(&mut self) {
        self.zoom_by(0.85);
    }

    /// Restaura a escala de zoom para 100% (1.0).
    pub fn zoom_reset(&mut self) {
        self.zoom = 1.0;
    }

    /// Enquadra automaticamente todos os nós na viewport informada.
    pub fn fit_all(&mut self, viewport_width: f32, viewport_height: f32) {
        if let Some((min_x, min_y, max_x, max_y)) = self.canvas_data.bounding_box() {
            let width = (max_x - min_x) as f32;
            let height = (max_y - min_y) as f32;

            if width > 10.0 && height > 10.0 && viewport_width > 50.0 && viewport_height > 50.0 {
                let padding = 60.0;
                let scale_x = (viewport_width - padding * 2.0) / width;
                let scale_y = (viewport_height - padding * 2.0) / height;
                let fit_zoom = scale_x.min(scale_y).clamp(0.2, 2.0);

                let center_world_x = (min_x + max_x) as f32 / 2.0;
                let center_world_y = (min_y + max_y) as f32 / 2.0;

                self.zoom = fit_zoom;
                self.camera_x = (viewport_width / 2.0) - (center_world_x * fit_zoom);
                self.camera_y = (viewport_height / 2.0) - (center_world_y * fit_zoom);
            }
        }
    }

    /// Define o nó selecionado atualmente.
    pub fn select_node(&mut self, id: &str) {
        if id.is_empty() {
            self.selected_node_id = None;
        } else {
            self.selected_node_id = Some(id.to_string());
        }
    }

    /// Move um nó com base no deslocamento de tela (delta em pixels de tela convertido para escala do mundo).
    pub fn move_node(&mut self, id: &str, delta_screen_x: f32, delta_screen_y: f32) {
        let zoom = if self.zoom > 0.01 { self.zoom } else { 1.0 };
        let world_dx = (delta_screen_x / zoom) as i32;
        let world_dy = (delta_screen_y / zoom) as i32;

        if let Some(node) = self.canvas_data.get_node_mut(id) {
            let current_x = node.x();
            let current_y = node.y();
            node.set_position(current_x + world_dx, current_y + world_dy);
        }
    }

    /// Adiciona um novo cartão de texto no centro da visão atual.
    pub fn add_text_card(&mut self, text: &str) -> String {
        let count = self.canvas_data.node_count() + 1;
        let id = format!("text_{}", count);

        // Converte o centro aproximado da tela para o espaço do mundo
        let world_x = ((-self.camera_x + 300.0) / self.zoom) as i32;
        let world_y = ((-self.camera_y + 200.0) / self.zoom) as i32;

        self.canvas_data.add_node(CanvasNode::Text(TextNode {
            id: id.clone(),
            x: world_x,
            y: world_y,
            width: 260,
            height: 120,
            text: text.to_string(),
            color: None,
            extra: HashMap::new(),
        }));

        self.selected_node_id = Some(id.clone());
        id
    }

    /// Adiciona um novo cartão de nota no centro da visão atual.
    pub fn add_file_card(&mut self, file_path: &str) -> String {
        let count = self.canvas_data.node_count() + 1;
        let id = format!("file_{}", count);

        let world_x = ((-self.camera_x + 320.0) / self.zoom) as i32;
        let world_y = ((-self.camera_y + 220.0) / self.zoom) as i32;

        self.canvas_data.add_node(CanvasNode::File(FileNode {
            id: id.clone(),
            x: world_x,
            y: world_y,
            width: 280,
            height: 160,
            file: file_path.to_string(),
            subpath: None,
            color: None,
            extra: HashMap::new(),
        }));

        self.selected_node_id = Some(id.clone());
        id
    }

    /// Conecta dois nós através de uma aresta direcionada.
    pub fn connect_nodes(&mut self, from_id: &str, to_id: &str, label: Option<String>) -> String {
        let edge_id = format!("edge_{}_{}", from_id, to_id);
        let mut edge = CanvasEdge::new(&edge_id, from_id, to_id);
        edge.label = label;
        self.canvas_data.add_edge(edge);
        edge_id
    }

    /// Exclui o nó selecionado atualmente (com remoção de arestas em cascata).
    pub fn delete_selected(&mut self) -> Option<String> {
        if let Some(id) = self.selected_node_id.take() {
            self.canvas_data.remove_node(&id);
            Some(id)
        } else {
            None
        }
    }

    /// Converte os nós e arestas do domínio puro para modelos visuais prontos para renderização no Slint.
    pub fn to_visual_models(&self) -> (Vec<CanvasNodeVisual>, Vec<CanvasEdgeVisual>) {
        let mut visual_nodes = Vec::with_capacity(self.canvas_data.nodes.len());
        let mut node_positions = HashMap::new();

        for node in &self.canvas_data.nodes {
            let screen_x = (node.x() as f32 * self.zoom) + self.camera_x;
            let screen_y = (node.y() as f32 * self.zoom) + self.camera_y;
            let screen_w = node.width() as f32 * self.zoom;
            let screen_h = node.height() as f32 * self.zoom;

            let is_sel = self.selected_node_id.as_deref() == Some(node.id());

            let (title, content, subpath) = match node {
                CanvasNode::Text(t) => (String::new(), t.text.clone(), String::new()),
                CanvasNode::File(f) => {
                    let title = f.file.clone();
                    let sub = f.subpath.clone().unwrap_or_default();
                    (title, String::new(), sub)
                }
                CanvasNode::Link(l) => ("Link".to_string(), l.url.clone(), String::new()),
                CanvasNode::Group(g) => (g.label.clone().unwrap_or_default(), String::new(), String::new()),
            };

            let color_hex = node.color().unwrap_or("").to_string();

            // Salva posição central e laterais para ancoragem de arestas
            node_positions.insert(
                node.id().to_string(),
                (screen_x, screen_y, screen_w, screen_h),
            );

            visual_nodes.push(CanvasNodeVisual {
                id: node.id().to_string(),
                node_type: node.node_type().to_string(),
                screen_x,
                screen_y,
                screen_width: screen_w,
                screen_height: screen_h,
                color_hex,
                title,
                content,
                subpath,
                is_selected: is_sel,
            });
        }

        let mut visual_edges = Vec::with_capacity(self.canvas_data.edges.len());

        for edge in &self.canvas_data.edges {
            let from_pos = node_positions.get(&edge.from_node);
            let to_pos = node_positions.get(&edge.to_node);

            if let (Some(&(fx, fy, fw, fh)), Some(&(tx, ty, tw, th))) = (from_pos, to_pos) {
                // Ponto de saída do nó de origem
                let (from_x, from_y) = match edge.from_side {
                    Some(NodeSide::Top) => (fx + fw / 2.0, fy),
                    Some(NodeSide::Bottom) => (fx + fw / 2.0, fy + fh),
                    Some(NodeSide::Left) => (fx, fy + fh / 2.0),
                    Some(NodeSide::Right) => (fx + fw, fy + fh / 2.0),
                    None => (fx + fw, fy + fh / 2.0),
                };

                // Ponto de entrada do nó de destino
                let (to_x, to_y) = match edge.to_side {
                    Some(NodeSide::Top) => (tx + tw / 2.0, ty),
                    Some(NodeSide::Bottom) => (tx + tw / 2.0, ty + th),
                    Some(NodeSide::Left) => (tx, ty + th / 2.0),
                    Some(NodeSide::Right) => (tx + tw, ty + th / 2.0),
                    None => (tx, ty + th / 2.0),
                };

                visual_edges.push(CanvasEdgeVisual {
                    id: edge.id.clone(),
                    from_node: edge.from_node.clone(),
                    from_x,
                    from_y,
                    to_node: edge.to_node.clone(),
                    to_x,
                    to_y,
                    color_hex: edge.color.clone().unwrap_or_default(),
                    label: edge.label.clone().unwrap_or_default(),
                    has_arrow: edge.to_end == Some(EdgeEnd::Arrow) || edge.to_end.is_none(),
                });
            }
        }

        (visual_nodes, visual_edges)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canvas_view_initialization_and_pan_zoom() {
        let mut ctrl = CanvasViewController::with_default_demo();
        assert_eq!(ctrl.canvas_data.node_count(), 3);
        assert_eq!(ctrl.canvas_data.edge_count(), 1);

        // Pan
        let initial_cam_x = ctrl.camera_x;
        ctrl.pan(50.0, -30.0);
        assert_eq!(ctrl.camera_x, initial_cam_x + 50.0);

        // Zoom
        ctrl.zoom_in();
        assert!(ctrl.zoom > 1.0);
        ctrl.zoom_reset();
        assert_eq!(ctrl.zoom, 1.0);
    }

    #[test]
    fn test_add_and_delete_cards() {
        let mut ctrl = CanvasViewController::new(CanvasData::new());
        assert_eq!(ctrl.canvas_data.node_count(), 0);

        let t_id = ctrl.add_text_card("Texto de teste");
        assert_eq!(ctrl.canvas_data.node_count(), 1);
        assert_eq!(ctrl.selected_node_id, Some(t_id.clone()));

        let f_id = ctrl.add_file_card("Notas/Exemplo.md");
        assert_eq!(ctrl.canvas_data.node_count(), 2);

        ctrl.connect_nodes(&t_id, &f_id, Some("link".to_string()));
        assert_eq!(ctrl.canvas_data.edge_count(), 1);

        // Excluir nó selecionado deve remover em cascata a aresta
        ctrl.select_node(&t_id);
        let deleted = ctrl.delete_selected();
        assert_eq!(deleted, Some(t_id));
        assert_eq!(ctrl.canvas_data.node_count(), 1);
        assert_eq!(ctrl.canvas_data.edge_count(), 0);
    }

    #[test]
    fn test_visual_model_projection() {
        let mut ctrl = CanvasViewController::with_default_demo();
        let (nodes, edges) = ctrl.to_visual_models();

        assert_eq!(nodes.len(), 3);
        assert_eq!(edges.len(), 1);

        let intro_group = nodes.iter().find(|n| n.id == "group_intro");
        assert!(intro_group.is_some());
        if let Some(g) = intro_group {
            assert_eq!(g.node_type, "group");
            assert_eq!(g.title, "Mapa Mental do Projeto");
        }

        let edge = &edges[0];
        assert_eq!(edge.from_node, "node_welcome");
        assert_eq!(edge.to_node, "node_note_arch");
        assert!(edge.has_arrow);
    }
}
