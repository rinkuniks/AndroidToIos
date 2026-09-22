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
            let mut f = std::fs::File::create(&tmp).map_err(|e| Error::Checkpoint(e.to_string()))?;
            f.write_all(&json).map_err(|e| Error::Checkpoint(e.to_string()))?;
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
        let n = reader.read(&mut buf).map_err(|e| Error::Checkpoint(e.to_string()))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
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
