//! Motor de colaboração ao vivo em tempo real estilo Google Docs baseado em CRDT ([`Yrs`]).
//!
//! Princípios Arquiteturais:
//! - Edição simultânea sem bloqueios e sem servidor central (P2P / Local-First puro).
//! - Convergência determinística: não importa a ordem de chegada das mensagens na rede,
//!   o texto final em todas as máquinas é 100% idêntico.
//! - Cursors e presença ao vivo (`PeerCursor`): visualização de cursores coloridos de colegas.
//! - Persistência direta em arquivos Markdown (`.md`) puros no disco local.

use crate::error::{Result, SyncError};
use gnosis_core::VaultPath;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};
use yrs::{
    updates::decoder::Decode,
    Doc, GetString, ReadTxn, StateVector, Text, Transact, Update,
};

/// Metadados de cursor e presença em tempo real de um participante da sala.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerCursor {
    /// ID numérico único do participante.
    pub peer_id: u64,
    /// Nome de exibição (ex: "Alice", "Bob").
    pub user_name: String,
    /// Posição do cursor no texto (índice de caracteres).
    pub cursor_index: u32,
    /// Comprimento de seleção de texto (0 = sem seleção).
    pub selection_len: u32,
    /// Cor hexadecimal do cursor do usuário (ex: "#4CAF50", "#2196F3").
    pub color_hex: String,
    /// Timestamp Unix da última atividade do usuário.
    pub last_active_epoch_secs: u64,
}

/// Documento colaborativo gerenciado pelo motor CRDT `Yrs`.
pub struct CollaborativeDoc {
    path: VaultPath,
    peer_id: u64,
    doc: Doc,
    cursors: Arc<RwLock<BTreeMap<u64, PeerCursor>>>,
}

impl CollaborativeDoc {
    /// Cria um novo documento colaborativo a partir do conteúdo Markdown inicial.
    pub fn new(path: VaultPath, peer_id: u64, initial_content: &str) -> Self {
        let doc = Doc::new();
        let text = doc.get_or_insert_text("content");

        if !initial_content.is_empty() {
            let mut txn = doc.transact_mut();
            text.push(&mut txn, initial_content);
        }

        Self {
            path,
            peer_id,
            doc,
            cursors: Arc::new(RwLock::new(BTreeMap::new())),
        }
    }

    /// Retorna o caminho do Vault associado a este documento.
    pub fn path(&self) -> &VaultPath {
        &self.path
    }

    /// Retorna o ID do participante local.
    pub fn peer_id(&self) -> u64 {
        self.peer_id
    }

    /// Insere texto em uma posição de caractere específica.
    ///
    /// Retorna os bytes de atualização delta codificados para transmissão aos pares na rede.
    pub fn insert(&self, offset: u32, text_to_insert: &str) -> Vec<u8> {
        let text = self.doc.get_or_insert_text("content");
        let mut txn = self.doc.transact_mut();

        text.insert(&mut txn, offset, text_to_insert);

        txn.encode_update_v1()
    }

    /// Remove um intervalo de texto a partir de um deslocamento.
    ///
    /// Retorna os bytes de atualização delta para transmissão aos pares.
    pub fn delete(&self, offset: u32, len: u32) -> Vec<u8> {
        let text = self.doc.get_or_insert_text("content");
        let mut txn = self.doc.transact_mut();

        text.remove_range(&mut txn, offset, len);

        txn.encode_update_v1()
    }

    /// Aplica uma atualização delta recebida de outro nó na rede local ou internet.
    ///
    /// O motor Yrs funde as operações automaticamente sem causar sobrescritas.
    pub fn apply_remote_update(&self, update_bytes: &[u8]) -> Result<()> {
        let update = Update::decode_v1(update_bytes)
            .map_err(|e| SyncError::Crdt(format!("Erro ao decodificar update CRDT: {:?}", e)))?;

        let mut txn = self.doc.transact_mut();
        txn.apply_update(update);

        Ok(())
    }

    /// Exporta o texto consolidado atual no formato Markdown puro.
    pub fn to_markdown(&self) -> String {
        let text = self.doc.get_or_insert_text("content");
        let txn = self.doc.transact();
        text.get_string(&txn)
    }

    /// Gera um snapshot completo do estado CRDT para enviar a novos participantes que acabaram de entrar na sala.
    pub fn encode_full_state(&self) -> Vec<u8> {
        let txn = self.doc.transact();
        txn.encode_state_as_update_v1(&StateVector::default())
    }

    /// Atualiza e retorna o cursor do participante local.
    pub fn update_local_cursor(
        &self,
        user_name: &str,
        cursor_index: u32,
        selection_len: u32,
        color_hex: &str,
    ) -> PeerCursor {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        let cursor = PeerCursor {
            peer_id: self.peer_id,
            user_name: user_name.to_string(),
            cursor_index,
            selection_len,
            color_hex: color_hex.to_string(),
            last_active_epoch_secs: now,
        };

        if let Ok(mut guard) = self.cursors.write() {
            guard.insert(self.peer_id, cursor.clone());
        }

        cursor
    }

    /// Aplica a posição de cursor recebida de outro participante.
    pub fn apply_peer_cursor(&self, cursor: PeerCursor) {
        if let Ok(mut guard) = self.cursors.write() {
            guard.insert(cursor.peer_id, cursor);
        }
    }

    /// Retorna a lista de cursores ativos de todos os colegas na nota.
    pub fn list_active_cursors(&self) -> Vec<PeerCursor> {
        if let Ok(guard) = self.cursors.read() {
            guard.values().cloned().collect()
        } else {
            Vec::new()
        }
    }

    /// Grava atomicamente o texto consolidado no arquivo físico Markdown correspondente no disco do Vault.
    pub async fn flush_to_disk(&self, vault_root: &std::path::Path) -> Result<()> {
        let full_path = vault_root.join(self.path.as_str());

        if let Some(parent) = full_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }

        let tmp_path = full_path.with_extension("tmp");
        let markdown = self.to_markdown();

        tokio::fs::write(&tmp_path, markdown.as_bytes()).await?;
        tokio::fs::rename(&tmp_path, &full_path).await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collaborative_two_users_simultaneous_typing() -> Result<()> {
        let path = VaultPath::parse("Anotacoes/Pesquisa.md")?;
        let initial_text = "# Pesquisa de Rust\n";

        // Cria instância no computador de Alice com o texto inicial
        let doc_alice = CollaborativeDoc::new(path.clone(), 1, initial_text);

        // Bob conecta-se à sala e sincroniza o estado inicial de Alice
        let initial_snapshot = doc_alice.encode_full_state();
        let doc_bob = CollaborativeDoc::new(path, 2, "");
        doc_bob.apply_remote_update(&initial_snapshot)?;

        // Alice e Bob digitam SIMULTANEAMENTE sem comunicação prévia
        // Alice digita "Alice diz ola. " na posição 19
        let update_alice = doc_alice.insert(19, "Alice diz ola. ");

        // Bob digita "Bob também contribui." na posição 19
        let update_bob = doc_bob.insert(19, "Bob tambem contribui. ");

        // Alice e Bob recebem a edição um do outro via rede
        doc_bob.apply_remote_update(&update_alice)?;
        doc_alice.apply_remote_update(&update_bob)?;

        let text_alice = doc_alice.to_markdown();
        let text_bob = doc_bob.to_markdown();

        // Prova matemática de convergência CRDT:
        // Ambas as telas exibem EXATAMENTE o mesmo texto, sem sobrescrita e sem perda de caracteres!
        assert_eq!(text_alice, text_bob);
        assert!(text_alice.contains("Alice diz ola."));
        assert!(text_alice.contains("Bob tambem contribui."));

        Ok(())
    }

    #[test]
    fn test_offline_editing_and_reconnection_merge() -> Result<()> {
        let path = VaultPath::parse("Offline.md")?;
        let doc_a = CollaborativeDoc::new(path.clone(), 10, "Base inicial.\n");

        // B sincroniza o estado inicial
        let initial_snapshot = doc_a.encode_full_state();
        let doc_b = CollaborativeDoc::new(path, 20, "");
        doc_b.apply_remote_update(&initial_snapshot)?;
        assert_eq!(doc_b.to_markdown(), "Base inicial.\n");

        // A e B ficam completamente OFFLINE em redes separadas
        // A faz duas modificações
        let up_a1 = doc_a.insert(0, "Aviso importante: ");
        let up_a2 = doc_a.insert(doc_a.to_markdown().len() as u32, "Fim A.\n");

        // B faz modificações no meio do texto
        let up_b1 = doc_b.insert(5, " [revisado por B]");

        // Ambos voltam a ficar online e trocam os deltas acumulados
        doc_b.apply_remote_update(&up_a1)?;
        doc_b.apply_remote_update(&up_a2)?;
        doc_a.apply_remote_update(&up_b1)?;

        let final_a = doc_a.to_markdown();
        let final_b = doc_b.to_markdown();

        assert_eq!(final_a, final_b);
        assert!(final_a.starts_with("Aviso importante:"));
        assert!(final_a.contains("[revisado por B]"));
        assert!(final_a.ends_with("Fim A.\n"));

        Ok(())
    }

    #[test]
    fn test_peer_cursor_tracking() {
        let path = match VaultPath::parse("Cursores.md") {
            Ok(p) => p,
            Err(_) => return,
        };

        let doc = CollaborativeDoc::new(path, 1, "Linha de teste para cursores.");

        // Atualiza cursor de Alice
        let alice_cursor = doc.update_local_cursor("Alice", 10, 0, "#FF5722");
        assert_eq!(alice_cursor.user_name, "Alice");
        assert_eq!(alice_cursor.cursor_index, 10);

        // Recebe cursor de Bob
        let bob_cursor = PeerCursor {
            peer_id: 2,
            user_name: "Bob".to_string(),
            cursor_index: 25,
            selection_len: 4,
            color_hex: "#2196F3".to_string(),
            last_active_epoch_secs: 1000,
        };
        doc.apply_peer_cursor(bob_cursor);

        let active = doc.list_active_cursors();
        assert_eq!(active.len(), 2);
    }
}
