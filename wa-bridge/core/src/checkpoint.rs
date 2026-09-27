//! Checkpoint store — persist state after every meaningful checkpoint
//! (v1.1 §11) so interrupted transfers resume from the last **verified**
//! chunk (H3, Plan Phase 1 gate).

use crate::error::{Error, Result};
use crate::manifest::{Manifest, SessionState, TransferState};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub migration_id: String,
    pub state: SessionState,
    /// object_id -> highest contiguous verified chunk index.
    pub verified_through: std::collections::BTreeMap<String, u64>,
    /// JSON-serialized manifest snapshot.
    pub manifest_json: String,
}

impl Checkpoint {
    /// First chunk sequence still needed for `object_id` (0 when untouched).
    ///
    /// This is the resume planner: the sender must (re)start at this sequence,
    /// so already-verified chunks are never re-sent and no gap is ever skipped
    /// (H3 — "resume from the last **verified** chunk").
    pub fn resume_from(&self, object_id: &str) -> u64 {
        self.verified_through
            .get(object_id)
            .map_or(0, |last| last + 1)
    }
}

/// Highest contiguous verified chunk index in a set of received sequences.
///
/// `None` when sequence 0 has not arrived — i.e. nothing can be assumed about
/// the object's prefix, so the safe resume point is 0.
pub fn highest_contiguous(sequences: &std::collections::HashSet<u64>) -> Option<u64> {
    let mut next = 0u64;
    while sequences.contains(&next) {
        next += 1;
    }
    if next == 0 {
        None
    } else {
        Some(next - 1)
    }
}

#[derive(Debug)]
pub struct CheckpointStore {
    dir: PathBuf,
}

impl CheckpointStore {
    pub fn open(dir: impl AsRef<Path>) -> Result<Self> {
        let dir = dir.as_ref().to_path_buf();
        std::fs::create_dir_all(&dir).map_err(|e| Error::Checkpoint(e.to_string()))?;
        Ok(Self { dir })
    }

    fn path(&self, migration_id: &str) -> PathBuf {
        self.dir.join(format!("checkpoint-{migration_id}.json"))
    }

    /// Build a checkpoint from the current manifest state.
    pub fn snapshot(manifest: &Manifest) -> Checkpoint {
        let mut verified_through = std::collections::BTreeMap::new();
        for obj in &manifest.objects {
            if obj.transfer_state == TransferState::Verified {
                verified_through.insert(obj.id.clone(), obj.chunk_count.saturating_sub(1));
            }
        }
        Self::snapshot_with(manifest, verified_through)
    }

    /// Build a checkpoint from a **partial** run, recording the last verified
    /// chunk per object.
    ///
    /// This is what makes mid-object resume work: a transfer interrupted at 47%
    /// persists "object X is verified through chunk 12", so the next run starts
    /// at chunk 13 instead of re-sending the whole object (Plan Phase 1 gate, H3).
    pub fn snapshot_with(
        manifest: &Manifest,
        verified_through: std::collections::BTreeMap<String, u64>,
    ) -> Checkpoint {
        Checkpoint {
            migration_id: manifest.migration_id.clone(),
            state: manifest.state,
            verified_through,
            manifest_json: serde_json::to_string(manifest).expect("manifest serializes"),
        }
    }

    /// Atomic persist: write to temp file, fsync, rename. Survives app kill /
    /// power loss mid-write (Phase 10 chaos gate depends on this).
    pub fn save(&self, cp: &Checkpoint) -> Result<()> {
        let path = self.path(&cp.migration_id);
        let tmp = path.with_extension("json.tmp");
        let json = serde_json::to_vec_pretty(cp).map_err(|e| Error::Checkpoint(e.to_string()))?;
        {
            let mut f =
                std::fs::File::create(&tmp).map_err(|e| Error::Checkpoint(e.to_string()))?;
            f.write_all(&json)
                .map_err(|e| Error::Checkpoint(e.to_string()))?;
            f.sync_all().map_err(|e| Error::Checkpoint(e.to_string()))?;
        }
        std::fs::rename(&tmp, &path).map_err(|e| Error::Checkpoint(e.to_string()))?;
        Ok(())
    }

    pub fn load(&self, migration_id: &str) -> Result<Option<Checkpoint>> {
        let path = self.path(migration_id);
        if !path.exists() {
            return Ok(None);
        }
        let mut json = String::new();
        std::fs::File::open(&path)
            .and_then(|mut f| f.read_to_string(&mut json))
            .map_err(|e| Error::Checkpoint(e.to_string()))?;
        serde_json::from_str(&json).map_err(|e| Error::Checkpoint(e.to_string()))
    }

    /// Restore a manifest from a checkpoint, if one exists.
    pub fn restore_manifest(&self, migration_id: &str) -> Result<Option<Manifest>> {
        match self.load(migration_id)? {
            None => Ok(None),
            Some(cp) => serde_json::from_str(&cp.manifest_json)
                .map(Some)
                .map_err(|e| Error::Checkpoint(e.to_string())),
        }
    }
}

/// Streaming SHA-256 over any reader (used for object content hashes without
/// loading files into memory — required for 100 GB media).
pub fn sha256_streaming(reader: &mut impl Read) -> Result<String> {
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1024 * 1024];
    loop {
        let n = reader
            .read(&mut buf)
            .map_err(|e| Error::Checkpoint(e.to_string()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// Hash an object once, producing the whole-object digest (`Object::content_hash`)
/// **and** the per-chunk digests used for Level-1 verification and mid-object
/// resume (ADR-006).
///
/// The returned vector always has exactly one entry per chunk, matching how the
/// engine slices the object (final chunk may be short).
pub fn sha256_chunks(reader: &mut impl Read, chunk_size: u32) -> Result<(String, Vec<String>)> {
    if chunk_size == 0 {
        return Err(Error::Checkpoint("chunk size must be non-zero".into()));
    }
    let chunk_size = chunk_size as usize;
    let mut whole = Sha256::new();
    let mut per_chunk = Vec::new();
    let mut buf = vec![0u8; chunk_size];

    loop {
        let mut filled = 0usize;
        // Fill one whole chunk (or hit EOF).
        while filled < chunk_size {
            let n = reader
                .read(&mut buf[filled..])
                .map_err(|e| Error::Checkpoint(e.to_string()))?;
            if n == 0 {
                break;
            }
            filled += n;
        }
        if filled == 0 {
            break;
        }
        whole.update(&buf[..filled]);
        per_chunk.push(hex::encode(Sha256::digest(&buf[..filled])));
    }

    Ok((hex::encode(whole.finalize()), per_chunk))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::Category;

    #[test]
    fn save_load_restore_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = CheckpointStore::open(dir.path()).unwrap();

        let mut m = Manifest::new("mig1".into(), "src".into(), "dst".into());
        m.state = SessionState::Transferring;
        m.add_object("o1".into(), Category::Video, 1000, 400);
        m.objects[0].transfer_state = TransferState::Complete;

        let cp = CheckpointStore::snapshot(&m);
        store.save(&cp).unwrap();

        let loaded = store.load("mig1").unwrap().expect("checkpoint exists");
        assert_eq!(loaded.state, SessionState::Transferring);

        let restored = store.restore_manifest("mig1").unwrap().unwrap();
        assert_eq!(restored.objects[0].transfer_state, TransferState::Complete);
        assert_eq!(restored.state, SessionState::Transferring);
    }

    #[test]
    fn load_missing_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let store = CheckpointStore::open(dir.path()).unwrap();
        assert!(store.load("nope").unwrap().is_none());
        assert!(store.restore_manifest("nope").unwrap().is_none());
    }

    #[test]
    fn snapshot_with_records_partial_progress_and_resumes() {
        let mut m = Manifest::new("mig".into(), "src".into(), "dst".into());
        m.add_object("a".into(), Category::Image, 1000, 400); // 3 chunks
        let mut progress = std::collections::BTreeMap::new();
        progress.insert("a".to_string(), 0u64); // only chunk 0 verified so far

        let cp = CheckpointStore::snapshot_with(&m, progress);
        assert_eq!(
            cp.resume_from("a"),
            1,
            "resume after the last verified chunk"
        );
        assert_eq!(cp.resume_from("never-touched"), 0);
    }

    #[test]
    fn highest_contiguous_reports_the_safe_resume_point() {
        use std::collections::HashSet;
        assert_eq!(highest_contiguous(&HashSet::new()), None);
        // A hole after 2 means nothing past 2 can be trusted.
        assert_eq!(highest_contiguous(&HashSet::from([0, 1, 2, 5])), Some(2));
        // Sequence 0 missing => no verified prefix at all.
        assert_eq!(highest_contiguous(&HashSet::from([1, 2])), None);
        assert_eq!(highest_contiguous(&HashSet::from([0, 1, 2])), Some(2));
    }

    #[test]
    fn checkpoint_round_trips_partial_progress_through_disk() {
        let dir = tempfile::tempdir().unwrap();
        let store = CheckpointStore::open(dir.path()).unwrap();
        let mut m = Manifest::new("mig9".into(), "src".into(), "dst".into());
        m.add_object("a".into(), Category::Video, 10_000, 400);

        let mut progress = std::collections::BTreeMap::new();
        progress.insert("a".into(), 12u64);
        store
            .save(&CheckpointStore::snapshot_with(&m, progress))
            .unwrap();

        let loaded = store.load("mig9").unwrap().expect("checkpoint");
        assert_eq!(loaded.resume_from("a"), 13);
    }

    #[test]
    fn snapshot_marks_verified_objects() {
        let mut m = Manifest::new("mig2".into(), "src".into(), "dst".into());
        m.add_object("a".into(), Category::Image, 500, 400);
        m.add_object("b".into(), Category::Image, 500, 400);
        m.objects[1].transfer_state = TransferState::Verified;

        let cp = CheckpointStore::snapshot(&m);
        // b: 500 bytes @ 400-byte chunks => 2 chunks; fully verified => last index 1.
        assert_eq!(m.objects[1].chunk_count, 2);
        assert_eq!(cp.verified_through.get("b"), Some(&1));
        assert!(!cp.verified_through.contains_key("a"));
    }
}
