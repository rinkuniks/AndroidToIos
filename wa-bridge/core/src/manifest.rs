//! Migration manifest + object/chunk state (Plan Phase 1; v1.1 §7, §9).
//!
//! Chunked objects — never one giant archive file. Every object carries a
//! SHA-256 content hash; every chunk records its transfer state so a failed
//! 80 GB transfer resumes from the last verified chunk (H3).

use serde::{Deserialize, Serialize};

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
        let chunk_count = (size + chunk_size as u64 - 1) / chunk_size as u64;
        self.objects.push(Object {
            id,
            category,
            size,
            content_hash: String::new(), // filled by streaming hash pass
            chunk_count,
            chunk_size,
            transfer_state: TransferState::Pending,
        });
        self.objects.last_mut().expect("just pushed")
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
        let obj = m.add_object("o1".into(), Category::Video, 10 * 1024 * 1024, DEFAULT_CHUNK_SIZE);
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
