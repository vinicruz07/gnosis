//! Implementação pura e segura da especificação aberta JSON Canvas v1.0.
//!
//! A especificação JSON Canvas é um formato aberto e extensível baseado em JSON
//! para representação de notas visuais, quadros brancos e grafos espaciais,
//! 100% interoperável com arquivos `.canvas` gerados pelo Obsidian.
//!
//! Especificação de referência: <https://jsoncanvas.org/spec/1.0/>
//!
//! Garantias de Qualidade:
//! - 100% Rust Seguro (`#![forbid(unsafe_code)]`).
//! - Zero pânicos em tempo de execução (`clippy::unwrap_used`, `clippy::expect_used`).
//! - Preservação resiliente de atributos desconhecidos (round-trip seguro sem perda de dados).

use std::collections::HashMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{CoreError, Result};

/// Lado do nó para ancoragem de arestas (`fromSide` e `toSide`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeSide {
    Top,
    Right,
    Bottom,
    Left,
}

impl NodeSide {
    /// Retorna o nome canônico do lado.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Top => "top",
            Self::Right => "right",
            Self::Bottom => "bottom",
            Self::Left => "left",
        }
    }
}

/// Tipo de terminação de aresta (`fromEnd` e `toEnd`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EdgeEnd {
    None,
    Arrow,
}

/// Modo de ajuste de imagem de fundo para nós de grupo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BackgroundStyle {
    Cover,
    Ratio,
    Repeat,
}

/// Nó contendo texto Markdown puro.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TextNode {
    pub id: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// Nó referenciando um arquivo existente no Vault (ex: nota Markdown, imagem ou anexo).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FileNode {
    pub id: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subpath: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// Nó referenciando um link web externo (URL).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LinkNode {
    pub id: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// Nó de agrupamento visual para organizar outros nós ou definir regiões temáticas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GroupNode {
    pub id: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,
    #[serde(rename = "backgroundStyle", skip_serializing_if = "Option::is_none")]
    pub background_style: Option<BackgroundStyle>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

/// Variantes polimórficas de nós da especificação JSON Canvas v1.0.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum CanvasNode {
    Text(TextNode),
    File(FileNode),
    Link(LinkNode),
    Group(GroupNode),
}

impl CanvasNode {
    /// Identificador único do nó no canvas.
    pub fn id(&self) -> &str {
        match self {
            Self::Text(n) => &n.id,
            Self::File(n) => &n.id,
            Self::Link(n) => &n.id,
            Self::Group(n) => &n.id,
        }
    }

    /// Posição X no plano infinito.
    pub fn x(&self) -> i32 {
        match self {
            Self::Text(n) => n.x,
            Self::File(n) => n.x,
            Self::Link(n) => n.x,
            Self::Group(n) => n.x,
        }
    }

    /// Posição Y no plano infinito.
    pub fn y(&self) -> i32 {
        match self {
            Self::Text(n) => n.y,
            Self::File(n) => n.y,
            Self::Link(n) => n.y,
            Self::Group(n) => n.y,
        }
    }

    /// Largura do nó em pixels lógicos.
    pub fn width(&self) -> i32 {
        match self {
            Self::Text(n) => n.width,
            Self::File(n) => n.width,
            Self::Link(n) => n.width,
            Self::Group(n) => n.width,
        }
    }

    /// Altura do nó em pixels lógicos.
    pub fn height(&self) -> i32 {
        match self {
            Self::Text(n) => n.height,
            Self::File(n) => n.height,
            Self::Link(n) => n.height,
            Self::Group(n) => n.height,
        }
    }

    /// Cor opcional associada ao nó (número 1-6 ou cor hexadecimal).
    pub fn color(&self) -> Option<&str> {
        match self {
            Self::Text(n) => n.color.as_deref(),
            Self::File(n) => n.color.as_deref(),
            Self::Link(n) => n.color.as_deref(),
            Self::Group(n) => n.color.as_deref(),
        }
    }

    /// Retorna o nome da variante como string da especificação (`"text"`, `"file"`, `"link"`, `"group"`).
    pub const fn node_type(&self) -> &'static str {
        match self {
            Self::Text(_) => "text",
            Self::File(_) => "file",
            Self::Link(_) => "link",
            Self::Group(_) => "group",
        }
    }

    /// Altera a posição espacial $(x, y)$ do nó.
    pub fn set_position(&mut self, x: i32, y: i32) {
        match self {
            Self::Text(n) => {
                n.x = x;
                n.y = y;
            }
            Self::File(n) => {
                n.x = x;
                n.y = y;
            }
            Self::Link(n) => {
                n.x = x;
                n.y = y;
            }
            Self::Group(n) => {
                n.x = x;
                n.y = y;
            }
        }
    }

    /// Altera as dimensões $(largura, altura)$ do nó.
    pub fn set_size(&mut self, width: i32, height: i32) {
        match self {
            Self::Text(n) => {
                n.width = width;
                n.height = height;
            }
            Self::File(n) => {
                n.width = width;
                n.height = height;
            }
            Self::Link(n) => {
                n.width = width;
                n.height = height;
            }
            Self::Group(n) => {
                n.width = width;
                n.height = height;
            }
        }
    }

    /// Altera a cor do nó.
    pub fn set_color(&mut self, color: Option<String>) {
        match self {
            Self::Text(n) => n.color = color,
            Self::File(n) => n.color = color,
            Self::Link(n) => n.color = color,
            Self::Group(n) => n.color = color,
        }
    }
}

/// Aresta orientada ou não orientada conectando dois nós.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CanvasEdge {
    pub id: String,
    #[serde(rename = "fromNode")]
    pub from_node: String,
    #[serde(rename = "fromSide", skip_serializing_if = "Option::is_none")]
    pub from_side: Option<NodeSide>,
    #[serde(rename = "fromEnd", skip_serializing_if = "Option::is_none")]
    pub from_end: Option<EdgeEnd>,
    #[serde(rename = "toNode")]
    pub to_node: String,
    #[serde(rename = "toSide", skip_serializing_if = "Option::is_none")]
    pub to_side: Option<NodeSide>,
    #[serde(rename = "toEnd", skip_serializing_if = "Option::is_none")]
    pub to_end: Option<EdgeEnd>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

impl CanvasEdge {
    /// Cria uma nova aresta orientada padrão entre dois nós.
    pub fn new(id: impl Into<String>, from_node: impl Into<String>, to_node: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            from_node: from_node.into(),
            from_side: None,
            from_end: None,
            to_node: to_node.into(),
            to_side: None,
            to_end: Some(EdgeEnd::Arrow),
            color: None,
            label: None,
            extra: HashMap::new(),
        }
    }
}

/// Estrutura raiz de um arquivo `.canvas` em conformidade com JSON Canvas v1.0.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct CanvasData {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nodes: Vec<CanvasNode>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub edges: Vec<CanvasEdge>,

    #[serde(flatten)]
    pub extra: HashMap<String, Value>,
}

impl CanvasData {
    /// Cria um canvas vazio.
    pub fn new() -> Self {
        Self::default()
    }

    /// Desserializa um JSON Canvas a partir de uma string JSON UTF-8.
    pub fn from_json(json_str: &str) -> Result<Self> {
        serde_json::from_str(json_str).map_err(CoreError::from)
    }

    /// Serializa o canvas para JSON compacto.
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string(self).map_err(CoreError::from)
    }

    /// Serializa o canvas para JSON formatado com identação legível.
    pub fn to_json_pretty(&self) -> Result<String> {
        serde_json::to_string_pretty(self).map_err(CoreError::from)
    }

    /// Quantidade de nós no canvas.
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Quantidade de arestas no canvas.
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    /// Adiciona um novo nó ao canvas.
    pub fn add_node(&mut self, node: CanvasNode) {
        self.nodes.push(node);
    }

    /// Remove um nó pelo seu `id`, e remove em cascata todas as arestas conectadas a ele.
    /// Retorna o nó removido, se encontrado.
    pub fn remove_node(&mut self, id: &str) -> Option<CanvasNode> {
        let index = self.nodes.iter().position(|n| n.id() == id)?;
        let removed_node = self.nodes.remove(index);

        // Remove em cascata qualquer aresta vinculada
        self.edges.retain(|e| e.from_node != id && e.to_node != id);

        Some(removed_node)
    }

    /// Retorna referência a um nó pelo seu `id`.
    pub fn get_node(&self, id: &str) -> Option<&CanvasNode> {
        self.nodes.iter().find(|n| n.id() == id)
    }

    /// Retorna referência mutável a um nó pelo seu `id`.
    pub fn get_node_mut(&mut self, id: &str) -> Option<&mut CanvasNode> {
        self.nodes.iter_mut().find(|n| n.id() == id)
    }

    /// Adiciona uma nova aresta ao canvas.
    pub fn add_edge(&mut self, edge: CanvasEdge) {
        self.edges.push(edge);
    }

    /// Remove uma aresta pelo seu `id`.
    /// Retorna a aresta removida, se encontrada.
    pub fn remove_edge(&mut self, id: &str) -> Option<CanvasEdge> {
        let index = self.edges.iter().position(|e| e.id == id)?;
        Some(self.edges.remove(index))
    }

    /// Retorna referência a uma aresta pelo seu `id`.
    pub fn get_edge(&self, id: &str) -> Option<&CanvasEdge> {
        self.edges.iter().find(|e| e.id == id)
    }

    /// Retorna referência mutável a uma aresta pelo seu `id`.
    pub fn get_edge_mut(&mut self, id: &str) -> Option<&mut CanvasEdge> {
        self.edges.iter_mut().find(|e| e.id == id)
    }

    /// Retorna todas as arestas conectadas (origem ou destino) a um determinado nó.
    pub fn connected_edges(&self, node_id: &str) -> Vec<&CanvasEdge> {
        self.edges
            .iter()
            .filter(|e| e.from_node == node_id || e.to_node == node_id)
            .collect()
    }

    /// Calcula a caixa delimitadora (bounding box) de todos os nós presentes.
    /// Retorna `Some((min_x, min_y, max_x, max_y))` ou `None` se não houver nós.
    pub fn bounding_box(&self) -> Option<(i32, i32, i32, i32)> {
        if self.nodes.is_empty() {
            return None;
        }

        let mut min_x = i32::MAX;
        let mut min_y = i32::MAX;
        let mut max_x = i32::MIN;
        let mut max_y = i32::MIN;

        for node in &self.nodes {
            let x = node.x();
            let y = node.y();
            let w = node.width();
            let h = node.height();

            if x < min_x {
                min_x = x;
            }
            if y < min_y {
                min_y = y;
            }
            if x + w > max_x {
                max_x = x + w;
            }
            if y + h > max_y {
                max_y = y + h;
            }
        }

        Some((min_x, min_y, max_x, max_y))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_canvas_serialization_roundtrip() {
        let canvas = CanvasData::new();
        let json = canvas.to_json();
        assert!(json.is_ok());
        if let Ok(serialized) = json {
            let deserialized = CanvasData::from_json(&serialized);
            assert!(deserialized.is_ok());
            if let Ok(data) = deserialized {
                assert_eq!(data.node_count(), 0);
                assert_eq!(data.edge_count(), 0);
            }
        }
    }

    #[test]
    fn test_spec_canonical_nodes_roundtrip() {
        let mut canvas = CanvasData::new();

        canvas.add_node(CanvasNode::Text(TextNode {
            id: "text-1".to_string(),
            x: 100,
            y: 200,
            width: 300,
            height: 150,
            text: "# Título\nNota conceitual".to_string(),
            color: Some("1".to_string()),
            extra: HashMap::new(),
        }));

        canvas.add_node(CanvasNode::File(FileNode {
            id: "file-1".to_string(),
            x: 500,
            y: 200,
            width: 400,
            height: 300,
            file: "Notas/Gnosis.md".to_string(),
            subpath: Some("#Arquitetura".to_string()),
            color: None,
            extra: HashMap::new(),
        }));

        canvas.add_node(CanvasNode::Link(LinkNode {
            id: "link-1".to_string(),
            x: 100,
            y: 400,
            width: 350,
            height: 120,
            url: "https://jsoncanvas.org".to_string(),
            color: Some("#ff5500".to_string()),
            extra: HashMap::new(),
        }));

        canvas.add_node(CanvasNode::Group(GroupNode {
            id: "group-1".to_string(),
            x: 50,
            y: 50,
            width: 900,
            height: 600,
            label: Some("Grupo de Teste".to_string()),
            background: None,
            background_style: Some(BackgroundStyle::Cover),
            color: None,
            extra: HashMap::new(),
        }));

        canvas.add_edge(CanvasEdge {
            id: "edge-1".to_string(),
            from_node: "text-1".to_string(),
            from_side: Some(NodeSide::Right),
            from_end: Some(EdgeEnd::None),
            to_node: "file-1".to_string(),
            to_side: Some(NodeSide::Left),
            to_end: Some(EdgeEnd::Arrow),
            color: Some("4".to_string()),
            label: Some("fundamenta".to_string()),
            extra: HashMap::new(),
        });

        assert_eq!(canvas.node_count(), 4);
        assert_eq!(canvas.edge_count(), 1);

        let json = canvas.to_json_pretty();
        assert!(json.is_ok());

        if let Ok(serialized) = json {
            let loaded = CanvasData::from_json(&serialized);
            assert!(loaded.is_ok());

            if let Ok(data) = loaded {
                assert_eq!(data.node_count(), 4);
                assert_eq!(data.edge_count(), 1);

                let text_node = data.get_node("text-1");
                assert!(text_node.is_some());
                if let Some(CanvasNode::Text(t)) = text_node {
                    assert_eq!(t.x, 100);
                    assert_eq!(t.text, "# Título\nNota conceitual");
                    assert_eq!(t.color.as_deref(), Some("1"));
                } else {
                    panic!("esperava TextNode");
                }

                let edge = data.get_edge("edge-1");
                assert!(edge.is_some());
                if let Some(e) = edge {
                    assert_eq!(e.from_side, Some(NodeSide::Right));
                    assert_eq!(e.to_side, Some(NodeSide::Left));
                    assert_eq!(e.to_end, Some(EdgeEnd::Arrow));
                    assert_eq!(e.label.as_deref(), Some("fundamenta"));
                }
            }
        }
    }

    #[test]
    fn test_preserve_unknown_attributes() {
        let json_input = r#"{
            "nodes": [
                {
                    "id": "node-custom",
                    "type": "text",
                    "x": 10,
                    "y": 20,
                    "width": 100,
                    "height": 50,
                    "text": "Conteudo",
                    "custom_metadata_tag": "gnosis-ai-generated",
                    "zIndex": 42
                }
            ],
            "edges": [],
            "canvas_theme_version": 2
        }"#;

        let parsed = CanvasData::from_json(json_input);
        assert!(parsed.is_ok());

        if let Ok(canvas) = parsed {
            assert_eq!(canvas.extra.get("canvas_theme_version"), Some(&Value::from(2)));

            let node = canvas.get_node("node-custom");
            assert!(node.is_some());
            if let Some(CanvasNode::Text(t)) = node {
                assert_eq!(
                    t.extra.get("custom_metadata_tag"),
                    Some(&Value::from("gnosis-ai-generated"))
                );
                assert_eq!(t.extra.get("zIndex"), Some(&Value::from(42)));
            }

            // Garante que a reserialização retém os campos desconhecidos
            let reserialized = canvas.to_json();
            assert!(reserialized.is_ok());
            if let Ok(json_out) = reserialized {
                assert!(json_out.contains("custom_metadata_tag"));
                assert!(json_out.contains("gnosis-ai-generated"));
                assert!(json_out.contains("canvas_theme_version"));
            }
        }
    }

    #[test]
    fn test_node_removal_cascades_to_edges() {
        let mut canvas = CanvasData::new();
        canvas.add_node(CanvasNode::Text(TextNode {
            id: "A".to_string(),
            x: 0,
            y: 0,
            width: 100,
            height: 100,
            text: "A".to_string(),
            color: None,
            extra: HashMap::new(),
        }));
        canvas.add_node(CanvasNode::Text(TextNode {
            id: "B".to_string(),
            x: 200,
            y: 0,
            width: 100,
            height: 100,
            text: "B".to_string(),
            color: None,
            extra: HashMap::new(),
        }));
        canvas.add_node(CanvasNode::Text(TextNode {
            id: "C".to_string(),
            x: 400,
            y: 0,
            width: 100,
            height: 100,
            text: "C".to_string(),
            color: None,
            extra: HashMap::new(),
        }));

        canvas.add_edge(CanvasEdge::new("e1", "A", "B"));
        canvas.add_edge(CanvasEdge::new("e2", "B", "C"));
        canvas.add_edge(CanvasEdge::new("e3", "A", "C"));

        assert_eq!(canvas.node_count(), 3);
        assert_eq!(canvas.edge_count(), 3);

        // Remover o nó "B" deve remover em cascata "e1" e "e2", mantendo "e3"
        let removed = canvas.remove_node("B");
        assert!(removed.is_some());
        assert_eq!(canvas.node_count(), 2);
        assert_eq!(canvas.edge_count(), 1);
        assert_eq!(canvas.edges[0].id, "e3");
    }

    #[test]
    fn test_bounding_box_computation() {
        let mut canvas = CanvasData::new();
        assert_eq!(canvas.bounding_box(), None);

        canvas.add_node(CanvasNode::Text(TextNode {
            id: "1".to_string(),
            x: -50,
            y: -20,
            width: 100,
            height: 80,
            text: "".to_string(),
            color: None,
            extra: HashMap::new(),
        }));

        canvas.add_node(CanvasNode::Text(TextNode {
            id: "2".to_string(),
            x: 200,
            y: 300,
            width: 150,
            height: 100,
            text: "".to_string(),
            color: None,
            extra: HashMap::new(),
        }));

        // Node 1: x: -50..50, y: -20..60
        // Node 2: x: 200..350, y: 300..400
        // Total bounding box: (-50, -20, 350, 400)
        assert_eq!(canvas.bounding_box(), Some((-50, -20, 350, 400)));
    }
}
