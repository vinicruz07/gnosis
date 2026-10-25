//! Gnosis Graph: Grafo de conhecimento, backlinks e análise de links bidirecionais.
//!
//! Diretrizes de Qualidade:
//! - 100% Rust Seguro (`#![forbid(unsafe_code)]`).
//! - Zero pânicos em tempo de execução (`clippy::unwrap_used`, `clippy::expect_used`, `clippy::panic`).
//! - Análise de conexões, notas órfãs e links não resolvidos (notas fantasmas).

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]

pub mod error;
pub mod model;
pub mod physics;
pub mod subgraph;

pub use error::{GraphError, Result};
pub use model::{EdgeType, GraphEdge, GraphNode, KnowledgeGraph, NodeType};
pub use physics::{GraphPhysicsSimulator, ParticleNode, PhysicsConfig, Point2D};
pub use subgraph::{GraphFilterConfig, SubgraphManager};
