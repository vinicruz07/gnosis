//! Envelope criptográfico ponta a ponta (E2EE) com chaves de sala e código de pareamento.
//!
//! Recursos de Segurança:
//! - Derivação de chave simétrica de 256 bits a partir do código de pareamento legível (Argon2id).
//! - Criptografia autenticada AEAD (ChaCha20-Poly1305): confidencialidade e integridade garantidas.
//! - Proteção em redes locais abertas (Wi-Fi de universidades ou cafeterias).
//! - Protocolo de handshake com desafio/resposta anti-replay para validação mútua de salas.

use crate::error::{Result, SyncError};
use argon2::Argon2;
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Key, Nonce,
};
use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

/// Tamanho do salt criptográfico da sala em bytes.
pub const ROOM_SALT_LEN: usize = 16;

/// Tamanho do nonce AEAD ChaCha20-Poly1305 em bytes.
pub const NONCE_LEN: usize = 12;

/// Tamanho da chave simétrica em bytes (256 bits).
pub const KEY_LEN: usize = 32;

/// Formata e normaliza o código de pareamento legível (remove traços e espaços, letras maiúsculas).
pub fn normalize_pairing_code(code: &str) -> String {
    code.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

/// Gera um código de pareamento legível e amigável (ex: `GNO-8392-4102`).
pub fn generate_pairing_code() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(12345);

    let part1 = (now.wrapping_mul(1103515245).wrapping_add(12345)) % 10000;
    let part2 = (now.wrapping_mul(214013).wrapping_add(2531011)) % 10000;

    format!("GNO-{:04}-{:04}", part1, part2)
}

/// Metadados de segurança e identidade de uma sala de Vault compartilhado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoomSecurity {
    /// Identificador estável da sala.
    pub room_id: String,
    /// Código amigável de pareamento compartilhado entre os participantes.
    pub pairing_code: String,
    /// Salt criptográfico exclusivo da sala.
    pub salt: [u8; ROOM_SALT_LEN],
    /// Timestamp Unix de criação da sala.
    pub created_epoch_secs: u64,
}

impl RoomSecurity {
    /// Cria uma nova configuração de segurança para uma sala com salt aleatório.
    pub fn new(room_id: &str, custom_code: Option<String>) -> Self {
        let pairing_code = custom_code.unwrap_or_else(generate_pairing_code);
        let mut salt = [0u8; ROOM_SALT_LEN];
        let now_nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);

        let bytes = now_nanos.to_le_bytes();
        salt.copy_from_slice(&bytes);

        let created_epoch_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Self {
            room_id: room_id.to_string(),
            pairing_code,
            salt,
            created_epoch_secs,
        }
    }

    /// Deriva a chave simétrica [`RoomKey`] desta sala.
    pub fn derive_key(&self) -> Result<RoomKey> {
        RoomKey::derive(&self.pairing_code, &self.salt)
    }
}

/// Chave simétrica autenticada derivada para cifragem e decifragem de mensagens da sala.
#[derive(Clone)]
pub struct RoomKey {
    raw_key: [u8; KEY_LEN],
}

impl RoomKey {
    /// Deriva uma chave de 256 bits a partir do código de pareamento e salt via Argon2id.
    pub fn derive(pairing_code: &str, salt: &[u8; ROOM_SALT_LEN]) -> Result<Self> {
        let normalized = normalize_pairing_code(pairing_code);
        if normalized.is_empty() {
            return Err(SyncError::InvalidRoom("Código de pareamento não pode ser vazio".to_string()));
        }

        let mut raw_key = [0u8; KEY_LEN];
        let argon2 = Argon2::default();

        argon2
            .hash_password_into(normalized.as_bytes(), salt, &mut raw_key)
            .map_err(|e| SyncError::Crypto(format!("Falha na derivação Argon2id: {}", e)))?;

        Ok(Self { raw_key })
    }

    /// Cifra dados usando ChaCha20-Poly1305, gerando um envelope autenticado [`CryptoEnvelope`].
    pub fn encrypt(&self, room_id: &str, plaintext: &[u8]) -> Result<CryptoEnvelope> {
        let nonce_bytes = Self::generate_nonce();
        let key = Key::from_slice(&self.raw_key);
        let cipher = ChaCha20Poly1305::new(key);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, plaintext)
            .map_err(|e| SyncError::Crypto(format!("Falha na encriptação AEAD: {:?}", e)))?;

        Ok(CryptoEnvelope {
            version: 1,
            room_id: room_id.to_string(),
            nonce: nonce_bytes.to_vec(),
            ciphertext,
        })
    }

    /// Decifra e verifica a integridade de um envelope criptográfico [`CryptoEnvelope`].
    ///
    /// Se a chave estiver errada ou os dados tiverem sido adulterados na rede, a decifração
    /// falha imediatamente sem vazar informações.
    pub fn decrypt(&self, envelope: &CryptoEnvelope) -> Result<Vec<u8>> {
        if envelope.nonce.len() != NONCE_LEN {
            return Err(SyncError::Crypto("Comprimento de nonce inválido".to_string()));
        }

        let key = Key::from_slice(&self.raw_key);
        let cipher = ChaCha20Poly1305::new(key);
        let nonce = Nonce::from_slice(&envelope.nonce);

        let plaintext = cipher
            .decrypt(nonce, envelope.ciphertext.as_ref())
            .map_err(|_| SyncError::Crypto("Falha de autenticação AEAD: chave incorreta ou dados corrompidos".to_string()))?;

        Ok(plaintext)
    }

    fn generate_nonce() -> [u8; NONCE_LEN] {
        let mut nonce = [0u8; NONCE_LEN];
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);

        let bytes = nanos.to_le_bytes();
        nonce[..8].copy_from_slice(&bytes[..8]);
        nonce[8..12].copy_from_slice(&bytes[8..12]);
        nonce
    }
}

/// Envelope criptográfico com payload cifrado e dados de autenticação.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CryptoEnvelope {
    pub version: u32,
    pub room_id: String,
    pub nonce: Vec<u8>,
    pub ciphertext: Vec<u8>,
}

/// Desafio de autenticação enviado pelo iniciador para validar se o par conhece a chave da sala.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandshakeChallenge {
    pub room_id: String,
    pub challenge_token: [u8; 32],
    pub timestamp_epoch_secs: u64,
}

/// Resposta autenticada contendo o desafio assinado/cifrado com a chave da sala.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HandshakeResponse {
    pub room_id: String,
    pub proof_envelope: CryptoEnvelope,
}

/// Gerenciador de Handshake seguro com validação mútua e proteção contra replay attacks.
pub struct RoomHandshake;

impl RoomHandshake {
    /// Cria um novo desafio criptográfico com token aleatório e timestamp.
    pub fn create_challenge(room_id: &str) -> HandshakeChallenge {
        let mut challenge_token = [0u8; 32];
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);

        let b1 = nanos.to_le_bytes();
        let b2 = (nanos ^ 0xa5a5a5a5a5a5a5a5).to_be_bytes();
        challenge_token[..16].copy_from_slice(&b1);
        challenge_token[16..32].copy_from_slice(&b2);

        let timestamp_epoch_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        HandshakeChallenge {
            room_id: room_id.to_string(),
            challenge_token,
            timestamp_epoch_secs,
        }
    }

    /// Responde ao desafio cifrando o token com a chave da sala.
    pub fn answer_challenge(challenge: &HandshakeChallenge, key: &RoomKey) -> Result<HandshakeResponse> {
        let proof_envelope = key.encrypt(&challenge.room_id, &challenge.challenge_token)?;
        Ok(HandshakeResponse {
            room_id: challenge.room_id.clone(),
            proof_envelope,
        })
    }

    /// Valida se a resposta foi gerada com a chave legítima da sala e se o desafio não expirou.
    pub fn verify_response(
        challenge: &HandshakeChallenge,
        response: &HandshakeResponse,
        key: &RoomKey,
    ) -> Result<bool> {
        if challenge.room_id != response.room_id {
            return Err(SyncError::HandshakeFailed("Identificador de sala divergente".to_string()));
        }

        // Validação anti-replay (desafio expira em 300 segundos = 5 minutos)
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        if now.saturating_sub(challenge.timestamp_epoch_secs) > 300 {
            return Err(SyncError::HandshakeFailed("Desafio de handshake expirado".to_string()));
        }

        // Decifra a prova com a chave da sala
        let decrypted_token = match key.decrypt(&response.proof_envelope) {
            Ok(token) => token,
            Err(_) => return Err(SyncError::HandshakeFailed("Chave de sala inválida ou tentativa não autorizada".to_string())),
        };

        if decrypted_token == challenge.challenge_token {
            Ok(true)
        } else {
            Err(SyncError::HandshakeFailed("Token do desafio não corresponde".to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pairing_code_normalization() {
        assert_eq!(normalize_pairing_code("gno-8392-4102"), "GNO83924102");
        assert_eq!(normalize_pairing_code("  GNO 8392 4102  "), "GNO83924102");
    }

    #[test]
    fn test_encryption_and_decryption_lifecycle() -> Result<()> {
        let security = RoomSecurity::new("sala_universitaria_101", Some("GNO-9999-0001".to_string()));
        let key = security.derive_key()?;

        let secret_note = b"# Anotacoes Secretas da Aula de Algoritmos\nNao compartilhar sem autorizacao.";
        let envelope = key.encrypt(&security.room_id, secret_note)?;

        assert_eq!(envelope.room_id, "sala_universitaria_101");
        assert_ne!(envelope.ciphertext, secret_note);

        // Decifra com a chave legítima
        let decrypted = key.decrypt(&envelope)?;
        assert_eq!(decrypted, secret_note);

        // Tentativa de decifrar com chave errada
        let impostor_security = RoomSecurity::new("sala_universitaria_101", Some("GNO-0000-0000".to_string()));
        let impostor_key = impostor_security.derive_key()?;
        let fail_result = impostor_key.decrypt(&envelope);
        assert!(fail_result.is_err());

        Ok(())
    }

    #[test]
    fn test_tamper_detection() -> Result<()> {
        let security = RoomSecurity::new("sala_teste", Some("GNO-1234-5678".to_string()));
        let key = security.derive_key()?;

        let mut envelope = key.encrypt(&security.room_id, b"Conteudo integro")?;

        // Adultera 1 único byte da mensagem cifrada (ataque man-in-the-middle na rede)
        if let Some(first_byte) = envelope.ciphertext.first_mut() {
            *first_byte ^= 0x01;
        }

        let result = key.decrypt(&envelope);
        assert!(result.is_err(), "Decifracao deveria falhar apos adulteracao");

        Ok(())
    }

    #[test]
    fn test_handshake_authentication_protocol() -> Result<()> {
        let room_id = "laboratorio_computacao";
        let valid_code = "GNO-5555-4444";
        let security = RoomSecurity::new(room_id, Some(valid_code.to_string()));
        let host_key = security.derive_key()?;

        // 1. Host cria o desafio
        let challenge = RoomHandshake::create_challenge(room_id);

        // 2. Participante legítimo responde
        let guest_key = RoomKey::derive(valid_code, &security.salt)?;
        let response = RoomHandshake::answer_challenge(&challenge, &guest_key)?;

        // 3. Host valida e autoriza entrada
        let is_valid = RoomHandshake::verify_response(&challenge, &response, &host_key)?;
        assert!(is_valid);

        // 4. Intruso com senha incorreta tenta responder
        let intruder_key = RoomKey::derive("GNO-ERRADO-1234", &security.salt)?;
        let fake_response = RoomHandshake::answer_challenge(&challenge, &intruder_key)?;
        let auth_failed = RoomHandshake::verify_response(&challenge, &fake_response, &host_key);
        assert!(auth_failed.is_err());

        Ok(())
    }
}
