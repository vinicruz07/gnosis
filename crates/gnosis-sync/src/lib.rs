//! Gnosis Sync: Sincronização Local-First e colaboração ao vivo em tempo real.
//!
//! Diretrizes de Qualidade:
//! - 100% Rust Seguro (`#![forbid(unsafe_code)]`).
//! - Zero pânicos em tempo de execução (`clippy::unwrap_used`, `clippy::expect_used`, `clippy::panic`).
//! - Detecção instantânea de diffs em $O(\log N)$ via [`VaultMerkleTree`].

#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::expect_used)]
#![deny(clippy::panic)]

pub mod crypto;
pub mod error;
pub mod merkle;
pub mod transport;

pub use crypto::{
    generate_pairing_code, normalize_pairing_code, CryptoEnvelope, HandshakeChallenge,
    HandshakeResponse, RoomHandshake, RoomKey, RoomSecurity, KEY_LEN, NONCE_LEN, ROOM_SALT_LEN,
};
pub use error::{Result, SyncError};
pub use merkle::{compute_hash, MerkleNode, VaultDiff, VaultMerkleTree};
pub use transport::{
    ByosStore, DirectoryByosStore, HybridRoomSynchronizer, MemoryByosStore, SyncMessage,
};
