//! Barramento assíncrono de eventos desacoplado para plugins.
//!
//! Permite que extensões recebam notificações de ciclo de vida da aplicação
//! (abertura de notas, salvamento, renderização de grafos e modificações de canvas).

use gnosis_core::path::VaultPath;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

/// Tipo discriminante de eventos para filtragem e subscrições seletivas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventType {
    NoteOpened,
    NoteSaved,
    NoteDeleted,
    GraphRendered,
    CanvasModified,
    AppStarted,
    AppShutdown,
    Custom,
}

/// Evento do sistema despachado aos plugins cadastrados.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PluginEvent {
    /// Disparado quando o usuário abre uma nota no editor.
    NoteOpened {
        path: VaultPath,
        title: String,
    },
    /// Disparado após uma nota ser gravada com sucesso no VFS.
    NoteSaved {
        path: VaultPath,
        content_len: usize,
    },
    /// Disparado quando uma nota é removida ou movida para a lixeira.
    NoteDeleted {
        path: VaultPath,
    },
    /// Disparado quando a tela de grafo interativo é recalculada e renderizada.
    GraphRendered {
        node_count: usize,
        edge_count: usize,
    },
    /// Disparado quando um arquivo `.canvas` tem nós ou conexões alteradas.
    CanvasModified {
        canvas_path: VaultPath,
        node_id: Option<String>,
    },
    /// Disparado na inicialização da aplicação e montagem do Vault.
    AppStarted,
    /// Disparado antes do encerramento da aplicação para persistência de estado.
    AppShutdown,
    /// Evento arbitrário customizado entre plugins ou scripts externos.
    Custom {
        event_name: String,
        payload_json: String,
    },
}

impl PluginEvent {
    /// Retorna o tipo discriminante correspondente a este evento.
    pub fn event_type(&self) -> EventType {
        match self {
            Self::NoteOpened { .. } => EventType::NoteOpened,
            Self::NoteSaved { .. } => EventType::NoteSaved,
            Self::NoteDeleted { .. } => EventType::NoteDeleted,
            Self::GraphRendered { .. } => EventType::GraphRendered,
            Self::CanvasModified { .. } => EventType::CanvasModified,
            Self::AppStarted => EventType::AppStarted,
            Self::AppShutdown => EventType::AppShutdown,
            Self::Custom { .. } => EventType::Custom,
        }
    }
}

/// Barramento assíncrono pub/sub thread-safe para plugins.
#[derive(Debug, Clone)]
pub struct EventBus {
    /// Canal de difusão de eventos de alta velocidade.
    sender: broadcast::Sender<PluginEvent>,
    /// Mapeamento de subscrições ativas por plugin e tipo de evento.
    subscriptions: Arc<RwLock<HashMap<String, Vec<EventType>>>>,
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new(256)
    }
}

impl EventBus {
    /// Inicializa o barramento com capacidade de buffer especificada.
    pub fn new(capacity: usize) -> Self {
        let (sender, _) = broadcast::channel(capacity);
        Self {
            sender,
            subscriptions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Inscreve um plugin para receber determinados tipos de eventos.
    pub async fn subscribe_plugin(&self, plugin_id: &str, interest: Vec<EventType>) {
        let mut map = self.subscriptions.write().await;
        let _ = map.insert(plugin_id.to_string(), interest);
    }

    /// Remove a inscrição de um plugin do barramento.
    pub async fn unsubscribe_plugin(&self, plugin_id: &str) {
        let mut map = self.subscriptions.write().await;
        let _ = map.remove(plugin_id);
    }

    /// Cria um receptor assíncrono para escutar os eventos globais do barramento.
    pub fn create_receiver(&self) -> broadcast::Receiver<PluginEvent> {
        self.sender.subscribe()
    }

    /// Publica um evento no barramento, retornando o número de receptores ativos.
    pub fn publish(&self, event: PluginEvent) -> usize {
        self.sender.send(event).unwrap_or(0)
    }

    /// Verifica se um plugin específico possui interesse registrado em um tipo de evento.
    pub async fn is_interested(&self, plugin_id: &str, event_type: EventType) -> bool {
        let map = self.subscriptions.read().await;
        if let Some(types) = map.get(plugin_id) {
            types.contains(&event_type)
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_event_bus_publish_and_receive() {
        let bus = EventBus::new(16);
        let mut rx = bus.create_receiver();

        let path = VaultPath::parse("Notas/Ideia.md").expect("valid path");
        let event = PluginEvent::NoteOpened {
            path: path.clone(),
            title: "Ideia".to_string(),
        };

        let delivered = bus.publish(event);
        assert_eq!(delivered, 1);

        let received = rx.recv().await;
        assert!(received.is_ok());
        if let Ok(ev) = received {
            assert_eq!(ev.event_type(), EventType::NoteOpened);
            if let PluginEvent::NoteOpened { path: p, title } = ev {
                assert_eq!(p, path);
                assert_eq!(title, "Ideia");
            }
        }
    }

    #[tokio::test]
    async fn test_subscriptions_filtering() {
        let bus = EventBus::new(16);
        bus.subscribe_plugin("word-counter", vec![EventType::NoteOpened, EventType::NoteSaved]).await;

        assert!(bus.is_interested("word-counter", EventType::NoteOpened).await);
        assert!(bus.is_interested("word-counter", EventType::NoteSaved).await);
        assert!(!bus.is_interested("word-counter", EventType::CanvasModified).await);

        bus.unsubscribe_plugin("word-counter").await;
        assert!(!bus.is_interested("word-counter", EventType::NoteOpened).await);
    }
}
