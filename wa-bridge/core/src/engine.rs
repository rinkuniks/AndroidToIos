//! Chunked transfer engine (Plan Phase 1; roadmap v1.1 §7, §11, §9).
//!
//! [`Sender`] streams objects as chunked messages over any [`Transport`],
//! retrying a failed chunk (after `reconnect`) and only advancing state on a
//! transport ack — so a retry never duplicates confirmed data (H3 gate). It
//! also resumes from a per-object `resume_from` closure (checkpoint-driven).
//! [`Receiver`] reconstructs each object from chunks and verifies the streamed
//! SHA-256 of every object (v1.2 §9 Levels 1 & 2). State and object completion
//! are size-driven, so the manifest stays the single source of truth.
//!
//! Cross-process mid-object hash reconstruction is deferred (see
//! `docs/architecture/DECISIONS.md`, ADR-002). Single-process retry and
//! object-level checkpoint resume are covered by tests.

use crate::{Chunk, Error, Manifest, Object, Result, TransferState, Transport};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};

/// Reserved object id carrying the manifest frame itself (sent before any data).
pub const MANIFEST_OBJECT_ID: &str = "wa-bridge/manifest/v1";
/// Default per-chunk retry count before giving up on a failed chunk (H3).
pub const DEFAULT_MAX_RETRIES: u32 = 3;

/// Source of object bytes for the sender (Android reads from SAF/database).
pub trait MediaSource {
    fn read_at(&mut self, object: &Object, sequence: u64, buf: &mut [u8])
        -> std::io::Result<usize>;
}
/// Sink that stages received object bytes (iOS/iPadOS writes to a staging dir).
pub trait ObjectSink {
    fn write_chunk(&mut self, object_id: &str, sequence: u64, data: &[u8]) -> std::io::Result<()>;
    fn complete_object(&mut self, object_id: &str) -> std::io::Result<()>;
    /// Re-read the staged object in sequence order and confirm its SHA-256.
    ///
    /// This is Level 2 (archive integrity) verification, deliberately
    /// independent of chunk arrival order, so a *resumed* object is checked
    /// against exactly the digest a fresh transfer would produce (Plan Phase 1
    /// gate "zero silent corruption"; v1.2 §9).
    fn verify_object(&mut self, object_id: &str, expected_sha256: &str) -> std::io::Result<bool>;
}
/// Progress + checkpoint callbacks. All methods are no-ops by default.
pub trait ProgressSink {
    fn on_object_progress(&mut self, _object_id: &str, _bytes: u64, _size: u64) {}
    fn on_object_complete(&mut self, _object_id: &str) {}
    fn on_chunk_verified(&mut self, _object_id: &str, _sequence: u64) {}
    fn on_reconnect(&mut self, _object_id: &str, _sequence: u64) {}
}

/// [`ProgressSink`] that records how far each object got, so a caller can
/// persist a [`crate::checkpoint::Checkpoint`] after every verified chunk group
/// and resume from exactly that point (Plan Phase 1 gate, H3).
///
/// Both apps use this: Android wraps it in a `CheckpointStore` write, iOS feeds
/// it into its staging state. It carries no file content or secrets — only ids
/// and sequence numbers (L7).
#[derive(Debug, Default)]
pub struct ResumeTracker {
    verified_through: std::collections::BTreeMap<String, u64>,
    completed: HashSet<String>,
    reconnects: u64,
    bytes: std::collections::BTreeMap<String, u64>,
}

impl ResumeTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Last contiguous verified chunk index per object.
    pub fn verified_through(&self) -> &std::collections::BTreeMap<String, u64> {
        &self.verified_through
    }

    /// First sequence still needed for `object_id` — pass this straight into
    /// `Sender::send_objects`'s `resume_from` closure.
    pub fn resume_from(&self, object_id: &str) -> u64 {
        self.verified_through
            .get(object_id)
            .map_or(0, |last| last + 1)
    }

    /// Objects finished in this session.
    pub fn completed(&self) -> usize {
        self.completed.len()
    }

    /// Bytes observed per object (diagnostics / ETA only).
    pub fn bytes_seen(&self, object_id: &str) -> u64 {
        self.bytes.get(object_id).copied().unwrap_or(0)
    }

    pub fn reconnects(&self) -> u64 {
        self.reconnects
    }

    /// Snapshot the current progress into a persistable checkpoint.
    pub fn checkpoint(&self, manifest: &Manifest) -> crate::checkpoint::Checkpoint {
        crate::checkpoint::CheckpointStore::snapshot_with(manifest, self.verified_through.clone())
    }
}

impl ProgressSink for ResumeTracker {
    fn on_chunk_verified(&mut self, object_id: &str, sequence: u64) {
        let last = self
            .verified_through
            .entry(object_id.to_string())
            .or_insert(sequence);
        if sequence > *last {
            *last = sequence;
        }
    }

    fn on_object_progress(&mut self, object_id: &str, bytes: u64, _size: u64) {
        self.bytes.insert(object_id.to_string(), bytes);
    }

    fn on_object_complete(&mut self, object_id: &str) {
        self.completed.insert(object_id.to_string());
    }

    fn on_reconnect(&mut self, _object_id: &str, _sequence: u64) {
        self.reconnects += 1;
    }
}

/// `true` for transport/engine errors considered recoverable via reconnect.
fn is_retryable(error: &Error) -> bool {
    matches!(error, Error::Transport(_) | Error::Engine(_))
}

/// Chunked sender.
pub struct Sender<'a, T: Transport + ?Sized> {
    transport: &'a mut T,
}

impl<'a, T: Transport + ?Sized> Sender<'a, T> {
    pub fn new(transport: &'a mut T) -> Self {
        Self { transport }
    }

    /// Emit the manifest frame first (vault-before-transfer ordering, L6).
    pub async fn send_manifest(&mut self, manifest: &Manifest) -> Result<()> {
        let json = serde_json::to_vec(manifest).map_err(|e| Error::Manifest(e.to_string()))?;
        self.transport
            .send_chunk(Chunk {
                object_id: MANIFEST_OBJECT_ID.to_string(),
                sequence: 0,
                data: json,
            })
            .await
            .map(|_ack| ())
            .map_err(|e| Error::Transport(e.to_string()))
    }

    /// Send every object the receiver has not yet verified.
    ///
    /// `resume_from` returns the first sequence to (re)send per object (0 on a
    /// fresh run; `last_verified + 1` after a restart). State only advances
    /// after a transport ack, so a retry never duplicates confirmed data.
    pub async fn send_objects(
        &mut self,
        manifest: &Manifest,
        source: &mut dyn MediaSource,
        progress: &mut dyn ProgressSink,
        max_retries: u32,
        resume_from: &dyn Fn(&Object) -> u64,
    ) -> Result<()> {
        for obj in &manifest.objects {
            let start = resume_from(obj);
            if start >= obj.chunk_count || obj.transfer_state == TransferState::Verified {
                progress.on_object_complete(&obj.id);
                continue;
            }
            let chunk_size = obj.chunk_size as u64;
            let mut sequence = start;
            let mut offset = sequence * chunk_size;
            let mut remaining = obj.size.saturating_sub(offset);
            // Whole-object digest is accumulated only for legacy manifests that
            // carry no per-chunk hashes (ADR-006), and only from sequence 0.
            let mut hasher = if !obj.has_chunk_hashes() && sequence == 0 {
                Some(Sha256::new())
            } else {
                None
            };
            loop {
                if remaining == 0 {
                    break;
                }
                let want = (chunk_size.min(remaining)) as usize;
                let mut buf = vec![0u8; want];
                let n = source
                    .read_at(obj, sequence, &mut buf)
                    .map_err(|e| Error::SourceRead {
                        object_id: obj.id.clone(),
                        source: e,
                    })?;
                if n == 0 {
                    return Err(Error::SizeMismatch {
                        object_id: obj.id.clone(),
                    });
                }
                // Level 1 (sender side): never ship a chunk whose bytes no
                // longer match the inventory — catches source mutation
                // mid-transfer instead of corrupting the destination (L8).
                if let Some(expected) = obj.expected_chunk_hash(sequence) {
                    let actual = hex::encode(Sha256::digest(&buf[..n]));
                    if actual != expected {
                        return Err(Error::IntegrityMismatch {
                            object_id: obj.id.clone(),
                            chunk_index: sequence,
                        });
                    }
                }
                if let Some(h) = hasher.as_mut() {
                    h.update(&buf[..n]);
                }
                let chunk = Chunk {
                    object_id: obj.id.clone(),
                    sequence,
                    data: buf[..n].to_vec(),
                };
                let mut retries = 0u32;
                loop {
                    match self.transport.send_chunk(chunk.clone()).await {
                        Ok(_ack) => {
                            sequence += 1;
                            offset += n as u64;
                            remaining -= n as u64;
                            progress.on_object_progress(&obj.id, offset, obj.size);
                            progress.on_chunk_verified(&obj.id, sequence - 1);
                            break;
                        }
                        Err(e) => {
                            let err = Error::Transport(e.to_string());
                            if is_retryable(&err) && retries < max_retries {
                                retries += 1;
                                self.transport
                                    .reconnect()
                                    .await
                                    .map_err(|e2| Error::Transport(e2.to_string()))?;
                                progress.on_reconnect(&obj.id, sequence);
                                continue;
                            }
                            return Err(err);
                        }
                    }
                }
            }
            if remaining != 0 {
                return Err(Error::SizeMismatch {
                    object_id: obj.id.clone(),
                });
            }
            if let Some(h) = hasher {
                let digest = hex::encode(h.clone().finalize());
                if digest != obj.content_hash {
                    return Err(Error::HashMismatch {
                        object_id: obj.id.clone(),
                    });
                }
            }
            progress.on_object_complete(&obj.id);
        }
        Ok(())
    }
}
/// Per-object set of chunk sequences a receiver already holds (its own
/// checkpoint). Seeding this is what lets a receiver *finish* an object that the
/// sender only partially re-sends after a resume, instead of waiting forever for
/// chunks that will never arrive (Plan Phase 1 gate, H3).
pub type ReceivedIndex = HashMap<String, HashSet<u64>>;

/// Chunked receiver with streaming SHA-256 verification (v1.2 §9 Levels 1 & 2).
pub struct Receiver<'a, T: Transport + ?Sized> {
    transport: &'a mut T,
    max_retries: u32,
}

impl<'a, T: Transport + ?Sized> Receiver<'a, T> {
    pub fn new(transport: &'a mut T) -> Self {
        Self {
            transport,
            max_retries: DEFAULT_MAX_RETRIES,
        }
    }

    /// Override the reconnect/retry budget used when a link drops mid-transfer.
    pub fn with_max_retries(mut self, max_retries: u32) -> Self {
        self.max_retries = max_retries;
        self
    }

    /// Read the next chunk, reconnecting on a recoverable transport failure just
    /// like the sender does — so a Wi-Fi blip or a router restart does not end
    /// the session while chunks are still in flight (Phase 4 behaviour).
    async fn next_chunk(&mut self, progress: &mut dyn ProgressSink) -> Result<Chunk> {
        let mut attempt = 0u32;
        loop {
            match self.transport.receive_chunk().await {
                Ok(chunk) => return Ok(chunk),
                Err(e) => {
                    let err = Error::Transport(e.to_string());
                    if is_retryable(&err) && attempt < self.max_retries {
                        attempt += 1;
                        self.transport
                            .reconnect()
                            .await
                            .map_err(|e2| Error::Transport(e2.to_string()))?;
                        progress.on_reconnect("", 0);
                        continue;
                    }
                    return Err(err);
                }
            }
        }
    }

    /// Read the manifest frame emitted by [`Sender::send_manifest`].
    pub async fn receive_manifest(&mut self) -> Result<Manifest> {
        let chunk = self
            .transport
            .receive_chunk()
            .await
            .map_err(|e| Error::Transport(e.to_string()))?;
        if chunk.object_id != MANIFEST_OBJECT_ID || chunk.sequence != 0 {
            return Err(Error::Engine(format!(
                "expected manifest frame, got {}@{}",
                chunk.object_id, chunk.sequence
            )));
        }
        serde_json::from_slice(&chunk.data).map_err(|e| Error::Manifest(e.to_string()))
    }

    /// Receive objects until every manifest object is verified.
    ///
    /// Convenience wrapper for a receiver starting a fresh session; see
    /// [`Receiver::receive_objects_from`] to continue a resumed one.
    pub async fn receive_objects(
        &mut self,
        manifest: &Manifest,
        sink: &mut dyn ObjectSink,
        progress: &mut dyn ProgressSink,
    ) -> Result<()> {
        let mut received = ReceivedIndex::new();
        self.receive_objects_from(manifest, sink, progress, &mut received)
            .await
    }

    /// Receive objects, seeded with the chunk sequences this receiver already
    /// staged in a previous run.
    ///
    /// Completion is driven by **chunk indexes**, not by bytes seen, so a
    /// sender that resumes mid-object (starting at sequence N) still completes
    /// the object. Every chunk is verified against the manifest's per-chunk
    /// hash (Level 1) before it is staged, and the finished object is re-hashed
    /// in sequence order (Level 2) so reordered or resumed delivery can never
    /// slip corruption through.
    pub async fn receive_objects_from(
        &mut self,
        manifest: &Manifest,
        sink: &mut dyn ObjectSink,
        progress: &mut dyn ProgressSink,
        received: &mut ReceivedIndex,
    ) -> Result<()> {
        let total = manifest.objects.len() as u32;
        let mut bytes: HashMap<String, u64> = HashMap::new();
        let mut completed: u32 = 0;

        // Pre-mark objects already verified in the manifest (resume skip).
        for obj in &manifest.objects {
            if obj.transfer_state == TransferState::Verified {
                sink.complete_object(&obj.id)
                    .map_err(|e| Error::SourceRead {
                        object_id: obj.id.clone(),
                        source: e,
                    })?;
                completed += 1;
                progress.on_object_complete(&obj.id);
            }
        }

        while completed < total {
            let chunk = self.next_chunk(progress).await?;
            let object_id = chunk.object_id.clone();
            let obj = manifest
                .objects
                .iter()
                .find(|o| o.id == object_id)
                .ok_or_else(|| Error::Engine(format!("unknown object {object_id}")))?;

            if obj.transfer_state == TransferState::Verified {
                continue;
            }
            if chunk.sequence >= obj.chunk_count {
                return Err(Error::Engine(format!(
                    "chunk {}/{} is out of range for object {object_id}",
                    chunk.sequence, obj.chunk_count
                )));
            }

            // Level 1: verify the chunk against the inventory *before* staging.
            if let Some(expected) = obj.expected_chunk_hash(chunk.sequence) {
                let actual = hex::encode(Sha256::digest(&chunk.data));
                if actual != expected {
                    return Err(Error::IntegrityMismatch {
                        object_id,
                        chunk_index: chunk.sequence,
                    });
                }
            }

            // Re-delivered chunks (retry after reconnect) are dropped, so the
            // index stays the single source of truth for completion.
            if !received
                .entry(object_id.clone())
                .or_default()
                .insert(chunk.sequence)
            {
                continue;
            }

            sink.write_chunk(&object_id, chunk.sequence, &chunk.data)
                .map_err(|e| Error::SourceRead {
                    object_id: object_id.clone(),
                    source: e,
                })?;

            let seen = {
                let e = bytes.entry(object_id.clone()).or_insert(0);
                *e += chunk.data.len() as u64;
                *e
            };
            progress.on_object_progress(&object_id, seen, obj.size);

            let have = received.get(&object_id).map_or(0, |s| s.len()) as u64;
            if have == obj.chunk_count {
                // Level 2: the staged object must match the source digest no
                // matter when or in what order its chunks arrived.
                let ok = sink
                    .verify_object(&object_id, &obj.content_hash)
                    .map_err(|e| Error::SourceRead {
                        object_id: object_id.clone(),
                        source: e,
                    })?;
                if !ok {
                    return Err(Error::HashMismatch { object_id });
                }
                received.remove(&object_id);
                bytes.remove(&object_id);
                sink.complete_object(&object_id)
                    .map_err(|e| Error::SourceRead {
                        object_id: object_id.clone(),
                        source: e,
                    })?;
                completed += 1;
                progress.on_object_complete(&object_id);
            }
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::Category;
    use crate::transport::{FaultHandle, PeerPlatform};
    use crate::{checkpoint, loopback_pair, PeerInfo};
    use std::collections::HashMap;
    use std::io::Cursor;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::Arc;

    fn peer(name: &str, platform: PeerPlatform) -> PeerInfo {
        PeerInfo {
            device_name: name.into(),
            platform,
            addresses: vec!["127.0.0.1".into()],
        }
    }

    fn gen_bytes(id: &str, size: u64) -> Vec<u8> {
        let b = id.as_bytes();
        (0..size).map(|i| b[(i as usize) % b.len()]).collect()
    }

    fn build(chunk_size: u32) -> (Manifest, MemorySource, HashMap<String, u32>) {
        let specs = [
            ("o0", Category::Image, 3000u64),
            ("o1", Category::Video, 7000),
            ("o2", Category::Image, 512),
        ];
        let mut m = Manifest::new("e2e".into(), "android".into(), "ios".into());
        let mut data = HashMap::new();
        let mut cs = HashMap::new();
        for (id, cat, size) in specs {
            let bytes = gen_bytes(id, size);
            let hash = checkpoint::sha256_streaming(&mut Cursor::new(&bytes)).unwrap();
            let o = m.add_object(id.into(), cat, size, chunk_size);
            o.content_hash = hash;
            data.insert(id.into(), bytes);
            cs.insert(id.into(), chunk_size);
        }
        (m, MemorySource { data }, cs)
    }

    #[derive(Clone)]
    struct MemorySource {
        data: HashMap<String, Vec<u8>>,
    }
    impl MediaSource for MemorySource {
        fn read_at(
            &mut self,
            object: &Object,
            sequence: u64,
            buf: &mut [u8],
        ) -> std::io::Result<usize> {
            let all = self.data.get(&object.id).ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::NotFound, object.id.clone())
            })?;
            let off = (sequence as usize) * (object.chunk_size as usize);
            if off >= all.len() {
                return Ok(0);
            }
            let end = (off + buf.len()).min(all.len());
            buf[..end - off].copy_from_slice(&all[off..end]);
            Ok(end - off)
        }
    }

    struct VecSink {
        staged: HashMap<String, Vec<u8>>,
        cs: HashMap<String, u32>,
    }
    impl ObjectSink for VecSink {
        fn write_chunk(
            &mut self,
            object_id: &str,
            sequence: u64,
            data: &[u8],
        ) -> std::io::Result<()> {
            let cs = self.cs.get(object_id).copied().unwrap_or(0) as usize;
            let off = (sequence as usize) * cs;
            let need = off + data.len();
            let v = self.staged.entry(object_id.to_string()).or_default();
            if v.len() < need {
                v.resize(need, 0);
            }
            v[off..need].copy_from_slice(data);
            Ok(())
        }
        fn complete_object(&mut self, _object_id: &str) -> std::io::Result<()> {
            Ok(())
        }

        fn verify_object(
            &mut self,
            object_id: &str,
            expected_sha256: &str,
        ) -> std::io::Result<bool> {
            let staged = match self.staged.get(object_id) {
                Some(s) => s,
                None => return Ok(false),
            };
            // Chunks are written by offset, so the buffer is in sequence order
            // no matter what order they arrived in.
            let actual = checkpoint::sha256_streaming(&mut Cursor::new(staged))
                .map_err(|e| std::io::Error::other(e.to_string()))?;
            Ok(actual == expected_sha256)
        }
    }

    struct NullProgress;
    impl ProgressSink for NullProgress {}

    #[tokio::test]
    async fn manifest_is_first_frame() {
        let (mut a, mut b, _fh) = loopback_pair(
            peer("A", PeerPlatform::Android),
            peer("B", PeerPlatform::Iphone),
        );
        let (manifest, _src, _cs) = build(512);
        Sender::new(&mut a).send_manifest(&manifest).await.unwrap();
        let chunk = b.receive_chunk().await.unwrap();
        assert_eq!(chunk.object_id, MANIFEST_OBJECT_ID);
        assert_eq!(chunk.sequence, 0);
        let decoded: Manifest = serde_json::from_slice(&chunk.data).unwrap();
        assert_eq!(decoded.migration_id, "e2e");
    }

    #[tokio::test]
    async fn happy_path_transfers_and_verifies_all() {
        let (mut a, mut b, _fh) = loopback_pair(
            peer("A", PeerPlatform::Android),
            peer("B", PeerPlatform::Iphone),
        );
        let (manifest, mut src, cs) = build(512);
        let mut sink = VecSink {
            staged: HashMap::new(),
            cs,
        };
        let mut sp = NullProgress;
        let mut rp = NullProgress;

        let (sres, rres) = tokio::join!(
            async {
                let mut s = Sender::new(&mut a);
                s.send_manifest(&manifest).await?;
                s.send_objects(&manifest, &mut src, &mut sp, DEFAULT_MAX_RETRIES, &|_| 0)
                    .await
            },
            async {
                let mut r = Receiver::new(&mut b);
                let m = r.receive_manifest().await?;
                assert_eq!(m.migration_id, manifest.migration_id);
                r.receive_objects(&m, &mut sink, &mut rp).await
            },
        );
        assert!(sres.is_ok(), "sender: {:?}", sres);
        assert!(rres.is_ok(), "receiver: {:?}", rres);

        for obj in &manifest.objects {
            let staged = sink.staged.get(&obj.id).expect("object staged");
            assert_eq!(staged.len() as u64, obj.size);
            let hash = checkpoint::sha256_streaming(&mut Cursor::new(staged)).unwrap();
            assert_eq!(hash, obj.content_hash, "hash mismatch {}", obj.id);
        }
    }

    #[tokio::test]
    async fn link_cut_triggers_reconnect_and_succeeds() {
        let (mut a, mut b, fault) = loopback_pair(
            peer("A", PeerPlatform::Android),
            peer("B", PeerPlatform::Iphone),
        );
        let (manifest, mut src, cs) = build(512);
        let mut sink = VecSink {
            staged: HashMap::new(),
            cs,
        };
        let cut_after = Arc::new(AtomicU64::new(0));
        let reconnects = Arc::new(AtomicU64::new(0));

        struct CutProgress {
            cut_after: Arc<AtomicU64>,
            reconnects: Arc<AtomicU64>,
            fault: FaultHandle,
        }
        impl ProgressSink for CutProgress {
            fn on_chunk_verified(&mut self, _id: &str, _seq: u64) {
                if self.cut_after.fetch_add(1, Ordering::SeqCst) == 0 {
                    self.fault.cut();
                }
            }
            fn on_reconnect(&mut self, _id: &str, _seq: u64) {
                self.reconnects.fetch_add(1, Ordering::SeqCst);
            }
        }

        let mut sp = CutProgress {
            cut_after: cut_after.clone(),
            reconnects: reconnects.clone(),
            fault: fault.clone(),
        };
        let mut rp = NullProgress;

        let (sres, rres) = tokio::join!(
            async {
                let mut s = Sender::new(&mut a);
                s.send_manifest(&manifest).await?;
                s.send_objects(&manifest, &mut src, &mut sp, DEFAULT_MAX_RETRIES, &|_| 0)
                    .await
            },
            async {
                let mut r = Receiver::new(&mut b);
                let m = r.receive_manifest().await?;
                r.receive_objects(&m, &mut sink, &mut rp).await
            },
        );
        assert!(sres.is_ok(), "sender: {:?}", sres);
        assert!(rres.is_ok(), "receiver: {:?}", rres);
        assert!(cut_after.load(Ordering::SeqCst) >= 1, "link never cut");
        assert!(reconnects.load(Ordering::SeqCst) >= 1, "no reconnect");
        assert!(!fault.is_cut(), "link should be restored");

        for obj in &manifest.objects {
            let staged = sink.staged.get(&obj.id).unwrap();
            let hash = checkpoint::sha256_streaming(&mut Cursor::new(staged)).unwrap();
            assert_eq!(hash, obj.content_hash, "hash mismatch {}", obj.id);
        }
    }

    /// Manifest whose objects carry per-chunk hashes (the ADR-006 fast path), so
    /// tests exercise real Level-1 chunk verification.
    fn build_with_chunk_hashes(chunk_size: u32) -> (Manifest, MemorySource, HashMap<String, u32>) {
        let specs = [
            ("o0", Category::Image, 3000u64),
            ("o1", Category::Video, 7000),
            ("o2", Category::Image, 512),
        ];
        let mut m = Manifest::new("e2e".into(), "android".into(), "ios".into());
        let mut data = HashMap::new();
        let mut cs = HashMap::new();
        for (id, cat, size) in specs {
            let bytes = gen_bytes(id, size);
            let (whole, per_chunk) =
                checkpoint::sha256_chunks(&mut Cursor::new(&bytes), chunk_size).unwrap();
            let o = m.add_object(id.into(), cat, size, chunk_size);
            o.content_hash = whole;
            assert!(
                o.chunk_hashes_match_chunk_count(),
                "exactly one hash per chunk for {id}"
            );
            m.set_chunk_hashes(id, per_chunk).unwrap();
            data.insert(id.into(), bytes);
            cs.insert(id.into(), chunk_size);
        }
        (m, MemorySource { data }, cs)
    }

    /// The bug this covers: completion used to be byte-driven, so a sender
    /// resuming mid-object left the receiver waiting forever for chunks it never
    /// re-sends. Completion is now chunk-index driven, and the seeded index
    /// closes the object. (Against the old code this test hangs, not fails.)
    #[tokio::test]
    async fn mid_object_resume_completes_instead_of_hanging() {
        let (mut a, mut b, _fh) = loopback_pair(
            peer("A", PeerPlatform::Android),
            peer("B", PeerPlatform::Ipad),
        );
        let (manifest, mut src, cs) = build_with_chunk_hashes(1024);
        let target = "o1"; // 7000 bytes @ 1024 => 7 chunks
        let obj = manifest.objects.iter().find(|o| o.id == target).unwrap();
        assert_eq!(obj.chunk_count, 7);

        const RESUME_AT: u64 = 3;
        let already_staged = src.data.get(target).unwrap().clone();

        // The receiver's previous run staged chunks 0..RESUME_AT, and its
        // checkpoint knows exactly which sequences it holds.
        let mut sink = VecSink {
            staged: HashMap::new(),
            cs: cs.clone(),
        };
        let mut seed = ReceivedIndex::new();
        for seq in 0..RESUME_AT {
            let start = (seq as usize) * 1024;
            let end = (start + 1024).min(already_staged.len());
            sink.write_chunk(target, seq, &already_staged[start..end])
                .unwrap();
            seed.entry(target.to_string()).or_default().insert(seq);
        }

        let (sres, rres) = tokio::join!(
            async {
                let mut s = Sender::new(&mut a);
                s.send_manifest(&manifest).await?;
                s.send_objects(
                    &manifest,
                    &mut src,
                    &mut NullProgress,
                    DEFAULT_MAX_RETRIES,
                    &|o| if o.id == target { RESUME_AT } else { 0 },
                )
                .await
            },
            async {
                let mut r = Receiver::new(&mut b);
                let m = r.receive_manifest().await?;
                r.receive_objects_from(&m, &mut sink, &mut NullProgress, &mut seed)
                    .await
            },
        );
        assert!(sres.is_ok(), "sender: {sres:?}");
        assert!(rres.is_ok(), "receiver: {rres:?}");

        // Zero silent corruption: every object, resumed or fresh, hash-verifies.
        for obj in &manifest.objects {
            let staged = sink.staged.get(&obj.id).expect("object staged");
            assert_eq!(staged.len() as u64, obj.size, "size for {}", obj.id);
            let hash = checkpoint::sha256_streaming(&mut Cursor::new(staged)).unwrap();
            assert_eq!(hash, obj.content_hash, "resumed hash for {}", obj.id);
        }
    }

    /// Level 1 on the wire: a corrupted chunk is refused *before* it is staged,
    /// so corruption can never be silently written (v1.2 §9).
    #[tokio::test]
    async fn receiver_rejects_a_corrupted_chunk() {
        let (mut a, mut b, _fh) = loopback_pair(
            peer("A", PeerPlatform::Android),
            peer("B", PeerPlatform::Iphone),
        );
        let (manifest, src, cs) = build_with_chunk_hashes(1024);
        let target = manifest.objects[0].id.clone();
        let mut corrupt = src.data.get(&target).unwrap().clone();
        corrupt[0] ^= 0xFF; // one flipped byte in the first chunk

        let mut sink = VecSink {
            staged: HashMap::new(),
            cs: cs.clone(),
        };
        let mut seed = ReceivedIndex::new();

        let (sent, rres) = tokio::join!(
            async {
                let mut s = Sender::new(&mut a);
                s.send_manifest(&manifest).await?;
                s.transport
                    .send_chunk(Chunk {
                        object_id: target.clone(),
                        sequence: 0,
                        data: corrupt[..1024].to_vec(),
                    })
                    .await?;
                Ok::<(), Error>(())
            },
            async {
                let mut r = Receiver::new(&mut b);
                let m = r.receive_manifest().await?;
                r.receive_objects_from(&m, &mut sink, &mut NullProgress, &mut seed)
                    .await
            },
        );
        assert!(sent.is_ok(), "sender: {sent:?}");
        match rres {
            Err(Error::IntegrityMismatch {
                object_id,
                chunk_index,
            }) => {
                assert_eq!(object_id, target);
                assert_eq!(chunk_index, 0);
            }
            other => panic!("expected IntegrityMismatch, got {other:?}"),
        }
        // The corrupt chunk must never have reached the staging area.
        assert!(!sink.staged.contains_key(&target));
    }

    /// Level 1 on the sender: if the source bytes change after the inventory
    /// hash was taken, the sender stops instead of corrupting the destination
    /// (L8 — never damage or misrepresent source data).
    #[tokio::test]
    async fn sender_detects_source_mutation_mid_transfer() {
        let (mut a, _b, _fh) = loopback_pair(
            peer("A", PeerPlatform::Android),
            peer("B", PeerPlatform::Iphone),
        );
        let (manifest, mut src, _cs) = build_with_chunk_hashes(1024);
        let target = manifest.objects[0].id.clone();

        // The "file on the device" changes after the inventory pass.
        src.data.get_mut(&target).unwrap()[0] ^= 0xFF;

        let mut s = Sender::new(&mut a);
        s.send_manifest(&manifest).await.unwrap();
        let res = s
            .send_objects(
                &manifest,
                &mut src,
                &mut NullProgress,
                DEFAULT_MAX_RETRIES,
                &|_| 0,
            )
            .await;

        match res {
            Err(Error::IntegrityMismatch {
                object_id,
                chunk_index,
            }) => {
                assert_eq!(object_id, target);
                assert_eq!(chunk_index, 0);
            }
            other => panic!("expected IntegrityMismatch, got {other:?}"),
        }
    }

    /// Level 2 is not a no-op: a manifest whose whole-object digest is wrong
    /// (while every chunk hash is correct) must still be rejected at completion.
    #[tokio::test]
    async fn level_two_rejects_a_wrong_object_digest() {
        let (mut a, mut b, _fh) = loopback_pair(
            peer("A", PeerPlatform::Android),
            peer("B", PeerPlatform::Ipad),
        );
        let (mut manifest, mut src, cs) = build_with_chunk_hashes(1024);
        manifest.objects[0].content_hash = "00".repeat(32);

        let mut sink = VecSink {
            staged: HashMap::new(),
            cs: cs.clone(),
        };
        let mut seed = ReceivedIndex::new();

        let (sres, rres) = tokio::join!(
            async {
                let mut s = Sender::new(&mut a);
                s.send_manifest(&manifest).await?;
                s.send_objects(
                    &manifest,
                    &mut src,
                    &mut NullProgress,
                    DEFAULT_MAX_RETRIES,
                    &|_| 0,
                )
                .await
            },
            async {
                let mut r = Receiver::new(&mut b);
                let m = r.receive_manifest().await?;
                r.receive_objects_from(&m, &mut sink, &mut NullProgress, &mut seed)
                    .await
            },
        );
        assert!(sres.is_ok(), "sender: {sres:?}");
        assert!(
            matches!(rres, Err(Error::HashMismatch { .. })),
            "expected HashMismatch, got {rres:?}"
        );
    }

    /// The tracker must produce a checkpoint that a *later process* can resume
    /// from: fully verified objects point past their last chunk, and the
    /// snapshot survives a disk round-trip.
    #[tokio::test]
    async fn resume_tracker_builds_a_resumable_checkpoint() {
        let (mut a, mut b, _fh) = loopback_pair(
            peer("A", PeerPlatform::Android),
            peer("B", PeerPlatform::Ipad),
        );
        let (manifest, mut src, cs) = build_with_chunk_hashes(512);
        let mut sink = VecSink {
            staged: HashMap::new(),
            cs,
        };
        let mut tracker = ResumeTracker::new();

        let (sres, rres) = tokio::join!(
            async {
                let mut s = Sender::new(&mut a);
                s.send_manifest(&manifest).await?;
                s.send_objects(
                    &manifest,
                    &mut src,
                    &mut tracker,
                    DEFAULT_MAX_RETRIES,
                    &|_| 0,
                )
                .await
            },
            async {
                let mut r = Receiver::new(&mut b);
                let m = r.receive_manifest().await?;
                r.receive_objects(&m, &mut sink, &mut NullProgress).await
            },
        );
        assert!(sres.is_ok(), "sender: {sres:?}");
        assert!(rres.is_ok(), "receiver: {rres:?}");

        assert_eq!(tracker.completed(), manifest.objects.len());
        for obj in &manifest.objects {
            assert_eq!(
                tracker.resume_from(&obj.id),
                obj.chunk_count,
                "{} is fully verified, so nothing should be re-sent",
                obj.id
            );
        }

        let dir = tempfile::tempdir().unwrap();
        let store = checkpoint::CheckpointStore::open(dir.path()).unwrap();
        store.save(&tracker.checkpoint(&manifest)).unwrap();

        let restored = store
            .load(&manifest.migration_id)
            .unwrap()
            .expect("checkpoint persisted");
        for obj in &manifest.objects {
            assert_eq!(restored.resume_from(&obj.id), obj.chunk_count);
        }
        // The manifest comes back too, so a restarted app can rebuild its UI.
        let restored_manifest = store
            .restore_manifest(&manifest.migration_id)
            .unwrap()
            .unwrap();
        assert_eq!(restored_manifest.objects.len(), manifest.objects.len());
    }
}
