//! Controlador do diálogo de sincronização e salas compartilhadas E2EE.

use gnosis_sync::crypto::{generate_pairing_code, normalize_pairing_code, RoomSecurity};
use serde::{Deserialize, Serialize};

/// Informações sobre um colaborador conectado na sala em tempo real.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectedPeerInfo {
    pub peer_id: String,
    pub name: String,
    pub color_hex: String,
    pub active_note: String,
    pub is_online: bool,
}

/// Estado da sessão de sincronização do aplicativo.
#[derive(Debug, Clone)]
pub struct SyncDialogController {
    pub is_open: bool,
    pub current_room_code: Option<String>,
    pub input_room_code: String,
    pub user_display_name: String,
    pub is_connected: bool,
    pub security: Option<RoomSecurity>,
    pub peers: Vec<ConnectedPeerInfo>,
    pub status_message: String,
}

impl Default for SyncDialogController {
    fn default() -> Self {
        Self::new()
    }
}

impl SyncDialogController {
    /// Cria uma nova instância do controlador de sincronização.
    pub fn new() -> Self {
        Self {
            is_open: false,
            current_room_code: None,
            input_room_code: String::new(),
            user_display_name: "Você".to_string(),
            is_connected: false,
            security: None,
            peers: Vec::new(),
            status_message: "Pronto para conectar ou criar sala segura.".to_string(),
        }
    }

    /// Abre o diálogo de sincronização.
    pub fn open(&mut self) {
        self.is_open = true;
    }

    /// Fecha o diálogo de sincronização.
    pub fn close(&mut self) {
        self.is_open = false;
    }

    /// Gera um novo código aleatório no padrão amigável `GNO-XXXX-XXXX`.
    pub fn generate_room_code(&mut self) -> String {
        let code = generate_pairing_code();
        self.input_room_code = code.clone();
        code
    }

    /// Alias para geração rápida de código.
    pub fn generate_code(&mut self) -> String {
        self.generate_room_code()
    }

    /// Conecta o usuário a uma sala com base no código fornecido.
    /// Valida o formato e deriva a chave simétrica de 256 bits com Argon2id.
    pub fn join_room(&mut self, code: &str, user_name: &str) -> Result<String, String> {
        let normalized = normalize_pairing_code(code).map_err(|e| format!("Código de sala inválido: {}", e))?;

        // Deriva envelope criptográfico ponta a ponta
        let security = RoomSecurity::from_pairing_code(&normalized);
        self.security = Some(security);
        self.current_room_code = Some(normalized.clone());
        self.is_connected = true;
        self.user_display_name = if user_name.trim().is_empty() {
            "Você".to_string()
        } else {
            user_name.trim().to_string()
        };

        // Adiciona um peer de demonstração da rede local
        self.peers.clear();
        self.peers.push(ConnectedPeerInfo {
            peer_id: "peer_local_01".to_string(),
            name: "Colega (Faculdade)".to_string(),
            color_hex: "#a6e3a1".to_string(),
            active_note: "Gnosis Core.md".to_string(),
            is_online: true,
        });

        self.status_message = format!("Conectado à sala {} com criptografia E2EE ativa.", normalized);
        Ok(normalized)
    }

    /// Desconecta da sala atual e limpa a lista de participantes.
    pub fn leave_room(&mut self) {
        self.current_room_code = None;
        self.is_connected = false;
        self.security = None;
        self.peers.clear();
        self.status_message = "Desconectado da sala compartilhada.".to_string();
    }

    /// Adiciona um novo colaborador detectado na sala.
    pub fn add_peer(&mut self, peer: ConnectedPeerInfo) {
        if !self.peers.iter().any(|p| p.peer_id == peer.peer_id) {
            self.peers.push(peer);
        }
    }

    /// Remove um colaborador que saiu da sala.
    pub fn remove_peer(&mut self, peer_id: &str) {
        self.peers.retain(|p| p.peer_id != peer_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_dialog_room_code_generation_and_joining() {
        let mut ctrl = SyncDialogController::new();

        // Gera código de sala amigável
        let code = ctrl.generate_room_code();
        assert!(code.starts_with("GNO-"));
        assert_eq!(code.len(), 13); // GNO-XXXX-XXXX

        // Conecta à sala
        let joined = ctrl.join_room(&code, "Alice");
        assert!(joined.is_ok());
        assert!(ctrl.is_connected);
        assert_eq!(ctrl.current_room_code, Some(code));
        assert!(ctrl.security.is_some());
        assert!(!ctrl.peers.is_empty());

        // Desconecta
        ctrl.leave_room();
        assert!(!ctrl.is_connected);
        assert!(ctrl.current_room_code.is_none());
        assert!(ctrl.peers.is_empty());
    }

    #[test]
    fn test_join_with_invalid_room_code_fails_gracefully() {
        let mut ctrl = SyncDialogController::new();
        let res = ctrl.join_room("CODIGO_INVALIDO", "Bob");
        assert!(res.is_err());
        assert!(!ctrl.is_connected);
    }
}
