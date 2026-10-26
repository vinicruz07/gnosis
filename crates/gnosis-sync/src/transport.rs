//! Transporte híbrido P2P por sala (LAN/WebRTC) e armazenamento assíncrono BYOS.
//!
//! Arquitetura:
//! - Comunicação direta P2P quando participantes estão online simultaneamente.
//! - Armazenamento assíncrono (Store-and-Forward / BYOS) quando participantes estão em horários diferentes.
//! - Criptografia ponta a ponta (E2EE) garantida antes de qualquer dado tocar a rede ou o store.

use crate::crypto::{CryptoEnvelope, HandshakeChallenge, HandshakeResponse, RoomHandshake, RoomKey, RoomSecurity};
use crate::error::Result;
use crate::merkle::VaultMerkleTree;
use gnosis_core::VaultPath;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Mensagens de protocolo trocadas entre nós conectados diretamente (P2P LAN / WebRTC).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncMessage {
    /// Desafio de autenticação de sala.
    HandshakeRequest(HandshakeChallenge),
    /// Resposta ao desafio com prova cifrada.
    HandshakeResponse(HandshakeResponse),
    /// Solicitação de snapshot da árvore Merkle.
    MerkleQuery,
    /// Resposta contendo a árvore Merkle do Vault.
    MerkleSnapshot(VaultMerkleTree),
    /// Pedido de transferência de arquivos divergentes.
    DiffRequest { paths: Vec<VaultPath> },
    /// Envio de nota ou arquivo criptografado com metadados.
    EncryptedFilePayload {
        path: VaultPath,
        envelope: CryptoEnvelope,
        modified_epoch_secs: u64,
    },
    /// Pacote de alteração colaborativa ao vivo (CRDT).
    LiveEditBatch {
        doc_path: VaultPath,
        envelope: CryptoEnvelope,
    },
    /// Confirmação de recebimento.
    Ack { path: VaultPath },
    /// Heartbeat para manter a conexão P2P ativa.
    Ping,
    Pong,
}

/// Trait abstrata para armazenamento compartilhado assíncrono (Bring Your Own Storage).
///
/// Permite conectar pastas de rede, pen drives, WebDAV, Git ou buckets S3/R2.
#[async_trait::async_trait]
pub trait ByosStore: Send + Sync {
    /// Salva um envelope cifrado em uma chave relativa da sala.
    async fn put_envelope(&self, room_id: &str, key: &str, envelope: &CryptoEnvelope) -> Result<()>;

    /// Lê um envelope cifrado previamente salvo.
    async fn get_envelope(&self, room_id: &str, key: &str) -> Result<Option<CryptoEnvelope>>;

    /// Lista todas as chaves de envelopes registradas para a sala.
    async fn list_keys(&self, room_id: &str) -> Result<Vec<String>>;

    /// Exclui um envelope.
    async fn delete_envelope(&self, room_id: &str, key: &str) -> Result<bool>;
}

/// Implementação em memória do `ByosStore` para testes, simulação de múltiplos nós e mocks.
#[derive(Default, Clone)]
pub struct MemoryByosStore {
    storage: Arc<RwLock<HashMap<String, HashMap<String, CryptoEnvelope>>>>,
}

impl MemoryByosStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait::async_trait]
impl ByosStore for MemoryByosStore {
    async fn put_envelope(&self, room_id: &str, key: &str, envelope: &CryptoEnvelope) -> Result<()> {
        let mut guard = self.storage.write().await;
        let room_map = guard.entry(room_id.to_string()).or_default();
        room_map.insert(key.to_string(), envelope.clone());
        Ok(())
    }

    async fn get_envelope(&self, room_id: &str, key: &str) -> Result<Option<CryptoEnvelope>> {
        let guard = self.storage.read().await;
        if let Some(room_map) = guard.get(room_id) {
            Ok(room_map.get(key).cloned())
        } else {
            Ok(None)
        }
    }

    async fn list_keys(&self, room_id: &str) -> Result<Vec<String>> {
        let guard = self.storage.read().await;
        if let Some(room_map) = guard.get(room_id) {
            let mut keys: Vec<String> = room_map.keys().cloned().collect();
            keys.sort();
            Ok(keys)
        } else {
            Ok(Vec::new())
        }
    }

    async fn delete_envelope(&self, room_id: &str, key: &str) -> Result<bool> {
        let mut guard = self.storage.write().await;
        if let Some(room_map) = guard.get_mut(room_id) {
            Ok(room_map.remove(key).is_some())
        } else {
            Ok(false)
        }
    }
}

/// Implementação em disco físico do `ByosStore` (pasta local, rede SMB ou pasta sincronizada por nuvem).
pub struct DirectoryByosStore {
    base_dir: PathBuf,
}

impl DirectoryByosStore {
    pub fn new(base_dir: impl AsRef<Path>) -> Self {
        Self {
            base_dir: base_dir.as_ref().to_path_buf(),
        }
    }

    fn file_path(&self, room_id: &str, key: &str) -> PathBuf {
        let sanitized_key = key.replace(['/', '\\'], "__");
        self.base_dir.join(room_id).join(format!("{}.envelope", sanitized_key))
    }
}

#[async_trait::async_trait]
impl ByosStore for DirectoryByosStore {
    async fn put_envelope(&self, room_id: &str, key: &str, envelope: &CryptoEnvelope) -> Result<()> {
        let path = self.file_path(room_id, key);
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let tmp_path = path.with_extension("tmp");
        let json_bytes = serde_json::to_vec(envelope)?;

        tokio::fs::write(&tmp_path, &json_bytes).await?;
        tokio::fs::rename(&tmp_path, &path).await?;
        Ok(())
    }

    async fn get_envelope(&self, room_id: &str, key: &str) -> Result<Option<CryptoEnvelope>> {
        let path = self.file_path(room_id, key);
        if !path.exists() {
            return Ok(None);
        }

        let bytes = tokio::fs::read(&path).await?;
        let envelope: CryptoEnvelope = serde_json::from_slice(&bytes)?;
        Ok(Some(envelope))
    }

    async fn list_keys(&self, room_id: &str) -> Result<Vec<String>> {
        let room_dir = self.base_dir.join(room_id);
        if !room_dir.exists() {
            return Ok(Vec::new())
        }

        let mut entries = tokio::fs::read_dir(&room_dir).await?;
        let mut keys = Vec::new();

        while let Ok(Some(entry)) = entries.next_entry().await {
            let file_name = entry.file_name().to_string_lossy().to_string();
            if let Some(stem) = file_name.strip_suffix(".envelope") {
                let original_key = stem.replace("__", "/");
                keys.push(original_key);
            }
        }

        keys.sort();
        Ok(keys)
    }

    async fn delete_envelope(&self, room_id: &str, key: &str) -> Result<bool> {
        let path = self.file_path(room_id, key);
        if path.exists() {
            tokio::fs::remove_file(&path).await?;
            Ok(true)
        } else {
            Ok(false)
        }
    }
}

/// Orquestrador de sincronização de sala: coordena transferências P2P e persistência assíncrona.
pub struct HybridRoomSynchronizer {
    pub security: RoomSecurity,
    pub key: RoomKey,
}

impl HybridRoomSynchronizer {
    /// Inicializa o sincronizador a partir da configuração de segurança da sala.
    pub fn new(security: RoomSecurity) -> Result<Self> {
        let key = security.derive_key()?;
        Ok(Self { security, key })
    }

    /// Publica uma nota cifrada no armazenamento assíncrono (Store-and-Forward).
    pub async fn publish_note<S: ByosStore>(&self, store: &S, path: &VaultPath, content: &[u8]) -> Result<()> {
        let envelope = self.key.encrypt(&self.security.room_id, content)?;
        store.put_envelope(&self.security.room_id, path.as_str(), &envelope).await
    }

    /// Puxa e decifra uma nota do armazenamento assíncrono.
    pub async fn pull_note<S: ByosStore>(&self, store: &S, path: &VaultPath) -> Result<Option<Vec<u8>>> {
        let maybe_env = store.get_envelope(&self.security.room_id, path.as_str()).await?;
        match maybe_env {
            Some(env) => {
                let plaintext = self.key.decrypt(&env)?;
                Ok(Some(plaintext))
            }
            None => Ok(None),
        }
    }

    /// Sincroniza todas as notas disponíveis na sala a partir do armazenamento assíncrono.
    pub async fn sync_all_from_store<S: ByosStore>(&self, store: &S) -> Result<Vec<(VaultPath, Vec<u8>)>> {
        let keys = store.list_keys(&self.security.room_id).await?;
        let mut notes = Vec::new();

        for key in keys {
            if let Ok(path) = VaultPath::parse(&key) {
                if let Some(content) = self.pull_note(store, &path).await? {
                    notes.push((path, content));
                }
            }
        }

        Ok(notes)
    }

    /// Processa uma mensagem P2P recebida de outro participante na rede local ou WebRTC.
    pub fn handle_p2p_message(&self, message: SyncMessage) -> Result<Option<SyncMessage>> {
        match message {
            SyncMessage::HandshakeRequest(challenge) => {
                let response = RoomHandshake::answer_challenge(&challenge, &self.key)?;
                Ok(Some(SyncMessage::HandshakeResponse(response)))
            }
            SyncMessage::Ping => Ok(Some(SyncMessage::Pong)),
            _ => Ok(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_byos_store_and_forward_lifecycle() -> Result<()> {
        let store = MemoryByosStore::new();
        let room_code = "GNO-1111-2222";

        // 1. Participante A cria a sala e publica uma nota
        let security_a = RoomSecurity::new("projeto_tcc_2026", Some(room_code.to_string()));
        let peer_a = HybridRoomSynchronizer::new(security_a.clone())?;

        let note_path = VaultPath::parse("Capitulos/Introducao.md")?;
        let note_content = b"# Introducao ao Gnosis\nTexto escrito pelo colega A durante a tarde.";

        peer_a.publish_note(&store, &note_path, note_content).await?;

        // 2. Participante A desliga o computador (instância A é descartada)
        drop(peer_a);

        // 3. Participante B entra na sala de noite usando o mesmo código de pareamento
        let mut security_b = RoomSecurity::new("projeto_tcc_2026", Some(room_code.to_string()));
        security_b.salt = security_a.salt; // Salt compartilhado da sala
        let peer_b = HybridRoomSynchronizer::new(security_b)?;

        // Participante B baixa as notas do projeto mesmo com A offline!
        let pulled_opt = peer_b.pull_note(&store, &note_path).await?;
        let pulled_bytes = match pulled_opt {
            Some(b) => b,
            None => return Err(SyncError::Protocol("Nota nao encontrada no store".to_string())),
        };

        assert_eq!(pulled_bytes, note_content);

        // 4. Participante B adiciona nova nota no projeto compartilhado
        let note_b_path = VaultPath::parse("Capitulos/Conclusao.md")?;
        let note_b_content = b"# Conclusao\nAdicionado pelo colega B de madrugada.";
        peer_b.publish_note(&store, &note_b_path, note_b_content).await?;

        // 5. Lista todas as notas da sala
        let all_notes = peer_b.sync_all_from_store(&store).await?;
        assert_eq!(all_notes.len(), 2);

        Ok(())
    }

    #[tokio::test]
    async fn test_unauthorized_store_access() -> Result<()> {
        let store = MemoryByosStore::new();

        let security_legit = RoomSecurity::new("sala_privada", Some("GNO-7777-8888".to_string()));
        let peer_legit = HybridRoomSynchronizer::new(security_legit.clone())?;

        let note_path = VaultPath::parse("Segredo.md")?;
        peer_legit.publish_note(&store, &note_path, b"Dados confidenciais").await?;

        // Intruso tenta ler com código errado
        let mut security_intruder = RoomSecurity::new("sala_privada", Some("GNO-ERRADO-9999".to_string()));
        security_intruder.salt = security_legit.salt;
        let peer_intruder = HybridRoomSynchronizer::new(security_intruder)?;

        let intruder_attempt = peer_intruder.pull_note(&store, &note_path).await;
        assert!(intruder_attempt.is_err(), "Intruso com codigo errado nao pode decifrar o store");

        Ok(())
    }

    #[tokio::test]
    async fn test_directory_byos_store() -> Result<()> {
        let temp_dir = std::env::temp_dir().join(format!("gnosis_dir_store_{}", std::process::id()));
        let _ = tokio::fs::create_dir_all(&temp_dir).await;

        let store = DirectoryByosStore::new(&temp_dir);
        let security = RoomSecurity::new("sala_disco", Some("GNO-3333-4444".to_string()));
        let peer = HybridRoomSynchronizer::new(security)?;

        let path = VaultPath::parse("Pastas/Artigo.md")?;
        peer.publish_note(&store, &path, b"Gravado em pasta compartilhada").await?;

        let read_back = peer.pull_note(&store, &path).await?;
        assert_eq!(read_back, Some(b"Gravado em pasta compartilhada".to_vec()));

        let keys = store.list_keys("sala_disco").await?;
        assert_eq!(keys, vec!["Pastas/Artigo.md"]);

        let _ = tokio::fs::remove_dir_all(&temp_dir).await;
        Ok(())
    }
}
