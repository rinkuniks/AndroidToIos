//! Core error model. No secrets or keys may ever be embedded in error
//! messages or Debug output (L7).

use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("transport error: {0}")]
    Transport(String),

    #[error("integrity mismatch: object {object_id} chunk {chunk_index}")]
    IntegrityMismatch { object_id: String, chunk_index: u64 },

    #[error("checkpoint store error: {0}")]
    Checkpoint(String),

    #[error("manifest error: {0}")]
    Manifest(String),

    #[error("insufficient storage: need {required_bytes}, have {available_bytes}")]
    InsufficientStorage {
        required_bytes: u64,
        available_bytes: u64,
    },

    #[error("session is in state {state}; operation not permitted")]
    InvalidState { state: String },

    #[error("illegal session transition: {from} -> {to}")]
    InvalidTransition { from: String, to: String },

    #[error("source read error for object {object_id}")]
    SourceRead {
        object_id: String,
        #[source]
        source: std::io::Error,
    },

    #[error("hash mismatch for object {object_id}")]
    HashMismatch { object_id: String },

    #[error("size mismatch for object {object_id}")]
    SizeMismatch { object_id: String },

    #[error("engine error: {0}")]
    Engine(String),

    #[error("protocol error: {0}")]
    Protocol(String),

    #[error("pairing rejected: the code did not match, or the peer could not verify it")]
    PairingRejected,

    /// Crypto failures never carry key material, plaintext, or nonces (L7).
    /// The message is a static description, so no dynamic data can leak here.
    #[error("crypto error: {0}")]
    Crypto(&'static str),

    #[error("unsupported key version {found} (this build supports {supported})")]
    UnsupportedKeyVersion { found: u8, supported: u8 },

    #[error("wrong key purpose: frame is {found} but {expected} was used")]
    WrongKeyPurpose { found: String, expected: String },
}
