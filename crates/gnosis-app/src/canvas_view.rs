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
use std::fs;
use std::path::{Path, PathBuf};
use gnosis_core::canvas::{
    CanvasData, CanvasEdge, CanvasNode, EdgeEnd, FileNode, GroupNode, NodeSide, TextNode,
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
    pub is_editing: bool,
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

/// Orquestrador de visualização e persistência do Canvas/Membrana 2D infinito.
pub struct CanvasViewController {
    pub canvas_data: CanvasData,
    pub camera_x: f32,
    pub camera_y: f32,
    pub zoom: f32,
    pub selected_node_id: Option<String>,
    pub editing_node_id: Option<String>,
    pub active_file_path: Option<PathBuf>,
    pub is_modified: bool,
    pub drag_subpixel_x: f32,
    pub drag_subpixel_y: f32,
    pub dragging_node_id: Option<String>,
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
            editing_node_id: None,
            active_file_path: None,
            is_modified: false,
            drag_subpixel_x: 0.0,
            drag_subpixel_y: 0.0,
            dragging_node_id: None,
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

    /// Carrega dados de membrana a partir de um arquivo no disco (.canvas ou .membrana).
    pub fn load_from_file(&mut self, path: &Path) -> Result<(), String> {
        if !path.exists() {
            // Se o arquivo ainda não existir, inicializa canvas limpo com o path associado
            self.canvas_data = CanvasData::new();
            self.active_file_path = Some(path.to_path_buf());
            self.is_modified = false;
            return Ok(());
        }

        let raw = fs::read_to_string(path)
            .map_err(|e| format!("Falha ao ler arquivo de membrana {}: {}", path.display(), e))?;

        if raw.trim().is_empty() {
            self.canvas_data = CanvasData::new();
        } else {
            self.canvas_data = CanvasData::from_json(&raw)
                .map_err(|e| format!("Falha no parse do arquivo de membrana {}: {}", path.display(), e))?;
        }

        self.active_file_path = Some(path.to_path_buf());
        self.is_modified = false;
        Ok(())
    }

    /// Carrega dados de membrana a partir de uma string JSON.
    pub fn load_from_json(&mut self, json_str: &str) -> Result<(), String> {
        if json_str.trim().is_empty() {
            self.canvas_data = CanvasData::new();
        } else {
            self.canvas_data = CanvasData::from_json(json_str)
                .map_err(|e| format!("Falha no parse do JSON da membrana: {}", e))?;
        }
        self.is_modified = false;
        Ok(())
    }

    /// Salva os dados da membrana no disco atomicamente preservando rigorosamente o caminho e a extensão original.
    pub fn save_to_disk(&mut self) -> Result<(), String> {
        let Some(ref target_path) = self.active_file_path else {
            return Ok(());
        };

        let json = self.canvas_data.to_json_pretty()
            .map_err(|e| format!("Falha ao serializar membrana para JSON: {}", e))?;

        let parent = target_path.parent().unwrap_or_else(|| Path::new("."));
        if !parent.exists() {
            let _ = fs::create_dir_all(parent);
        }

        let temp_filename = format!(
            ".{}_{}.tmp",
            target_path.file_name().and_then(|n| n.to_str()).unwrap_or("membrane"),
            std::process::id()
        );
        let temp_path = parent.join(temp_filename);

        if let Err(e) = fs::write(&temp_path, &json) {
            let _ = fs::remove_file(&temp_path);
            return Err(format!("Falha na gravação do arquivo temporário {}: {}", temp_path.display(), e));
        }

        if let Err(e) = fs::rename(&temp_path, target_path) {
            if let Err(e_write) = fs::write(target_path, &json) {
                let _ = fs::remove_file(&temp_path);
                return Err(format!("Falha ao salvar membrana em {}: {} (fallback: {})", target_path.display(), e, e_write));
            }
            let _ = fs::remove_file(&temp_path);
        }

        self.is_modified = false;
        Ok(())
    }

    /// Retorna o caminho do arquivo ativo de membrana, se houver.
    pub fn active_file_path(&self) -> Option<&Path> {
        self.active_file_path.as_deref()
    }

    /// Define o caminho ativo de arquivo.
    pub fn set_active_file_path(&mut self, path: impl Into<PathBuf>) {
        self.active_file_path = Some(path.into());
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
        self.drag_subpixel_x += delta_screen_x / zoom;
        self.drag_subpixel_y += delta_screen_y / zoom;

        let step_x = self.drag_subpixel_x.trunc() as i32;
        let step_y = self.drag_subpixel_y.trunc() as i32;

        if step_x != 0 || step_y != 0 {
            if let Some(node) = self.canvas_data.get_node_mut(id) {
                let current_x = node.x();
                let current_y = node.y();
                node.set_position(current_x + step_x, current_y + step_y);
                self.drag_subpixel_x -= step_x as f32;
                self.drag_subpixel_y -= step_y as f32;
                self.is_modified = true;
            }
        }
    }

    /// Localiza o nó ou grupo visual sob as coordenadas de tela do mouse.
    pub fn find_node_at(&self, mouse_x: f32, mouse_y: f32) -> Option<String> {
        let (visual_nodes, _) = self.to_visual_models();
        // 1. Verifica primeiro os cartões de conteúdo (renderizados na frente)
        for node in visual_nodes.iter().rev() {
            if node.node_type != "group" {
                if mouse_x >= node.screen_x
                    && mouse_x <= node.screen_x + node.screen_width
                    && mouse_y >= node.screen_y
                    && mouse_y <= node.screen_y + node.screen_height
                {
                    return Some(node.id.clone());
                }
            }
        }
        // 2. Se nenhum cartão foi clicado, verifica os grupos (renderizados atrás)
        for node in visual_nodes.iter().rev() {
            if node.node_type == "group" {
                if mouse_x >= node.screen_x
                    && mouse_x <= node.screen_x + node.screen_width
                    && mouse_y >= node.screen_y
                    && mouse_y <= node.screen_y + node.screen_height
                {
                    return Some(node.id.clone());
                }
            }
        }
        None
    }

    /// Processa mouse down na Membrana: se atingir um cartão, prepara para arrastá-lo; senão, para pan.
    pub fn handle_pointer_down(&mut self, mouse_x: f32, mouse_y: f32) -> Option<String> {
        let node_id = self.find_node_at(mouse_x, mouse_y);
        self.dragging_node_id = node_id.clone();
        node_id
    }

    /// Processa o arraste na Membrana: move o cartão se houver um ativo, ou pan da câmera.
    /// Retorna `true` se um cartão foi movido, ou `false` se a câmera foi panned.
    pub fn handle_pointer_move(&mut self, dx: f32, dy: f32) -> bool {
        if let Some(id) = self.dragging_node_id.clone() {
            self.move_node(&id, dx, dy);
            true
        } else {
            self.pan(dx, dy);
            false
        }
    }

    /// Processa o término do arraste na Membrana, liberando o nó ativo.
    pub fn handle_pointer_up(&mut self) -> Option<String> {
        self.dragging_node_id.take()
    }

    /// Cria uma conexão direcionada entre dois cartões na Membrana.
    pub fn connect_cards(&mut self, from_id: &str, to_id: &str) -> String {
        if from_id.is_empty() || to_id.is_empty() || from_id == to_id {
            return String::new();
        }

        // Se já existe uma conexão entre esses dois nós, retorna o ID existente sem duplicar
        if let Some(existing) = self.canvas_data.edges.iter().find(|e| {
            (e.from_node == from_id && e.to_node == to_id)
                || (e.from_node == to_id && e.to_node == from_id)
        }) {
            return existing.id.clone();
        }

        let edge_id = format!("edge_{}_{}", from_id, to_id);
        let edge = CanvasEdge {
            id: edge_id.clone(),
            from_node: from_id.to_string(),
            from_side: Some(NodeSide::Right),
            from_end: Some(EdgeEnd::None),
            to_node: to_id.to_string(),
            to_side: Some(NodeSide::Left),
            to_end: Some(EdgeEnd::Arrow),
            color: Some("#89b4fa".to_string()),
            label: None,
            extra: HashMap::new(),
        };
        self.canvas_data.add_edge(edge);
        self.is_modified = true;
        edge_id
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
        self.is_modified = true;
        id
    }

    /// Adiciona um novo cartão de texto em uma coordenada de tela específica (ex: cursor do mouse).
    pub fn add_text_card_at(&mut self, text: &str, screen_x: f32, screen_y: f32) -> String {
        let count = self.canvas_data.node_count() + 1;
        let id = format!("text_{}", count);

        let zoom = if self.zoom > 0.01 { self.zoom } else { 1.0 };
        let world_x = ((screen_x - self.camera_x) / zoom) as i32;
        let world_y = ((screen_y - self.camera_y) / zoom) as i32;

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
        self.is_modified = true;
        id
    }

    /// Inicia a edição in-place do nó informado (apenas nós de texto).
    pub fn start_editing_node(&mut self, id: &str) {
        if let Some(CanvasNode::Text(_)) = self.canvas_data.get_node(id) {
            self.editing_node_id = Some(id.to_string());
            self.selected_node_id = Some(id.to_string());
        }
    }

    /// Aplica o novo texto no nó de texto e encerra a edição in-place.
    pub fn apply_node_edit(&mut self, id: &str, new_text: &str) -> bool {
        if let Some(CanvasNode::Text(t)) = self.canvas_data.get_node_mut(id) {
            t.text = new_text.to_string();
            self.is_modified = true;
            self.editing_node_id = None;
            true
        } else {
            self.editing_node_id = None;
            false
        }
    }

    /// Cancela a edição in-place ativa sem alterar o conteúdo.
    pub fn cancel_editing_node(&mut self) {
        self.editing_node_id = None;
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
        self.is_modified = true;
        id
    }

    /// Conecta dois nós através de uma aresta direcionada.
    pub fn connect_nodes(&mut self, from_id: &str, to_id: &str, label: Option<String>) -> String {
        let edge_id = format!("edge_{}_{}", from_id, to_id);
        let mut edge = CanvasEdge::new(&edge_id, from_id, to_id);
        edge.label = label;
        self.canvas_data.add_edge(edge);
        self.is_modified = true;
        edge_id
    }

    /// Exclui o nó selecionado atualmente (com remoção de arestas em cascata).
    pub fn delete_selected(&mut self) -> Option<String> {
        if let Some(id) = self.selected_node_id.take() {
            self.canvas_data.remove_node(&id);
            self.is_modified = true;
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
            let is_edit = self.editing_node_id.as_deref() == Some(node.id());

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
                is_editing: is_edit,
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
        let ctrl = CanvasViewController::with_default_demo();
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

    #[test]
    fn test_membrane_io_load_and_atomic_save_dual_format() {
        let temp_dir = std::env::temp_dir().join(format!("gnosis_test_membrane_io_{}", std::process::id()));
        let _ = fs::create_dir_all(&temp_dir);

        // 1. Teste com arquivo .canvas (Obsidian JSON Canvas interoperável)
        let canvas_file = temp_dir.join("Diagrama.canvas");
        let initial_json = r#"{
            "nodes": [
                {"id": "c1", "type": "text", "x": 10, "y": 20, "width": 100, "height": 50, "text": "Ideia Inicial"}
            ],
            "edges": []
        }"#;
        let _ = fs::write(&canvas_file, initial_json);

        let mut ctrl = CanvasViewController::new(CanvasData::new());
        let load_res = ctrl.load_from_file(&canvas_file);
        assert!(load_res.is_ok());

        assert_eq!(ctrl.canvas_data.node_count(), 1);
        assert_eq!(ctrl.active_file_path(), Some(canvas_file.as_path()));
        assert!(!ctrl.is_modified);

        // Mutação e gravação contínua
        let _new_id = ctrl.add_text_card("Segundo Card");
        assert!(ctrl.is_modified);
        let save_res = ctrl.save_to_disk();
        assert!(save_res.is_ok());
        assert!(!ctrl.is_modified);

        // Validação no disco rígido: deve permanecer exatamente com extensão .canvas
        assert!(canvas_file.exists());
        if let Ok(saved_content) = fs::read_to_string(&canvas_file) {
            assert!(saved_content.contains("Ideia Inicial"));
            assert!(saved_content.contains("Segundo Card"));
        } else {
            panic!("Falha ao ler arquivo salvo {}", canvas_file.display());
        }

        // 2. Teste com arquivo .membrana (Gnosis Native Pergaminho)
        let membrana_file = temp_dir.join("Pergaminho_Arcano.membrana");
        let mut ctrl_mem = CanvasViewController::new(CanvasData::new());
        ctrl_mem.set_active_file_path(&membrana_file);
        let _id_m1 = ctrl_mem.add_text_card("Sabedoria Antiga");
        let save_mem_res = ctrl_mem.save_to_disk();
        assert!(save_mem_res.is_ok());

        assert!(membrana_file.exists());
        let mut reloaded_ctrl = CanvasViewController::new(CanvasData::new());
        let reload_res = reloaded_ctrl.load_from_file(&membrana_file);
        assert!(reload_res.is_ok());
        assert_eq!(reloaded_ctrl.canvas_data.node_count(), 1);
        assert_eq!(reloaded_ctrl.active_file_path(), Some(membrana_file.as_path()));

        // Limpeza
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_membrane_full_lifecycle_and_all_node_types() {
        use gnosis_core::canvas::{CanvasEdge, CanvasNode, FileNode, GroupNode, LinkNode, NodeSide, TextNode};

        let temp_dir = std::env::temp_dir().join(format!("gnosis_test_membrane_lifecycle_{}", std::process::id()));
        let _ = fs::create_dir_all(&temp_dir);
        let membrana_path = temp_dir.join("Cosmologia.membrana");

        // Monta um CanvasData completo com os 4 tipos de nós da especificação
        let mut data = CanvasData::new();
        data.add_node(CanvasNode::Text(TextNode {
            id: "n_text".to_string(),
            x: 100,
            y: 150,
            width: 250,
            height: 120,
            color: Some("#10B981".to_string()),
            text: "Princípio Hermético do Ritmo".to_string(),
            extra: Default::default(),
        }));

        data.add_node(CanvasNode::File(FileNode {
            id: "n_file".to_string(),
            x: 450,
            y: 150,
            width: 300,
            height: 200,
            color: Some("#6366F1".to_string()),
            file: "Grimorio/Alquimia.md".to_string(),
            subpath: Some("#Transmutacao".to_string()),
            extra: Default::default(),
        }));

        data.add_node(CanvasNode::Link(LinkNode {
            id: "n_link".to_string(),
            x: 100,
            y: 350,
            width: 280,
            height: 90,
            color: None,
            url: "https://gnosis.app/docs".to_string(),
            extra: Default::default(),
        }));

        data.add_node(CanvasNode::Group(GroupNode {
            id: "n_group".to_string(),
            x: 50,
            y: 50,
            width: 750,
            height: 450,
            color: Some("#F59E0B".to_string()),
            label: Some("Esfera Celestial".to_string()),
            background: None,
            background_style: None,
            extra: Default::default(),
        }));

        // Adiciona aresta conectando n_text -> n_file com lados e rótulo
        data.add_edge(CanvasEdge {
            id: "e_transmutar".to_string(),
            from_node: "n_text".to_string(),
            from_side: Some(NodeSide::Right),
            from_end: None,
            to_node: "n_file".to_string(),
            to_side: Some(NodeSide::Left),
            to_end: Some(EdgeEnd::Arrow),
            color: Some("#8B5CF6".to_string()),
            label: Some("inspira".to_string()),
            extra: Default::default(),
        });

        assert_eq!(data.node_count(), 4);
        assert_eq!(data.edge_count(), 1);

        // Instancia o controller e salva no arquivo .membrana
        let mut ctrl = CanvasViewController::new(data);
        ctrl.set_active_file_path(&membrana_path);
        let save_res = ctrl.save_to_disk();
        assert!(save_res.is_ok(), "Salvamento inicial de membrana falhou: {:?}", save_res);
        assert!(membrana_path.exists());

        // Carrega em uma instância completamente nova
        let mut reloaded = CanvasViewController::new(CanvasData::new());
        let load_res = reloaded.load_from_file(&membrana_path);
        assert!(load_res.is_ok(), "Carregamento de membrana falhou: {:?}", load_res);

        // Validação estrita de integridade dos 4 nós
        assert_eq!(reloaded.canvas_data.node_count(), 4);
        assert_eq!(reloaded.canvas_data.edge_count(), 1);

        let n1 = reloaded.canvas_data.get_node("n_text");
        assert!(matches!(n1, Some(CanvasNode::Text(t)) if t.text == "Princípio Hermético do Ritmo" && t.x == 100));

        let n2 = reloaded.canvas_data.get_node("n_file");
        assert!(matches!(n2, Some(CanvasNode::File(f)) if f.file == "Grimorio/Alquimia.md" && f.subpath.as_deref() == Some("#Transmutacao")));

        let n3 = reloaded.canvas_data.get_node("n_link");
        assert!(matches!(n3, Some(CanvasNode::Link(l)) if l.url == "https://gnosis.app/docs"));

        let n4 = reloaded.canvas_data.get_node("n_group");
        assert!(matches!(n4, Some(CanvasNode::Group(g)) if g.label.as_deref() == Some("Esfera Celestial") && g.width == 750));

        let edge = &reloaded.canvas_data.edges[0];
        assert_eq!(edge.from_node, "n_text");
        assert_eq!(edge.to_node, "n_file");
        assert_eq!(edge.label.as_deref(), Some("inspira"));

        // Projeção visual para a interface Slint
        let (visual_nodes, visual_edges) = reloaded.to_visual_models();
        assert_eq!(visual_nodes.len(), 4);
        assert_eq!(visual_edges.len(), 1);
        assert!(visual_edges[0].has_arrow);
        assert_eq!(visual_edges[0].label, "inspira");

        // Limpeza
        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_membrane_corrupted_json_resilience() {
        let temp_dir = std::env::temp_dir().join(format!("gnosis_test_membrane_corrupted_{}", std::process::id()));
        let _ = fs::create_dir_all(&temp_dir);
        let corrupted_file = temp_dir.join("Corrompido.membrana");

        // Grava JSON truncado e inválido
        let _ = fs::write(&corrupted_file, "{ \"nodes\": [ { \"id\": \"quebrado\", \"type\": ");

        let mut ctrl = CanvasViewController::new(CanvasData::new());
        let res = ctrl.load_from_file(&corrupted_file);

        // Deve retornar Err sem pânico e sem alterar os nós existentes
        assert!(res.is_err());
        assert_eq!(ctrl.canvas_data.node_count(), 0);

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_membrane_interactive_smooth_pan_and_drag() {
        let mut ctrl = CanvasViewController::new(CanvasData::new());
        let card_id = ctrl.add_text_card("Pensamento em Movimento");

        let initial_node = ctrl.canvas_data.get_node(&card_id).cloned().expect("Nó deve existir");
        let start_x = initial_node.x();
        let start_y = initial_node.y();

        // 1. Simulação de Pan Suave por passos de delta (arraste de fundo)
        let cam_start_x = ctrl.camera_x;
        let cam_start_y = ctrl.camera_y;
        ctrl.pan(12.0, -8.0);
        ctrl.pan(5.0, 3.0);
        assert_eq!(ctrl.camera_x, cam_start_x + 17.0);
        assert_eq!(ctrl.camera_y, cam_start_y - 5.0);

        // 2. Simulação de Arraste de Nó com Zoom 1.0 (passos de delta contínuos)
        ctrl.move_node(&card_id, 20.0, 30.0);
        ctrl.move_node(&card_id, -5.0, 10.0);

        let moved_node = ctrl.canvas_data.get_node(&card_id).expect("Nó deve existir");
        assert_eq!(moved_node.x(), start_x + 15);
        assert_eq!(moved_node.y(), start_y + 40);
        assert!(ctrl.is_modified);

        // 3. Simulação de Arraste de Nó sob Zoom 2.0 (escala do mundo recalculada com precisão)
        ctrl.zoom = 2.0;
        let before_zoom_move_x = moved_node.x();
        let before_zoom_move_y = moved_node.y();
        ctrl.move_node(&card_id, 40.0, -20.0); // 40px na tela / zoom 2.0 = 20 unidades no mundo

        let post_zoom_node = ctrl.canvas_data.get_node(&card_id).expect("Nó deve existir");
        assert_eq!(post_zoom_node.x(), before_zoom_move_x + 20);
        assert_eq!(post_zoom_node.y(), before_zoom_move_y - 10);
    }

    #[test]
    fn test_membrane_node_editing_in_place_and_positional_creation() {
        let temp_dir = std::env::temp_dir().join(format!("gnosis_test_membrane_edit_{}", std::process::id()));
        let _ = fs::create_dir_all(&temp_dir);
        let membrana_path = temp_dir.join("EdicaoInPlace.membrana");

        let mut ctrl = CanvasViewController::new(CanvasData::new());
        ctrl.set_active_file_path(&membrana_path);

        // 1. Criação posicional na coordenada (400, 250) da tela
        ctrl.camera_x = 100.0;
        ctrl.camera_y = 50.0;
        ctrl.zoom = 1.0;
        let card_id = ctrl.add_text_card_at("Axioma Original", 400.0, 250.0);

        let node = ctrl.canvas_data.get_node(&card_id).expect("Nó deve existir");
        assert_eq!(node.x(), 300); // 400 - 100 = 300
        assert_eq!(node.y(), 200); // 250 - 50 = 200

        // 2. Início de Edição In-Place
        assert_eq!(ctrl.editing_node_id, None);
        ctrl.start_editing_node(&card_id);
        assert_eq!(ctrl.editing_node_id, Some(card_id.clone()));

        // Projeção visual com is_editing == true
        let (visual_nodes, _) = ctrl.to_visual_models();
        let v_node = visual_nodes.iter().find(|n| n.id == card_id).expect("Visual node deve existir");
        assert!(v_node.is_editing);
        assert_eq!(v_node.content, "Axioma Original");

        // 3. Aplicação da Edição com persistência
        let ok = ctrl.apply_node_edit(&card_id, "Axioma Transmutado e Revelado");
        assert!(ok);
        assert_eq!(ctrl.editing_node_id, None);

        let (visual_nodes_after, _) = ctrl.to_visual_models();
        let v_node_after = visual_nodes_after.iter().find(|n| n.id == card_id).expect("Visual node deve existir");
        assert!(!v_node_after.is_editing);
        assert_eq!(v_node_after.content, "Axioma Transmutado e Revelado");

        let save_res = ctrl.save_to_disk();
        assert!(save_res.is_ok());

        // 4. Recarrega do disco e valida conteúdo atualizado
        let mut reloaded = CanvasViewController::new(CanvasData::new());
        let _ = reloaded.load_from_file(&membrana_path);
        let reloaded_node = reloaded.canvas_data.get_node(&card_id).expect("Nó recarregado");
        if let CanvasNode::Text(t) = reloaded_node {
            assert_eq!(t.text, "Axioma Transmutado e Revelado");
        } else {
            panic!("Tipo de nó incorreto");
        }

        // 5. Teste de cancelamento de edição
        ctrl.start_editing_node(&card_id);
        assert_eq!(ctrl.editing_node_id, Some(card_id.clone()));
        ctrl.cancel_editing_node();
        assert_eq!(ctrl.editing_node_id, None);

        let _ = fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_membrane_interactive_connect_cards_and_subpixel_drag() {
        let mut ctrl = CanvasViewController::new(CanvasData::new());
        let id_a = ctrl.add_text_card_at("Cartão A", 100.0, 100.0);
        let id_b = ctrl.add_text_card_at("Cartão B", 400.0, 100.0);

        // Subpixel drag: pequenos deltas fracionários que acumulam
        let initial_node_a = ctrl.canvas_data.get_node(&id_a).expect("Node A").clone();
        let init_x = initial_node_a.x();
        let init_y = initial_node_a.y();

        // 3 movimentos de 0.4px = 1.2px (deve mover pelo menos 1px sem perder precisão)
        ctrl.move_node(&id_a, 0.4, 0.4);
        ctrl.move_node(&id_a, 0.4, 0.4);
        ctrl.move_node(&id_a, 0.4, 0.4);

        let moved_node_a = ctrl.canvas_data.get_node(&id_a).expect("Node A");
        assert_eq!(moved_node_a.x(), init_x + 1);
        assert_eq!(moved_node_a.y(), init_y + 1);

        // Conectar cartões A e B
        assert_eq!(ctrl.canvas_data.edges.len(), 0);
        let edge_id = ctrl.connect_cards(&id_a, &id_b);
        assert!(!edge_id.is_empty());
        assert_eq!(ctrl.canvas_data.edges.len(), 1);

        let edge = &ctrl.canvas_data.edges[0];
        assert_eq!(edge.from_node, id_a);
        assert_eq!(edge.to_node, id_b);
        assert_eq!(edge.from_side, Some(NodeSide::Right));
        assert_eq!(edge.to_side, Some(NodeSide::Left));

        // Conectar para a mesma aresta não deve duplicar
        let dup_id = ctrl.connect_cards(&id_a, &id_b);
        assert_eq!(dup_id, edge_id);
        assert_eq!(ctrl.canvas_data.edges.len(), 1);

        // Conectar para si mesmo não deve criar aresta circular
        let self_id = ctrl.connect_cards(&id_a, &id_a);
        assert!(self_id.is_empty());
        assert_eq!(ctrl.canvas_data.edges.len(), 1);

        // Projeção visual deve conter a aresta entre A e B
        let (_, visual_edges) = ctrl.to_visual_models();
        assert_eq!(visual_edges.len(), 1);
        assert_eq!(visual_edges[0].id, edge_id);
    }

    #[test]
    fn test_canvas_view_hit_testing_and_unified_drag() {
        let mut ctrl = CanvasViewController::new(CanvasData::new());
        ctrl.camera_x = 0.0;
        ctrl.camera_y = 0.0;
        ctrl.zoom = 1.0;

        let card_id = ctrl.add_text_card_at("Cartão Teste", 100.0, 100.0);

        // 1. Hit-testing: clique no centro do cartão (100 + 50, 100 + 30)
        let hit = ctrl.find_node_at(150.0, 130.0);
        assert_eq!(hit, Some(card_id.clone()));

        // 2. Hit-testing: clique fora do cartão
        let miss = ctrl.find_node_at(10.0, 10.0);
        assert_eq!(miss, None);

        // 3. Pointer Down no cartão
        let down_target = ctrl.handle_pointer_down(150.0, 130.0);
        assert_eq!(down_target, Some(card_id.clone()));
        assert_eq!(ctrl.dragging_node_id, Some(card_id.clone()));

        // 4. Pointer Move deve mover o cartão (retornando true)
        let initial_node = ctrl.canvas_data.get_node(&card_id).unwrap().clone();
        let init_x = initial_node.x();
        let init_y = initial_node.y();

        let moved = ctrl.handle_pointer_move(20.0, 15.0);
        assert!(moved);

        let moved_node = ctrl.canvas_data.get_node(&card_id).unwrap();
        assert_eq!(moved_node.x(), init_x + 20);
        assert_eq!(moved_node.y(), init_y + 15);

        // 5. Pointer Up deve liberar o nó
        let released = ctrl.handle_pointer_up();
        assert_eq!(released, Some(card_id));
        assert_eq!(ctrl.dragging_node_id, None);

        // 6. Pointer Down no vazio deve realizar pan
        let pan_target = ctrl.handle_pointer_down(5.0, 5.0);
        assert_eq!(pan_target, None);
        assert_eq!(ctrl.dragging_node_id, None);

        let initial_cam_x = ctrl.camera_x;
        let panned = ctrl.handle_pointer_move(30.0, 40.0);
        assert!(!panned);
        assert_eq!(ctrl.camera_x, initial_cam_x + 30.0);
    }
}
