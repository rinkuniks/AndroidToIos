//! AEAD framing, Argon2id KDF, and key versioning (Plan Phase 1; v1.2 §8).
//!
//! Every frame written to a transport or a vault is sealed with AES-256-GCM and
//! carries an explicit header, so a receiver can always tell *which* key
//! version and *which* purpose produced it:
//!
//! ```text
//! [ key_version: 1 ][ purpose: 1 ][ nonce: 12 ][ ciphertext || tag: n + 16 ]
//! ```
//!
//! Additional authenticated data (AAD) binds a frame to its logical position:
//! [`chunk_aad`] binds object id + sequence (so a sealed chunk cannot be
//! replayed into another object or slot) and [`manifest_aad`] binds the
//! migration id. Keys never appear in `Debug` output, error messages, or logs
//! (L7): crypto errors carry static descriptions only.
//!
//! Keys are derived per **purpose** ([`KeyPurpose`]) from the passphrase using
//! Argon2id with a purpose label mixed into the input, so a vault key can never
//! open a session frame — that mismatch is detected via the header as well.

use crate::{Error, Result};
use aes_gcm::aead::{Aead, Payload};
use aes_gcm::{Aes256Gcm, KeyInit, Nonce};
use argon2::{Algorithm, Argon2, Params, Version};
use serde::{Deserialize, Serialize};
use std::fmt;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// Symmetric key length (AES-256-GCM).
pub const KEY_LEN: usize = 32;
/// GCM nonce length (96-bit, the recommended GCM nonce size).
pub const NONCE_LEN: usize = 12;
/// GCM authentication tag length.
pub const TAG_LEN: usize = 16;
/// Argon2id salt length.
pub const SALT_LEN: usize = 16;
/// `key_version || purpose || nonce`.
pub const HEADER_LEN: usize = 2 + NONCE_LEN;
/// Smallest possible sealed frame (empty plaintext).
pub const MIN_SEALED_LEN: usize = HEADER_LEN + TAG_LEN;

/// Key version emitted by this build. A future build must keep reading version 1
/// vaults and bump [`KEY_VERSION`] rather than silently re-keying.
pub const KEY_VERSION: u8 = 1;

/// Domain separation for keys and frames.
///
/// Session keys live only in memory for one transfer; vault keys are
/// passphrase-derived and portable (v1.2 §8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyPurpose {
    /// Ephemeral key for one transport session (paired via QR / discovery).
    Session,
    /// Portable key held in the Secure Vault header.
    Vault,
}

impl KeyPurpose {
    /// Wire/header encoding. Stable — never renumber a shipped purpose.
    pub const fn byte(self) -> u8 {
        match self {
            KeyPurpose::Session => 1,
            KeyPurpose::Vault => 2,
        }
    }

    /// Label mixed into the KDF input for domain separation.
    pub const fn label(self) -> &'static str {
        match self {
            KeyPurpose::Session => "wa-bridge/session/v1",
            KeyPurpose::Vault => "wa-bridge/vault/v1",
        }
    }

    /// Human-readable name used in error messages (contains no secrets).
    pub const fn name(self) -> &'static str {
        match self {
            KeyPurpose::Session => "session",
            KeyPurpose::Vault => "vault",
        }
    }

    pub fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            1 => Some(KeyPurpose::Session),
            2 => Some(KeyPurpose::Vault),
            _ => None,
        }
    }
}

/// Argon2id cost parameters, persisted alongside the vault so another machine
/// (e.g. the Phase 8 desktop viewer) can re-derive the same key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct KdfParams {
    /// Memory cost in KiB.
    pub m_cost_kib: u32,
    /// Time cost (iterations).
    pub t_cost: u32,
    /// Parallelism / lanes.
    pub p_cost: u32,
}

impl Default for KdfParams {
    /// 64 MiB / 3 iterations / 1 lane — defensible on mobile for a single
    /// unlock, and re-tunable in Phase 0/10 without a format change.
    fn default() -> Self {
        Self {
            m_cost_kib: 64 * 1024,
            t_cost: 3,
            p_cost: 1,
        }
    }
}

/// A 32-byte symmetric key that zeroizes on drop and never prints itself.
#[derive(Zeroize, ZeroizeOnDrop)]
pub struct SecretKey([u8; KEY_LEN]);

impl SecretKey {
    /// Wrap raw key bytes (e.g. a key received out-of-band via QR).
    pub fn from_bytes(bytes: [u8; KEY_LEN]) -> Self {
        Self(bytes)
    }

    /// Borrow the raw key bytes. Callers must not log or persist the result.
    pub(crate) fn expose(&self) -> &[u8; KEY_LEN] {
        &self.0
    }
}

// Deliberately redacted: `Debug` output must never contain key material (L7).
impl fmt::Debug for SecretKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SecretKey(<redacted {} bytes>)", KEY_LEN)
    }
}

/// Generate a fresh random key from the OS CSPRNG.
pub fn random_key() -> Result<SecretKey> {
    Ok(SecretKey(os_random::<KEY_LEN>()?))
}

/// Generate a fresh KDF salt from the OS CSPRNG.
pub fn random_salt() -> Result<[u8; SALT_LEN]> {
    os_random::<SALT_LEN>()
}

/// Generate a fresh AEAD nonce from the OS CSPRNG.
pub fn random_nonce() -> Result<[u8; NONCE_LEN]> {
    os_random::<NONCE_LEN>()
}

fn os_random<const N: usize>() -> Result<[u8; N]> {
    let mut buf = [0u8; N];
    getrandom::getrandom(&mut buf).map_err(|_| Error::Crypto("os rng unavailable"))?;
    Ok(buf)
}

/// Derive a purpose-separated key from a passphrase with Argon2id.
///
/// Deterministic for a given `(passphrase, purpose, salt, params)` tuple, so the
/// vault can be reopened on another machine using the recorded header values.
pub fn derive_key(
    passphrase: &str,
    purpose: KeyPurpose,
    salt: &[u8; SALT_LEN],
    params: &KdfParams,
) -> Result<SecretKey> {
    let params = Params::new(
        params.m_cost_kib,
        params.t_cost,
        params.p_cost,
        Some(KEY_LEN),
    )
    .map_err(|_| Error::Crypto("invalid kdf parameters"))?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);

    // The purpose label is mixed into the password input so the two key spaces
    // stay independent even for an identical passphrase and salt.
    let mut input = Vec::with_capacity(purpose.label().len() + passphrase.len() + 1);
    input.extend_from_slice(purpose.label().as_bytes());
    input.push(0x1f);
    input.extend_from_slice(passphrase.as_bytes());

    let mut out = [0u8; KEY_LEN];
    let result = argon.hash_password_into(&input, salt, &mut out);
    input.zeroize();
    result.map_err(|_| Error::Crypto("kdf failed"))?;
    Ok(SecretKey(out))
}

/// AAD binding a sealed frame to one (object, sequence) slot on the wire.
pub fn chunk_aad(object_id: &str, sequence: u64) -> Vec<u8> {
    let mut aad = Vec::with_capacity(24 + object_id.len());
    aad.extend_from_slice(b"wa-bridge/chunk/v1");
    aad.push(0x00);
    aad.extend_from_slice(object_id.as_bytes());
    aad.push(0x00);
    aad.extend_from_slice(&sequence.to_be_bytes());
    aad
}

/// AAD binding a sealed manifest frame to one migration.
pub fn manifest_aad(migration_id: &str) -> Vec<u8> {
    let mut aad = Vec::with_capacity(28 + migration_id.len());
    aad.extend_from_slice(b"wa-bridge/manifest/v1");
    aad.push(0x00);
    aad.extend_from_slice(migration_id.as_bytes());
    aad
}

/// Seal `plaintext` with AES-256-GCM, emitting `header || ciphertext || tag`.
pub fn seal(key: &SecretKey, purpose: KeyPurpose, aad: &[u8], plaintext: &[u8]) -> Result<Vec<u8>> {
    let cipher =
        Aes256Gcm::new_from_slice(key.expose()).map_err(|_| Error::Crypto("invalid key length"))?;
    let nonce_bytes = random_nonce()?;
    let nonce = Nonce::from_slice(&nonce_bytes);

    let mut out = Vec::with_capacity(HEADER_LEN + plaintext.len() + TAG_LEN);
    out.push(KEY_VERSION);
    out.push(purpose.byte());
    out.extend_from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(
            nonce,
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|_| Error::Crypto("seal failed"))?;
    out.extend_from_slice(&ciphertext);
    Ok(out)
}

/// Open a frame produced by [`seal`]. Validates key version, purpose, and the
/// GCM tag — a wrong key, a flipped byte, or a replay into another slot all
/// fail here rather than yielding silently wrong data.
pub fn open(key: &SecretKey, purpose: KeyPurpose, aad: &[u8], sealed: &[u8]) -> Result<Vec<u8>> {
    if sealed.len() < MIN_SEALED_LEN {
        return Err(Error::Crypto("frame too short"));
    }
    let version = sealed[0];
    if version != KEY_VERSION {
        return Err(Error::UnsupportedKeyVersion {
            found: version,
            supported: KEY_VERSION,
        });
    }
    let found_purpose =
        KeyPurpose::from_byte(sealed[1]).ok_or(Error::Crypto("unknown key purpose"))?;
    if found_purpose != purpose {
        return Err(Error::WrongKeyPurpose {
            found: found_purpose.name().to_string(),
            expected: purpose.name().to_string(),
        });
    }

    let cipher =
        Aes256Gcm::new_from_slice(key.expose()).map_err(|_| Error::Crypto("invalid key length"))?;
    let nonce = Nonce::from_slice(&sealed[2..HEADER_LEN]);
    cipher
        .decrypt(
            nonce,
            Payload {
                msg: &sealed[HEADER_LEN..],
                aad,
            },
        )
        .map_err(|_| Error::Crypto("authentication failed"))
}

/// Seal one chunk frame, bound to its object id and sequence number.
pub fn seal_chunk(
    key: &SecretKey,
    object_id: &str,
    sequence: u64,
    plaintext: &[u8],
) -> Result<Vec<u8>> {
    seal(
        key,
        KeyPurpose::Session,
        &chunk_aad(object_id, sequence),
        plaintext,
    )
}

/// Open one chunk frame produced by [`seal_chunk`].
pub fn open_chunk(
    key: &SecretKey,
    object_id: &str,
    sequence: u64,
    sealed: &[u8],
) -> Result<Vec<u8>> {
    open(
        key,
        KeyPurpose::Session,
        &chunk_aad(object_id, sequence),
        sealed,
    )
}

/// Seal the manifest frame, bound to its migration id.
pub fn seal_manifest(key: &SecretKey, migration_id: &str, manifest_json: &[u8]) -> Result<Vec<u8>> {
    seal(
        key,
        KeyPurpose::Vault,
        &manifest_aad(migration_id),
        manifest_json,
    )
}

/// Open the manifest frame produced by [`seal_manifest`].
pub fn open_manifest(key: &SecretKey, migration_id: &str, sealed: &[u8]) -> Result<Vec<u8>> {
    open(key, KeyPurpose::Vault, &manifest_aad(migration_id), sealed)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cheap Argon2id settings so the KDF tests stay fast; production defaults
    /// live in `KdfParams::default()`.
    fn fast_params() -> KdfParams {
        KdfParams {
            m_cost_kib: 8,
            t_cost: 1,
            p_cost: 1,
        }
    }

    fn fixed_key(byte: u8) -> SecretKey {
        SecretKey::from_bytes([byte; KEY_LEN])
    }

    #[test]
    fn seal_open_roundtrip_and_header_layout() {
        let key = random_key().unwrap();
        let aad = chunk_aad("obj-1", 7);
        let plaintext = b"hello wa-bridge";

        let sealed = seal(&key, KeyPurpose::Session, &aad, plaintext).unwrap();

        assert_eq!(sealed.len(), HEADER_LEN + plaintext.len() + TAG_LEN);
        assert_eq!(sealed[0], KEY_VERSION, "key version must lead the frame");
        assert_eq!(sealed[1], KeyPurpose::Session.byte());
        // The plaintext must not appear in the frame.
        assert!(!sealed.windows(plaintext.len()).any(|w| w == plaintext));

        let opened = open(&key, KeyPurpose::Session, &aad, &sealed).unwrap();
        assert_eq!(opened, plaintext);
    }

    #[test]
    fn empty_payload_roundtrips() {
        let key = fixed_key(0x11);
        let sealed = seal(&key, KeyPurpose::Session, b"aad", b"").unwrap();
        assert_eq!(sealed.len(), MIN_SEALED_LEN);
        assert_eq!(
            open(&key, KeyPurpose::Session, b"aad", &sealed).unwrap(),
            b""
        );
    }

    #[test]
    fn every_flipped_ciphertext_byte_is_rejected() {
        let key = fixed_key(0x22);
        let aad = chunk_aad("o1", 0);
        let sealed = seal(&key, KeyPurpose::Session, &aad, b"0123456789").unwrap();

        // Ciphertext and tag bytes are both covered by the GCM tag.
        for i in HEADER_LEN..sealed.len() {
            let mut tampered = sealed.clone();
            tampered[i] ^= 0x01;
            assert!(
                open(&key, KeyPurpose::Session, &aad, &tampered).is_err(),
                "flipped byte {i} must not authenticate"
            );
        }
    }

    #[test]
    fn tampered_nonce_is_rejected() {
        let key = fixed_key(0x33);
        let sealed = seal(&key, KeyPurpose::Session, b"aad", b"payload").unwrap();
        let mut tampered = sealed.clone();
        tampered[2] ^= 0xFF; // first nonce byte
        assert!(open(&key, KeyPurpose::Session, b"aad", &tampered).is_err());
    }

    #[test]
    fn tampered_aad_is_rejected() {
        let key = fixed_key(0x34);
        let sealed = seal(&key, KeyPurpose::Session, &chunk_aad("o1", 1), b"payload").unwrap();
        let err = open(&key, KeyPurpose::Session, &chunk_aad("o1", 2), &sealed).unwrap_err();
        assert!(matches!(err, Error::Crypto("authentication failed")));
    }

    #[test]
    fn wrong_key_is_rejected() {
        let sealed = seal(&fixed_key(0x44), KeyPurpose::Session, b"aad", b"payload").unwrap();
        assert!(open(&fixed_key(0x45), KeyPurpose::Session, b"aad", &sealed).is_err());
    }

    #[test]
    fn crypto_errors_contain_no_dynamic_data() {
        let key = fixed_key(0xCD);
        let sealed = seal(&key, KeyPurpose::Session, b"aad", b"payload").unwrap();
        let err = open(&fixed_key(0xCE), KeyPurpose::Session, b"aad", &sealed).unwrap_err();
        assert_eq!(err.to_string(), "crypto error: authentication failed");
    }

    #[test]
    fn sealed_chunk_cannot_be_replayed_into_another_slot() {
        let key = fixed_key(0x55);
        let sealed = seal_chunk(&key, "obj-a", 3, b"chunk-body").unwrap();

        assert_eq!(
            open_chunk(&key, "obj-a", 3, &sealed).unwrap(),
            b"chunk-body"
        );
        // A different sequence, or a different object, must both fail.
        assert!(open_chunk(&key, "obj-a", 4, &sealed).is_err());
        assert!(open_chunk(&key, "obj-b", 3, &sealed).is_err());
    }

    #[test]
    fn key_purpose_is_enforced() {
        let key = fixed_key(0x66);
        let sealed = seal(&key, KeyPurpose::Vault, &manifest_aad("mig-1"), b"{}").unwrap();
        let err = open(&key, KeyPurpose::Session, &manifest_aad("mig-1"), &sealed).unwrap_err();
        assert!(
            matches!(err, Error::WrongKeyPurpose { ref found, ref expected }
                if found == "vault" && expected == "session"),
            "unexpected error: {err:?}"
        );
    }

    #[test]
    fn unsupported_key_version_is_rejected() {
        let key = fixed_key(0x77);
        let mut sealed = seal(&key, KeyPurpose::Session, b"aad", b"payload").unwrap();
        sealed[0] = KEY_VERSION + 1;
        assert!(matches!(
            open(&key, KeyPurpose::Session, b"aad", &sealed).unwrap_err(),
            Error::UnsupportedKeyVersion { found, supported }
                if found == KEY_VERSION + 1 && supported == KEY_VERSION
        ));
    }

    #[test]
    fn overshort_frames_are_rejected_without_panicking() {
        let key = fixed_key(0x88);
        for len in 0..MIN_SEALED_LEN {
            let frame: Vec<u8> = vec![KEY_VERSION, KeyPurpose::Session.byte()]
                .into_iter()
                .chain(std::iter::repeat(0u8))
                .take(len)
                .collect();
            assert!(open(&key, KeyPurpose::Session, b"aad", &frame).is_err());
        }
    }

    #[test]
    fn nonces_are_unique_across_seals() {
        let key = fixed_key(0x99);
        let mut nonces = std::collections::HashSet::new();
        for _ in 0..512 {
            let sealed = seal(&key, KeyPurpose::Session, b"aad", b"same-plaintext").unwrap();
            assert!(
                nonces.insert(sealed[2..HEADER_LEN].to_vec()),
                "nonce reuse detected — a GCM nonce must never repeat under one key"
            );
        }
        assert_eq!(nonces.len(), 512);
    }

    #[test]
    fn derive_key_is_deterministic_and_salt_sensitive() {
        let params = fast_params();
        let a = derive_key(
            "correct horse",
            KeyPurpose::Vault,
            &[1u8; SALT_LEN],
            &params,
        )
        .unwrap();
        let b = derive_key(
            "correct horse",
            KeyPurpose::Vault,
            &[1u8; SALT_LEN],
            &params,
        )
        .unwrap();
        let c = derive_key(
            "correct horse",
            KeyPurpose::Vault,
            &[2u8; SALT_LEN],
            &params,
        )
        .unwrap();
        let d = derive_key("wrong horse", KeyPurpose::Vault, &[1u8; SALT_LEN], &params).unwrap();

        assert_eq!(
            a.expose(),
            b.expose(),
            "same inputs must derive the same key"
        );
        assert_ne!(a.expose(), c.expose(), "salt must change the key");
        assert_ne!(a.expose(), d.expose(), "passphrase must change the key");
    }

    #[test]
    fn derive_key_is_purpose_separated() {
        let params = fast_params();
        let salt = [9u8; SALT_LEN];
        let session = derive_key("pw", KeyPurpose::Session, &salt, &params).unwrap();
        let vault = derive_key("pw", KeyPurpose::Vault, &salt, &params).unwrap();
        assert_ne!(
            session.expose(),
            vault.expose(),
            "a vault key must not equal the session key for the same passphrase"
        );
    }

    #[test]
    fn derived_key_seals_and_opens() {
        let key = derive_key("pw", KeyPurpose::Vault, &[7u8; SALT_LEN], &fast_params()).unwrap();
        let sealed = seal_manifest(&key, "mig-42", b"{\"state\":\"created\"}").unwrap();
        assert_eq!(
            open_manifest(&key, "mig-42", &sealed).unwrap(),
            b"{\"state\":\"created\"}"
        );
        // A different migration id is a different AAD, so the frame will not open.
        assert!(open_manifest(&key, "mig-43", &sealed).is_err());
    }

    #[test]
    fn debug_output_never_leaks_key_material() {
        let key = fixed_key(0xAB);
        let rendered = format!("{key:?}");
        assert!(rendered.contains("redacted"), "got: {rendered}");
        assert!(!rendered.to_lowercase().contains("abab"));
        assert!(!rendered.contains("171"));
    }
}
