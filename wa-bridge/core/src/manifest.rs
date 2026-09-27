//! Migration manifest + object/chunk state (Plan Phase 1; v1.1 §7, §9).
//!
//! Chunked objects — never one giant archive file. Every object carries a
//! SHA-256 content hash; every chunk records its transfer state so a failed
//! 80 GB transfer resumes from the last verified chunk (H3).

use serde::{Deserialize, Serialize};

use crate::{Error, Result};

/// Migration session state machine (v1.1 §11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SessionState {
    Created,
    Preflight,
    Checkpointing,
    Transferring,
    Paused,
    Reconnecting,
    TransferVerified,
    OfficialImportRequired,
    PostVerify,
    Complete,
}

impl SessionState {
    /// Stable display name used in UI copy and diagnostics (no jargon in the
    /// user-facing layer — see Plan Phase 4/9).
    pub const fn name(self) -> &'static str {
        match self {
            SessionState::Created => "created",
            SessionState::Preflight => "preflight",
            SessionState::Checkpointing => "checkpointing",
            SessionState::Transferring => "transferring",
            SessionState::Paused => "paused",
            SessionState::Reconnecting => "reconnecting",
            SessionState::TransferVerified => "transfer_verified",
            SessionState::OfficialImportRequired => "official_import_required",
            SessionState::PostVerify => "post_verify",
            SessionState::Complete => "complete",
        }
    }

    /// Terminal states accept no further transitions.
    pub const fn is_terminal(self) -> bool {
        matches!(self, SessionState::Complete)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Image,
    Video,
    Audio,
    VoiceNote,
    Document,
    Sticker,
    Metadata,
}

/// Per-object record in the manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Object {
    pub id: String,
    pub category: Category,
    pub size: u64,
    #[serde(rename = "sha256")]
    pub content_hash: String,
    pub chunk_count: u64,
    pub chunk_size: u32,
    pub transfer_state: TransferState,
    /// Per-chunk SHA-256 (lowercase hex) in sequence order.
    ///
    /// When present these give Level-1 integrity *per chunk* (so a corrupt or
    /// replayed chunk is rejected immediately instead of silently poisoning the
    /// object) and make mid-object resume safe across processes, because the
    /// object digest no longer has to be reconstructed from one continuous
    /// stream (Plan Phase 1 gate; ADR-006). Empty on manifests written before
    /// chunk-level verification existed — those fall back to whole-object
    /// verification at completion.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chunk_hashes: Vec<String>,
}

impl Object {
    /// The expected SHA-256 of chunk `sequence`, if this manifest carries them.
    pub fn expected_chunk_hash(&self, sequence: u64) -> Option<&str> {
        self.chunk_hashes.get(sequence as usize).map(String::as_str)
    }

    /// `true` when this object can be verified chunk by chunk.
    pub fn has_chunk_hashes(&self) -> bool {
        !self.chunk_hashes.is_empty()
    }

    /// Verify the invariant every writer must keep: one hash per chunk.
    pub fn chunk_hashes_match_chunk_count(&self) -> bool {
        self.chunk_hashes.is_empty() || self.chunk_hashes.len() as u64 == self.chunk_count
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TransferState {
    Pending,
    Partial,
    Complete,
    Verified,
}

/// Top-level manifest. Serialized to `vault.manifest` / transfer manifests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub migration_id: String,
    pub source_device: String,
    pub destination_device: String,
    pub created_at: String,
    pub state: SessionState,
    pub objects: Vec<Object>,
}

/// Default chunk size (4 MiB) — tuned in Phase 1 throughput baselining.
pub const DEFAULT_CHUNK_SIZE: u32 = 4 * 1024 * 1024;

impl Manifest {
    pub fn new(migration_id: String, source_device: String, destination_device: String) -> Self {
        Self {
            migration_id,
            source_device,
            destination_device,
            created_at: humantime_now(),
            state: SessionState::Created,
            objects: Vec::new(),
        }
    }

    pub fn add_object(
        &mut self,
        id: String,
        category: Category,
        size: u64,
        chunk_size: u32,
    ) -> &mut Object {
        // div_ceil needs Rust >= 1.73; use the portable formula instead.
        let chunk_count = size.div_ceil(chunk_size as u64);
        self.objects.push(Object {
            id,
            category,
            size,
            content_hash: String::new(), // filled by streaming hash pass
            chunk_count,
            chunk_size,
            transfer_state: TransferState::Pending,
            chunk_hashes: Vec::new(), // filled by sha256_chunks when available
        });
        self.objects.last_mut().expect("just pushed")
    }

    /// Attach per-chunk hashes to an object, enforcing "one hash per chunk".
    ///
    /// Rejects a mismatched vector rather than storing an object whose
    /// Level-1 verification would be skewed by an off-by-one.
    pub fn set_chunk_hashes(&mut self, object_id: &str, hashes: Vec<String>) -> Result<()> {
        let obj = self
            .objects
            .iter_mut()
            .find(|o| o.id == object_id)
            .ok_or_else(|| Error::Manifest(format!("unknown object {object_id}")))?;

        if hashes.len() as u64 != obj.chunk_count {
            return Err(Error::Manifest(format!(
                "object {object_id} expects {} chunk hashes, got {}",
                obj.chunk_count,
                hashes.len()
            )));
        }
        obj.chunk_hashes = hashes;
        Ok(())
    }

    /// Total bytes not yet verified — drives preflight "what will actually
    /// transfer" estimate (v1.2 Screen 5).
    pub fn remaining_bytes(&self) -> u64 {
        self.objects
            .iter()
            .filter(|o| o.transfer_state != TransferState::Verified)
            .map(|o| o.size)
            .sum()
    }
}

fn humantime_now() -> String {
    // ISO-8601 via std; richer timestamp handling deferred until needed.
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunk_count_uses_div_ceil() {
        // 10 MiB object at 4 MiB chunks => 3 chunks.
        let mut m = Manifest::new("mig".into(), "src".into(), "dst".into());
        let obj = m.add_object(
            "o1".into(),
            Category::Video,
            10 * 1024 * 1024,
            DEFAULT_CHUNK_SIZE,
        );
        assert_eq!(obj.chunk_count, 3);
        assert_eq!(obj.transfer_state, TransferState::Pending);
    }

    #[test]
    fn remaining_bytes_excludes_verified() {
        let mut m = Manifest::new("mig".into(), "src".into(), "dst".into());
        m.add_object("o1".into(), Category::Image, 1000, DEFAULT_CHUNK_SIZE);
        m.add_object("o2".into(), Category::Image, 2000, DEFAULT_CHUNK_SIZE);
        assert_eq!(m.remaining_bytes(), 3000);
        m.objects[0].transfer_state = TransferState::Verified;
        assert_eq!(m.remaining_bytes(), 2000);
    }
}
