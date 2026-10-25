//! Gnosis Graph: Grafo de conhecimento, backlinks e análise de links bidirecionais.

use petgraph::graph::{DiGraph, NodeIndex};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum NodeType {
    Note,
    Tag,
    Attachment,
}

#[derive(Debug, Clone)]
pub struct GraphNode {
    pub id: String,
    pub title: String,
    pub node_type: NodeType,
}

pub struct KnowledgeGraph {
    graph: DiGraph<GraphNode, ()>,
    node_indices: HashMap<String, NodeIndex>,
}

impl Default for KnowledgeGraph {
    fn default() -> Self {
        Self::new()
    }
}

impl KnowledgeGraph {
    pub fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            node_indices: HashMap::new(),
        }
    }
}
