//! Motor de gestos multi-touch (pinch-to-zoom) e física de inércia para mobile.
//!
//! Implementação 100% Safe Rust sem `unwrap()` ou `panic`.

#![deny(unsafe_code)]

use std::collections::HashMap;

/// Ponto de toque individual rastreado pelo sistema operacional.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TouchPoint {
    pub id: u64,
    pub x: f32,
    pub y: f32,
}

impl TouchPoint {
    pub fn new(id: u64, x: f32, y: f32) -> Self {
        Self { id, x, y }
    }

    /// Distância euclidiana até outro ponto de toque.
    pub fn distance_to(&self, other: &TouchPoint) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        (dx * dx + dy * dy).sqrt()
    }

    /// Centroide (ponto médio geométrico) entre dois toques.
    pub fn centroid_with(&self, other: &TouchPoint) -> (f32, f32) {
        ((self.x + other.x) / 2.0, (self.y + other.y) / 2.0)
    }
}

/// Estado e cálculo de gestos de pinça (pinch-to-zoom).
#[derive(Debug, Clone, PartialEq)]
pub struct PinchState {
    pub initial_distance: f32,
    pub current_distance: f32,
    pub centroid: (f32, f32),
}

impl PinchState {
    /// Calcula a escala instantânea relativa à distância inicial.
    pub fn scale_factor(&self) -> f32 {
        if self.initial_distance > 0.001 {
            self.current_distance / self.initial_distance
        } else {
            1.0
        }
    }
}

/// Simulador de física de inércia para pan suave após liberação táctil (fling).
#[derive(Debug, Clone, PartialEq)]
pub struct InertiaPhysics {
    pub velocity_x: f32,
    pub velocity_y: f32,
    pub friction: f32,
    pub min_velocity_threshold: f32,
}

impl Default for InertiaPhysics {
    fn default() -> Self {
        Self::new(0.92, 0.08)
    }
}

impl InertiaPhysics {
    /// Cria uma nova instância com coeficientes de atrito e limiar de parada.
    pub fn new(friction: f32, min_velocity_threshold: f32) -> Self {
        Self {
            velocity_x: 0.0,
            velocity_y: 0.0,
            friction: friction.clamp(0.01, 0.99),
            min_velocity_threshold: min_velocity_threshold.max(0.001),
        }
    }

    /// Inicia um movimento inercial a partir de um vetor de velocidade de liberação.
    pub fn fling(&mut self, vx: f32, vy: f32) {
        self.velocity_x = vx;
        self.velocity_y = vy;
    }

    /// Executa um passo da simulação de atrito.
    /// Retorna `Some((dx, dy))` com o deslocamento inercial a aplicar na câmera,
    /// ou `None` se a velocidade caiu abaixo do limiar de parada.
    pub fn step(&mut self) -> Option<(f32, f32)> {
        let speed = (self.velocity_x * self.velocity_x + self.velocity_y * self.velocity_y).sqrt();
        if speed < self.min_velocity_threshold {
            self.velocity_x = 0.0;
            self.velocity_y = 0.0;
            None
        } else {
            let dx = self.velocity_x;
            let dy = self.velocity_y;
            self.velocity_x *= self.friction;
            self.velocity_y *= self.friction;
            Some((dx, dy))
        }
    }

    /// Retorna `true` se a inércia ainda estiver em andamento.
    pub fn is_active(&self) -> bool {
        let speed = (self.velocity_x * self.velocity_x + self.velocity_y * self.velocity_y).sqrt();
        speed >= self.min_velocity_threshold
    }

    /// Interrompe qualquer movimento inercial ativo imediatamente (ex: ao tocar na tela).
    pub fn stop(&mut self) {
        self.velocity_x = 0.0;
        self.velocity_y = 0.0;
    }
}

/// Rastreador unificado de toques, gestos multi-touch e inércia táctil.
#[derive(Debug, Clone)]
pub struct TouchGestureTracker {
    active_touches: HashMap<u64, TouchPoint>,
    last_single_touch: Option<TouchPoint>,
    initial_pinch: Option<PinchState>,
    pub inertia: InertiaPhysics,
    last_delta: (f32, f32),
}

impl Default for TouchGestureTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl TouchGestureTracker {
    pub fn new() -> Self {
        Self {
            active_touches: HashMap::new(),
            last_single_touch: None,
            initial_pinch: None,
            inertia: InertiaPhysics::default(),
            last_delta: (0.0, 0.0),
        }
    }

    /// Processa o início de um toque (pointer down).
    pub fn touch_down(&mut self, id: u64, x: f32, y: f32) {
        self.inertia.stop();
        let pt = TouchPoint::new(id, x, y);
        self.active_touches.insert(id, pt);

        if self.active_touches.len() == 1 {
            self.last_single_touch = Some(pt);
            self.initial_pinch = None;
            self.last_delta = (0.0, 0.0);
        } else if self.active_touches.len() == 2 {
            self.setup_pinch();
        }
    }

    /// Processa a movimentação de um toque (pointer move).
    /// Retorna:
    /// - `TouchAction::Pan(dx, dy)` quando 1 dedo estiver em movimento.
    /// - `TouchAction::Pinch { scale_factor, centroid_x, centroid_y }` quando 2 dedos realizam pinça.
    /// - `TouchAction::None` caso contrário.
    pub fn touch_move(&mut self, id: u64, x: f32, y: f32) -> TouchAction {
        let new_pt = TouchPoint::new(id, x, y);
        self.active_touches.insert(id, new_pt);

        if self.active_touches.len() == 1 {
            if let Some(last) = self.last_single_touch {
                let dx = x - last.x;
                let dy = y - last.y;
                self.last_single_touch = Some(new_pt);
                self.last_delta = (dx, dy);
                TouchAction::Pan(dx, dy)
            } else {
                self.last_single_touch = Some(new_pt);
                TouchAction::None
            }
        } else if self.active_touches.len() == 2 {
            let pts: Vec<&TouchPoint> = self.active_touches.values().collect();
            if pts.len() >= 2 {
                let p1 = pts[0];
                let p2 = pts[1];
                let current_dist = p1.distance_to(p2);
                let (cx, cy) = p1.centroid_with(p2);

                if let Some(ref pinch) = self.initial_pinch {
                    let factor = if pinch.current_distance > 0.001 {
                        current_dist / pinch.current_distance
                    } else {
                        1.0
                    };

                    self.initial_pinch = Some(PinchState {
                        initial_distance: pinch.initial_distance,
                        current_distance: current_dist,
                        centroid: (cx, cy),
                    });

                    TouchAction::Pinch {
                        scale_factor: factor,
                        centroid_x: cx,
                        centroid_y: cy,
                    }
                } else {
                    self.setup_pinch();
                    TouchAction::None
                }
            } else {
                TouchAction::None
            }
        } else {
            TouchAction::None
        }
    }

    /// Processa a liberação ou cancelamento de um toque (pointer up / cancel).
    pub fn touch_up(&mut self, id: u64) {
        self.active_touches.remove(&id);

        if self.active_touches.is_empty() {
            // Ao soltar o último dedo, aplica a inércia se houve arraste significativo
            let (dx, dy) = self.last_delta;
            let magnitude = (dx * dx + dy * dy).sqrt();
            if magnitude > 1.0 {
                self.inertia.fling(dx * 1.2, dy * 1.2);
            }
            self.last_single_touch = None;
            self.initial_pinch = None;
            self.last_delta = (0.0, 0.0);
        } else if self.active_touches.len() == 1 {
            // Restou 1 dedo: reseta o pinch e volta para modo pan com o ponto remanescente
            if let Some(&remaining) = self.active_touches.values().next() {
                self.last_single_touch = Some(remaining);
            }
            self.initial_pinch = None;
        }
    }

    /// Configura o estado inicial do gesto de pinça quando 2 dedos são detectados.
    fn setup_pinch(&mut self) {
        let pts: Vec<&TouchPoint> = self.active_touches.values().collect();
        if pts.len() >= 2 {
            let dist = pts[0].distance_to(pts[1]);
            let centroid = pts[0].centroid_with(pts[1]);
            self.initial_pinch = Some(PinchState {
                initial_distance: dist,
                current_distance: dist,
                centroid,
            });
            self.last_single_touch = None;
        }
    }

    /// Quantidade de toques ativos na tela.
    pub fn active_touch_count(&self) -> usize {
        self.active_touches.len()
    }
}

/// Ação produzida pelo rastreador de toques.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TouchAction {
    None,
    Pan(f32, f32),
    Pinch {
        scale_factor: f32,
        centroid_x: f32,
        centroid_y: f32,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_touch_point_distance_and_centroid() {
        let p1 = TouchPoint::new(1, 0.0, 0.0);
        let p2 = TouchPoint::new(2, 60.0, 80.0);

        assert!((p1.distance_to(&p2) - 100.0).abs() < 0.001);

        let (cx, cy) = p1.centroid_with(&p2);
        assert_eq!(cx, 30.0);
        assert_eq!(cy, 40.0);
    }

    #[test]
    fn test_pinch_scale_factor_calculation() {
        let pinch = PinchState {
            initial_distance: 100.0,
            current_distance: 150.0,
            centroid: (50.0, 50.0),
        };
        assert!((pinch.scale_factor() - 1.5).abs() < 0.001);
    }

    #[test]
    fn test_inertia_physics_decay_and_stopping() {
        let mut inertia = InertiaPhysics::new(0.8, 0.5);
        inertia.fling(10.0, 0.0);
        assert!(inertia.is_active());

        let mut steps = 0;
        while let Some((dx, _)) = inertia.step() {
            assert!(dx > 0.0);
            steps += 1;
            if steps > 50 {
                break;
            }
        }

        assert!(!inertia.is_active());
        assert_eq!(inertia.step(), None);
    }

    #[test]
    fn test_touch_gesture_tracker_pan_and_pinch_lifecycle() {
        let mut tracker = TouchGestureTracker::new();

        // 1. Um dedo tocando e arrastando
        tracker.touch_down(1, 100.0, 100.0);
        assert_eq!(tracker.active_touch_count(), 1);

        let action = tracker.touch_move(1, 110.0, 105.0);
        assert_eq!(action, TouchAction::Pan(10.0, 5.0));

        // 2. Segundo dedo toca: entra em modo Pinch
        tracker.touch_down(2, 200.0, 200.0);
        assert_eq!(tracker.active_touch_count(), 2);

        // Afastando os dedos (Zoom In)
        let action = tracker.touch_move(2, 250.0, 250.0);
        match action {
            TouchAction::Pinch { scale_factor, centroid_x, centroid_y } => {
                assert!(scale_factor > 1.0);
                assert!(centroid_x > 0.0 && centroid_y > 0.0);
            }
            _ => panic!("Esperado gesto de Pinch"),
        }

        // 3. Levantar um dedo: volta a um dedo só
        tracker.touch_up(2);
        assert_eq!(tracker.active_touch_count(), 1);

        // 4. Levantar último dedo: aciona inércia se aplicável
        tracker.touch_up(1);
        assert_eq!(tracker.active_touch_count(), 0);
    }
}

