//! Simulação de física 2D de baixo custo computacional (Force-Directed Graph) com auto-sleep.
//!
//! Otimizações para hardware modesto (2015- ou anterior):
//! - Repulsão de Coulomb amortecida e atração por molas de Hooke vetorizadas em 2D.
//! - Travamento de velocidade máxima para evitar explosões numéricas.
//! - Mecanismo de **Auto-Sleep**: quando a energia cinética total do sistema cai abaixo
//!   do limiar de equilíbrio, a simulação congela consumindo **0% de CPU**.
//! - Suporte a fixação e arraste de nós pelo usuário com reativação instantânea (`wake_up`).

use crate::model::KnowledgeGraph;
use petgraph::visit::EdgeRef;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Ponto ou vetor bidimensional no espaço de visualização do grafo.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct Point2D {
    pub x: f32,
    pub y: f32,
}

impl Point2D {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    #[inline]
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    #[inline]
    pub fn distance_squared(&self, other: &Self) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        dx * dx + dy * dy
    }

    #[inline]
    pub fn distance(&self, other: &Self) -> f32 {
        self.distance_squared(other).sqrt()
    }

    #[inline]
    pub fn length_squared(&self) -> f32 {
        self.x * self.x + self.y * self.y
    }

    #[inline]
    pub fn length(&self) -> f32 {
        self.length_squared().sqrt()
    }
}

/// Partícula física associada a um nó do grafo.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ParticleNode {
    pub id: String,
    pub pos: Point2D,
    pub vel: Point2D,
    pub force: Point2D,
    pub mass: f32,
    pub is_pinned: bool,
}

/// Parâmetros de calibração do motor de física 2D.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PhysicsConfig {
    /// Constante de repulsão eletrostática entre nós.
    pub repulsion_constant: f32,
    /// Comprimento ideal de repouso das arestas (molas).
    pub spring_length: f32,
    /// Rigidez elástica das arestas conectadas.
    pub spring_stiffness: f32,
    /// Força de atração gravitacional suave em direção ao centro `(0, 0)`.
    pub center_gravity: f32,
    /// Fator de amortecimento de velocidade por passo de tempo (0.0 a 1.0).
    pub damping: f32,
    /// Velocidade máxima permitida por frame para evitar saltos bruscos.
    pub max_velocity: f32,
    /// Limiar de energia cinética total abaixo do qual a simulação entra em repouso (Auto-Sleep).
    pub sleep_energy_threshold: f32,
    /// Intervalo de tempo por passo de simulação.
    pub time_step: f32,
}

impl Default for PhysicsConfig {
    fn default() -> Self {
        Self {
            repulsion_constant: 10000.0,
            spring_length: 70.0,
            spring_stiffness: 0.05,
            center_gravity: 0.012,
            damping: 0.88,
            max_velocity: 15.0,
            sleep_energy_threshold: 0.05,
            time_step: 0.5,
        }
    }
}

/// Simulador físico de layout espacial do grafo em 2D.
pub struct GraphPhysicsSimulator {
    particles: Vec<ParticleNode>,
    id_to_index: HashMap<String, usize>,
    edges: Vec<(usize, usize, f32)>,
    config: PhysicsConfig,
    is_sleeping: bool,
    current_kinetic_energy: f32,
    total_steps: u64,
}

impl GraphPhysicsSimulator {
    /// Cria um novo simulador a partir de um `KnowledgeGraph`.
    pub fn from_knowledge_graph(graph: &KnowledgeGraph, config: PhysicsConfig) -> Self {
        let node_count = graph.node_count();
        let mut particles = Vec::with_capacity(node_count);
        let mut id_to_index = HashMap::with_capacity(node_count);

        let petgraph_ref = graph.inner_graph();

        // 1. Inicializa posições distribuídas em espiral suave ao redor do centro
        for (i, node_idx) in petgraph_ref.node_indices().enumerate() {
            if let Some(node) = petgraph_ref.node_weight(node_idx) {
                let angle = i as f32 * 0.5;
                let radius = 20.0 + (i as f32 * 8.0).sqrt() * 12.0;
                let initial_pos = Point2D::new(radius * angle.cos(), radius * angle.sin());

                id_to_index.insert(node.id.clone(), particles.len());
                particles.push(ParticleNode {
                    id: node.id.clone(),
                    pos: initial_pos,
                    vel: Point2D::ZERO,
                    force: Point2D::ZERO,
                    mass: 1.0,
                    is_pinned: false,
                });
            }
        }

        // 2. Mapeia arestas para pares de índices inteiros rápidos
        let mut edges = Vec::with_capacity(graph.edge_count());
        for edge_ref in petgraph_ref.edge_references() {
            if let (Some(source_node), Some(target_node)) = (
                petgraph_ref.node_weight(edge_ref.source()),
                petgraph_ref.node_weight(edge_ref.target()),
            ) {
                if let (Some(&u), Some(&v)) = (
                    id_to_index.get(&source_node.id),
                    id_to_index.get(&target_node.id),
                ) {
                    if u != v {
                        edges.push((u, v, edge_ref.weight().weight));
                    }
                }
            }
        }

        Self {
            particles,
            id_to_index,
            edges,
            config,
            is_sleeping: false,
            current_kinetic_energy: 100.0,
            total_steps: 0,
        }
    }

    /// Retorna se a simulação física está em estado de repouso (Auto-Sleep / 0% CPU).
    #[inline]
    pub fn is_sleeping(&self) -> bool {
        self.is_sleeping
    }

    /// Retorna a energia cinética atual do sistema.
    #[inline]
    pub fn kinetic_energy(&self) -> f32 {
        self.current_kinetic_energy
    }

    /// Reativa a simulação física caso novos nós sejam adicionados ou o usuário mova a tela.
    pub fn wake_up(&mut self) {
        self.is_sleeping = false;
    }

    /// Executa um passo atômico de simulação física (delta time).
    ///
    /// Retorna `true` se o grafo ainda está em movimento, ou `false` se atingiu o Auto-Sleep (0% CPU).
    pub fn step(&mut self) -> bool {
        if self.is_sleeping {
            return false;
        }

        self.total_steps += 1;
        let p_len = self.particles.len();

        // 1. Zera forças e aplica gravidade central
        for p in &mut self.particles {
            p.force = Point2D::ZERO;
            // Puxa suavemente para o centro (0, 0)
            p.force.x -= p.pos.x * self.config.center_gravity * p.mass;
            p.force.y -= p.pos.y * self.config.center_gravity * p.mass;
        }

        // 2. Repulsão eletrostática de Coulomb entre pares
        for i in 0..p_len {
            for j in (i + 1)..p_len {
                let dx = self.particles[i].pos.x - self.particles[j].pos.x;
                let dy = self.particles[i].pos.y - self.particles[j].pos.y;
                let dist_sq = dx * dx + dy * dy + 4.0; // Amortecimento para evitar divisão por zero

                if dist_sq < 360000.0 {
                    // Raio de corte de influência (~600px) para economizar cálculos em hardware modesto
                    let dist = dist_sq.sqrt();
                    let force = self.config.repulsion_constant / dist_sq;
                    let fx = (dx / dist) * force;
                    let fy = (dy / dist) * force;

                    self.particles[i].force.x += fx;
                    self.particles[i].force.y += fy;
                    self.particles[j].force.x -= fx;
                    self.particles[j].force.y -= fy;
                }
            }
        }

        // 3. Atração elástica por molas de Hooke nas arestas conectadas
        for &(u, v, weight) in &self.edges {
            if u < p_len && v < p_len {
                let dx = self.particles[v].pos.x - self.particles[u].pos.x;
                let dy = self.particles[v].pos.y - self.particles[u].pos.y;
                let dist = (dx * dx + dy * dy + 0.01).sqrt();
                let displacement = dist - self.config.spring_length;
                let spring_force = self.config.spring_stiffness * displacement * weight;

                let fx = (dx / dist) * spring_force;
                let fy = (dy / dist) * spring_force;

                self.particles[u].force.x += fx;
                self.particles[u].force.y += fy;
                self.particles[v].force.x -= fx;
                self.particles[v].force.y -= fy;
            }
        }

        // 4. Integração numérica com amortecimento de velocidade e cálculo da energia cinética
        let mut total_kinetic_energy = 0.0;
        let dt = self.config.time_step;

        for p in &mut self.particles {
            if !p.is_pinned {
                let ax = p.force.x / p.mass;
                let ay = p.force.y / p.mass;

                p.vel.x = (p.vel.x + ax * dt) * self.config.damping;
                p.vel.y = (p.vel.y + ay * dt) * self.config.damping;

                // Limita velocidade máxima para estabilidade absoluta
                let speed = p.vel.length();
                if speed > self.config.max_velocity {
                    p.vel.x = (p.vel.x / speed) * self.config.max_velocity;
                    p.vel.y = (p.vel.y / speed) * self.config.max_velocity;
                }

                p.pos.x += p.vel.x * dt;
                p.pos.y += p.vel.y * dt;

                total_kinetic_energy += p.vel.length_squared();
            }
        }

        self.current_kinetic_energy = total_kinetic_energy;

        // 5. Avaliação de Auto-Sleep: se o grafo estabilizou, congela o loop
        if total_kinetic_energy < self.config.sleep_energy_threshold {
            self.is_sleeping = true;
            false
        } else {
            true
        }
    }

    /// Fixa um nó em uma posição específica (ex: enquanto o usuário clica e arrasta no editor).
    pub fn pin_node(&mut self, id: &str, pos: Point2D) -> bool {
        if let Some(&idx) = self.id_to_index.get(id) {
            if let Some(p) = self.particles.get_mut(idx) {
                p.is_pinned = true;
                p.pos = pos;
                p.vel = Point2D::ZERO;
                self.wake_up();
                return true;
            }
        }
        false
    }

    /// Desafixa um nó, permitindo que a física volte a atuar sobre ele.
    pub fn unpin_node(&mut self, id: &str) -> bool {
        if let Some(&idx) = self.id_to_index.get(id) {
            if let Some(p) = self.particles.get_mut(idx) {
                p.is_pinned = false;
                self.wake_up();
                return true;
            }
        }
        false
    }

    /// Retorna a posição 2D atual de um nó pelo seu identificador.
    pub fn get_position(&self, id: &str) -> Option<Point2D> {
        let &idx = self.id_to_index.get(id)?;
        self.particles.get(idx).map(|p| p.pos)
    }

    /// Retorna uma lista de referências a todos os nós e suas coordenadas espaciais para renderização.
    pub fn get_all_positions(&self) -> Vec<(&str, Point2D)> {
        self.particles
            .iter()
            .map(|p| (p.id.as_str(), p.pos))
            .collect()
    }

    /// Retorna uma referência imutável à configuração ativa do simulador físico.
    pub fn config(&self) -> &PhysicsConfig {
        &self.config
    }

    /// Define a posição 2D de um nó pelo seu identificador e desperta a simulação.
    pub fn set_position(&mut self, id: &str, pos: Point2D) -> bool {
        self.pin_node(id, pos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{EdgeType, NodeType};

    #[test]
    fn test_point2d_vector_math() {
        let p1 = Point2D::new(0.0, 0.0);
        let p2 = Point2D::new(3.0, 4.0);

        assert_eq!(p1.distance_squared(&p2), 25.0);
        assert_eq!(p1.distance(&p2), 5.0);
    }

    #[test]
    fn test_physics_simulation_auto_sleep_convergence() {
        let mut graph = KnowledgeGraph::new();

        // Cria grafo pequeno de teste com 4 nós conectados
        graph.add_or_update_node("A", "Nota A", NodeType::Note, None);
        graph.add_or_update_node("B", "Nota B", NodeType::Note, None);
        graph.add_or_update_node("C", "Nota C", NodeType::Note, None);

        let _ = graph.add_edge("A", "B", EdgeType::WikiLink, None, 1.0);
        let _ = graph.add_edge("B", "C", EdgeType::WikiLink, None, 1.0);

        let config = PhysicsConfig {
            sleep_energy_threshold: 0.1,
            damping: 0.80, // Amortecimento rápido para teste convergente
            ..Default::default()
        };

        let mut sim = GraphPhysicsSimulator::from_knowledge_graph(&graph, config);
        assert!(!sim.is_sleeping());

        // Executa até estabilizar ou no máximo 300 passos
        let mut steps = 0;
        while steps < 300 && !sim.is_sleeping() {
            sim.step();
            steps += 1;
        }

        // Deve convergir e ativar o Auto-Sleep com sucesso
        assert!(sim.is_sleeping(), "Simulador deveria ter entrado em Auto-Sleep após {} passos", steps);
        assert!(sim.kinetic_energy() < 0.1);

        // Ao dar step em sleep, deve retornar false sem gastar CPU
        let active = sim.step();
        assert!(!active);
    }

    #[test]
    fn test_pin_and_drag_wakes_up_simulation() {
        let mut graph = KnowledgeGraph::new();
        graph.add_or_update_node("Node1", "Node1", NodeType::Note, None);
        graph.add_or_update_node("Node2", "Node2", NodeType::Note, None);
        let _ = graph.add_edge("Node1", "Node2", EdgeType::WikiLink, None, 1.0);

        let mut sim = GraphPhysicsSimulator::from_knowledge_graph(&graph, PhysicsConfig::default());

        // Força sleep
        sim.is_sleeping = true;

        // Ao fixar/arrastar um nó, a simulação deve acordar
        let pinned = sim.pin_node("Node1", Point2D::new(100.0, 100.0));
        assert!(pinned);
        assert!(!sim.is_sleeping());
        assert_eq!(sim.get_position("Node1"), Some(Point2D::new(100.0, 100.0)));
    }
}
