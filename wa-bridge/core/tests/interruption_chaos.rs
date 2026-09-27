//! Interruption / chaos harness (Plan Phase 1 gate; risks H3, C2).
//!
//! Real files on both sides, not in-memory buffers: the sender reads from a
//! source directory, the receiver stages into a staging directory, and the whole
//! thing is interrupted with a *permanent* transport failure (the peer is gone —
//! app killed, device slept, cable yanked) at 25%, 50% and 90% of the transfer.
//!
//! Nothing is allowed through on trust. After every interruption the run is
//! resumed from the last **verified** chunk and must produce byte-identical,
//! hash-verified objects — the Phase 1 exit gate "zero silent corruption: every
//! resumed object hash-verifies".
//!
//! Run:  cargo test --test interruption_chaos

use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use async_trait::async_trait;

use wa_bridge_core::checkpoint::{self, CheckpointStore};
use wa_bridge_core::engine::{
    MediaSource, ObjectSink, ProgressSink, ReceivedIndex, Receiver, ResumeTracker, Sender,
    DEFAULT_MAX_RETRIES,
};
use wa_bridge_core::manifest::{Category, Manifest, SessionState, TransferState};
use wa_bridge_core::transport::{
    Chunk, FaultHandle, LinkHealth, PeerInfo, PeerPlatform, Transport, VerifiedChunk,
};
use wa_bridge_core::{loopback_pair, Error, Object, Result};

/// Deterministic blob so runs are reproducible without fixtures.
fn blob(seed: &str, size: usize) -> Vec<u8> {
    let key = seed.as_bytes();
    (0..size)
        .map(|i| {
            key[i % key.len()]
                .wrapping_mul(31)
                .wrapping_add((i % 251) as u8)
        })
        .collect()
}

fn peer(name: &str, platform: PeerPlatform) -> PeerInfo {
    PeerInfo {
        device_name: name.into(),
        platform,
        addresses: vec!["127.0.0.1".into()],
    }
}

fn read_all(path: &Path) -> Vec<u8> {
    let mut f = fs::File::open(path).expect("open");
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).expect("read");
    buf
}

/// Reads object bytes straight off disk — one open per chunk, like SAF/MediaStore
/// access on a real Android device.
struct FileSource {
    paths: HashMap<String, PathBuf>,
}

impl MediaSource for FileSource {
    fn read_at(
        &mut self,
        object: &Object,
        sequence: u64,
        buf: &mut [u8],
    ) -> std::io::Result<usize> {
        let path = self
            .paths
            .get(&object.id)
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, object.id.clone()))?;
        let mut f = fs::File::open(path)?;
        f.seek(SeekFrom::Start(sequence * object.chunk_size as u64))?;
        let mut total = 0usize;
        while total < buf.len() {
            let n = f.read(&mut buf[total..])?;
            if n == 0 {
                break;
            }
            total += n;
        }
        Ok(total)
    }
}

/// Stages chunks into real files by offset, then verifies them by re-reading the
/// finished file — Level 2 integrity against the bytes actually on disk.
struct FileSink {
    dir: PathBuf,
    chunk_sizes: HashMap<String, u32>,
    have: HashMap<String, HashSet<u64>>,
}

impl FileSink {
    fn new(dir: impl AsRef<Path>, chunk_sizes: HashMap<String, u32>) -> Self {
        let dir = dir.as_ref().to_path_buf();
        fs::create_dir_all(&dir).expect("staging dir");
        Self {
            dir,
            chunk_sizes,
            have: HashMap::new(),
        }
    }

    fn path_for(&self, object_id: &str) -> PathBuf {
        self.dir.join(object_id.replace(['/', '\\'], "_"))
    }

    /// Chunks staged for `object_id`.
    fn staged(&self, object_id: &str) -> usize {
        self.have.get(object_id).map_or(0, HashSet::len)
    }

    /// Contiguous verified prefix per object — the receiver's checkpoint, and
    /// the authority for where a resume must restart.
    fn progress(&self) -> BTreeMap<String, u64> {
        self.have
            .iter()
            .filter_map(|(id, set)| {
                checkpoint::highest_contiguous(set).map(|last| (id.clone(), last))
            })
            .collect()
    }
}

impl ObjectSink for FileSink {
    fn write_chunk(&mut self, object_id: &str, sequence: u64, data: &[u8]) -> std::io::Result<()> {
        let cs = *self.chunk_sizes.get(object_id).unwrap_or(&0) as u64;
        let path = self.path_for(object_id);
        let mut f = fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)?;
        f.seek(SeekFrom::Start(sequence * cs))?;
        f.write_all(data)?;
        f.sync_all()?;
        self.have
            .entry(object_id.to_string())
            .or_default()
            .insert(sequence);
        Ok(())
    }

    fn complete_object(&mut self, object_id: &str) -> std::io::Result<()> {
        let path = self.path_for(object_id);
        if path.exists() {
            fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(path)?
                .sync_all()?;
        }
        Ok(())
    }

    fn verify_object(&mut self, object_id: &str, expected_sha256: &str) -> std::io::Result<bool> {
        let path = self.path_for(object_id);
        if !path.exists() {
            return Ok(false);
        }
        let mut f = fs::File::open(path)?;
        let actual = checkpoint::sha256_streaming(&mut f)
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        Ok(actual == expected_sha256)
    }
}

/// Transport wrapper that fails **permanently** once thrown, including
/// `reconnect` — "the peer is gone" (app killed, device asleep, cable yanked).
/// This is the interruption a resumed run has to survive.
struct KillSwitch<T: Transport> {
    inner: T,
    killed: Arc<AtomicU64>,
}

impl<T: Transport> KillSwitch<T> {
    fn is_dead(&self) -> bool {
        self.killed.load(Ordering::SeqCst) > 0
    }

    fn down(&self) -> Error {
        Error::Transport("peer gone".into())
    }
}

#[async_trait]
impl<T: Transport> Transport for KillSwitch<T> {
    async fn discover(&mut self) -> Result<Vec<PeerInfo>> {
        self.inner.discover().await
    }
    async fn authenticate(&mut self, peer: &PeerInfo) -> Result<()> {
        if self.is_dead() {
            return Err(self.down());
        }
        self.inner.authenticate(peer).await
    }
    async fn open_session(&mut self) -> Result<()> {
        if self.is_dead() {
            return Err(self.down());
        }
        self.inner.open_session().await
    }
    async fn send_chunk(&mut self, chunk: Chunk) -> Result<VerifiedChunk> {
        if self.is_dead() {
            return Err(self.down());
        }
        self.inner.send_chunk(chunk).await
    }
    async fn receive_chunk(&mut self) -> Result<Chunk> {
        if self.is_dead() {
            return Err(self.down());
        }
        self.inner.receive_chunk().await
    }
    async fn health(&self) -> Result<LinkHealth> {
        self.inner.health().await
    }
    async fn reconnect(&mut self) -> Result<()> {
        if self.is_dead() {
            return Err(self.down());
        }
        self.inner.reconnect().await
    }
    async fn close(&mut self) -> Result<()> {
        self.inner.close().await
    }
}

/// Flips one byte in the Nth chunk handed to the receiver — bit-rot on the wire.
/// Level 1 must catch it before the bytes are ever staged.
struct CorruptOnce<T: Transport> {
    inner: T,
    flip_at: u64,
    seen: u64,
}

#[async_trait]
impl<T: Transport> Transport for CorruptOnce<T> {
    async fn discover(&mut self) -> Result<Vec<PeerInfo>> {
        self.inner.discover().await
    }
    async fn authenticate(&mut self, peer: &PeerInfo) -> Result<()> {
        self.inner.authenticate(peer).await
    }
    async fn open_session(&mut self) -> Result<()> {
        self.inner.open_session().await
    }
    async fn send_chunk(&mut self, chunk: Chunk) -> Result<VerifiedChunk> {
        self.inner.send_chunk(chunk).await
    }
    async fn receive_chunk(&mut self) -> Result<Chunk> {
        let mut chunk = self.inner.receive_chunk().await?;
        self.seen += 1;
        if self.seen == self.flip_at && !chunk.data.is_empty() {
            chunk.data[0] ^= 0xFF;
        }
        Ok(chunk)
    }
    async fn health(&self) -> Result<LinkHealth> {
        self.inner.health().await
    }
    async fn reconnect(&mut self) -> Result<()> {
        self.inner.reconnect().await
    }
    async fn close(&mut self) -> Result<()> {
        self.inner.close().await
    }
}

/// Throws a switch after `cut_after` chunks have been staged, modelling an app
/// kill / device sleep rather than a recoverable link drop.
struct KillAfter {
    switch: Arc<AtomicU64>,
    cut_after: u64,
    staged: u64,
}

impl ProgressSink for KillAfter {
    fn on_object_progress(&mut self, _object_id: &str, _bytes: u64, _size: u64) {
        self.staged += 1;
        if self.staged >= self.cut_after {
            self.switch.store(1, Ordering::SeqCst);
        }
    }
}

/// Counts acked chunks (to prove a resume never re-sends verified data) and
/// reconnects (to prove a lossy session actually recovered).
#[derive(Default)]
struct SentCounter {
    chunks: u64,
    reconnects: u64,
}

impl ProgressSink for SentCounter {
    fn on_chunk_verified(&mut self, _object_id: &str, _sequence: u64) {
        self.chunks += 1;
    }
    fn on_reconnect(&mut self, _object_id: &str, _sequence: u64) {
        self.reconnects += 1;
    }
}

struct NullProgress;
impl ProgressSink for NullProgress {}

// The recoverable-drop test below tracks progress and cuts the link in one
// sink (`DropOnceDuringTracking`), so no standalone cut-once helper is needed.

/// 16 KiB chunks keep chunk counts high enough for meaningful 25/50/90% cut
/// points while keeping the harness fast. 56 chunks total: 40 + 13 + 3.
const CHUNK_SIZE: u32 = 16 * 1024;

const SPECS: &[(&str, Category, usize)] = &[
    ("video-0", Category::Video, 640 * 1024),
    ("image-0", Category::Image, 200 * 1024),
    ("doc-0", Category::Document, 40 * 1024),
];

struct Fixture {
    manifest: Manifest,
    source: FileSource,
    paths: HashMap<String, PathBuf>,
    chunk_sizes: HashMap<String, u32>,
    total_chunks: u64,
}

/// Write the source blobs to disk, hash them once (whole object + per chunk),
/// and produce the manifest a real inventory pass would emit.
fn build_fixture(root: &Path) -> Fixture {
    let source_dir = root.join("source");
    fs::create_dir_all(&source_dir).unwrap();

    let mut manifest = Manifest::new("chaos-1".into(), "android".into(), "ipad".into());
    let mut paths = HashMap::new();
    let mut chunk_sizes = HashMap::new();
    let mut total_chunks = 0u64;

    for (id, category, size) in SPECS {
        let path = source_dir.join(id);
        fs::write(&path, blob(id, *size)).unwrap();

        let mut f = fs::File::open(&path).unwrap();
        let (whole, per_chunk) = checkpoint::sha256_chunks(&mut f, CHUNK_SIZE).unwrap();

        let obj = manifest.add_object((*id).into(), *category, *size as u64, CHUNK_SIZE);
        obj.content_hash = whole;
        total_chunks += obj.chunk_count;
        manifest.set_chunk_hashes(id, per_chunk).unwrap();

        paths.insert((*id).to_string(), path);
        chunk_sizes.insert((*id).to_string(), CHUNK_SIZE);
    }

    Fixture {
        manifest,
        source: FileSource {
            paths: paths.clone(),
        },
        paths,
        chunk_sizes,
        total_chunks,
    }
}

/// Phase 1 exit gate: interrupt at 25%, 50% and 90% with a permanent failure,
/// resume from the last verified chunk in a fresh process, and require
/// byte-identical, hash-verified output — "zero silent corruption".
#[tokio::test]
async fn resume_after_permanent_interruption_at_25_50_90_percent() {
    for pct in [25u64, 50, 90] {
        let root = tempfile::tempdir().unwrap();
        let fixture = build_fixture(root.path());
        let cut_after = fixture.total_chunks * pct / 100;
        let staging = root.path().join("staging");

        // ---- Run 1: killed mid-transfer ----
        let switch = Arc::new(AtomicU64::new(0));
        let (a, b, _fh) = loopback_pair(
            peer("android", PeerPlatform::Android),
            peer("ipad", PeerPlatform::Ipad),
        );
        let mut sender_transport = KillSwitch {
            inner: a,
            killed: switch.clone(),
        };
        let mut receiver_transport = KillSwitch {
            inner: b,
            killed: switch.clone(),
        };

        let mut source = FileSource {
            paths: fixture.paths.clone(),
        };
        let mut sink = FileSink::new(&staging, fixture.chunk_sizes.clone());
        let mut tracker = ResumeTracker::new();
        let mut killer = KillAfter {
            switch: switch.clone(),
            cut_after,
            staged: 0,
        };

        let (sres, rres) = tokio::join!(
            async {
                let mut s = Sender::new(&mut sender_transport);
                s.send_manifest(&fixture.manifest).await?;
                s.send_objects(
                    &fixture.manifest,
                    &mut source,
                    &mut tracker,
                    DEFAULT_MAX_RETRIES,
                    &|_| 0,
                )
                .await
            },
            async {
                let mut r = Receiver::new(&mut receiver_transport);
                let m = r.receive_manifest().await?;
                r.receive_objects(&m, &mut sink, &mut killer).await
            },
        );

        assert!(
            rres.is_err(),
            "{pct}%: receiver should have died, got {rres:?}"
        );
        let staged_before: u64 = fixture
            .manifest
            .objects
            .iter()
            .map(|o| sink.staged(&o.id) as u64)
            .sum();
        assert!(
            staged_before > 0,
            "{pct}%: nothing was staged before the kill"
        );
        assert!(
            staged_before < fixture.total_chunks,
            "{pct}%: the transfer should be incomplete"
        );
        // The sender may legitimately have enqueued everything before the kill.
        let _ = &sres;

        // ---- The receiver persists its checkpoint, like a real app would ----
        let store = CheckpointStore::open(root.path().join("state")).unwrap();
        let mut resume_manifest = fixture.manifest.clone();
        for obj in resume_manifest.objects.iter_mut() {
            let staged = sink.staged(&obj.id) as u64;
            obj.transfer_state = if staged >= obj.chunk_count {
                TransferState::Verified
            } else if staged > 0 {
                TransferState::Partial
            } else {
                TransferState::Pending
            };
        }
        resume_manifest.state = SessionState::Paused;
        let receiver_progress = sink.progress();
        store
            .save(&CheckpointStore::snapshot_with(
                &resume_manifest,
                receiver_progress,
            ))
            .unwrap();

        // ---- Run 2: a brand-new process resumes ----
        let restored_manifest = store
            .restore_manifest("chaos-1")
            .unwrap()
            .expect("manifest persisted");
        let restored_cp = store
            .load("chaos-1")
            .unwrap()
            .expect("checkpoint persisted");

        // Chunks 0..=last are already on disk. The new sink starts empty, so this
        // seed is what tells the engine they exist.
        let mut seed: ReceivedIndex = ReceivedIndex::new();
        for (id, last) in &restored_cp.verified_through {
            seed.insert(id.clone(), (0..=*last).collect());
        }

        let (mut a2, mut b2, _fh2) = loopback_pair(
            peer("android", PeerPlatform::Android),
            peer("ipad", PeerPlatform::Ipad),
        );
        let mut source2 = FileSource {
            paths: fixture.paths.clone(),
        };
        let mut sink2 = FileSink::new(&staging, fixture.chunk_sizes.clone());
        let mut resumed = SentCounter::default();

        let (sres2, rres2) = tokio::join!(
            async {
                let mut s = Sender::new(&mut a2);
                s.send_manifest(&restored_manifest).await?;
                s.send_objects(
                    &restored_manifest,
                    &mut source2,
                    &mut resumed,
                    DEFAULT_MAX_RETRIES,
                    &|o| restored_cp.resume_from(&o.id),
                )
                .await
            },
            async {
                let mut r = Receiver::new(&mut b2);
                let m = r.receive_manifest().await?;
                r.receive_objects_from(&m, &mut sink2, &mut NullProgress, &mut seed)
                    .await
            },
        );
        assert!(sres2.is_ok(), "{pct}%: resume sender failed: {sres2:?}");
        assert!(rres2.is_ok(), "{pct}%: resume receiver failed: {rres2:?}");

        // Byte-identical staging + hash-verified objects.
        for obj in &fixture.manifest.objects {
            let expected = read_all(&fixture.paths[&obj.id]);
            let staged = read_all(&sink2.path_for(&obj.id));
            assert_eq!(staged.len() as u64, obj.size, "{pct}%: size of {}", obj.id);
            assert!(
                staged == expected,
                "{pct}%: {} is not byte-identical after resume",
                obj.id
            );
            assert_eq!(
                checkpoint::sha256_streaming(&mut &staged[..]).unwrap(),
                obj.content_hash,
                "{pct}%: hash of {} after resume",
                obj.id
            );
        }

        // Exactly the missing chunks were sent: every already-verified chunk was
        // skipped, and every missing chunk arrived.
        let already: u64 = fixture
            .manifest
            .objects
            .iter()
            .map(|o| restored_cp.resume_from(&o.id).min(o.chunk_count))
            .sum();
        assert_eq!(
            resumed.chunks,
            fixture.total_chunks - already,
            "{pct}%: resume must send exactly the missing chunks"
        );
    }
}

/// A *recoverable* drop (Wi-Fi blip, router restart) must not end the session:
/// both sides reconnect and the transfer finishes byte-identical with no user
/// action (Phase 4 behaviour, exercised through the core).
#[tokio::test]
async fn recoverable_drop_mid_object_recovers_without_user_action() {
    /// Cuts the link once, then keeps tracking progress.
    struct DropOnceDuringTracking {
        inner: ResumeTracker,
        fault: FaultHandle,
        remaining: u64,
        cut: bool,
    }

    impl ProgressSink for DropOnceDuringTracking {
        fn on_chunk_verified(&mut self, object_id: &str, sequence: u64) {
            self.inner.on_chunk_verified(object_id, sequence);
            if self.remaining > 0 {
                self.remaining -= 1;
                if self.remaining == 0 && !self.cut {
                    self.cut = true;
                    self.fault.cut();
                }
            }
        }
        fn on_object_progress(&mut self, object_id: &str, bytes: u64, size: u64) {
            self.inner.on_object_progress(object_id, bytes, size);
        }
        fn on_object_complete(&mut self, object_id: &str) {
            self.inner.on_object_complete(object_id);
        }
        fn on_reconnect(&mut self, object_id: &str, sequence: u64) {
            self.inner.on_reconnect(object_id, sequence);
        }
    }

    let root = tempfile::tempdir().unwrap();
    let fixture = build_fixture(root.path());
    let (mut a, mut b, fault) = loopback_pair(
        peer("android", PeerPlatform::Android),
        peer("ipad", PeerPlatform::Ipad),
    );

    let mut source = fixture.source;
    let mut sink = FileSink::new(root.path().join("staging"), fixture.chunk_sizes.clone());
    let mut sender_side = DropOnceDuringTracking {
        inner: ResumeTracker::new(),
        fault: fault.clone(),
        remaining: 5, // land the drop mid-object, not at a boundary
        cut: false,
    };
    let mut receiver_side = SentCounter::default();

    let (sres, rres) = tokio::join!(
        async {
            let mut s = Sender::new(&mut a);
            s.send_manifest(&fixture.manifest).await?;
            s.send_objects(
                &fixture.manifest,
                &mut source,
                &mut sender_side,
                DEFAULT_MAX_RETRIES,
                &|_| 0,
            )
            .await
        },
        async {
            let mut r = Receiver::new(&mut b);
            let m = r.receive_manifest().await?;
            r.receive_objects(&m, &mut sink, &mut receiver_side).await
        },
    );

    assert!(sres.is_ok(), "sender: {sres:?}");
    assert!(rres.is_ok(), "receiver: {rres:?}");
    assert!(sender_side.cut, "the drop was never injected");
    assert!(
        sender_side.inner.reconnects() + receiver_side.reconnects >= 1,
        "a dropped link must be reconnected automatically"
    );
    assert_eq!(
        sender_side.inner.completed(),
        fixture.manifest.objects.len(),
        "every object should be complete"
    );
    for obj in &fixture.manifest.objects {
        assert!(
            read_all(&sink.path_for(&obj.id)) == read_all(&fixture.paths[&obj.id]),
            "{} must be byte-identical after a recovered drop",
            obj.id
        );
    }
}

/// Bit-rot on the wire: the corrupted chunk must be rejected *before* staging,
/// and a clean resume afterwards must finish the job.
#[tokio::test]
async fn corrupted_chunk_is_rejected_and_a_clean_resume_finishes() {
    let root = tempfile::tempdir().unwrap();
    let fixture = build_fixture(root.path());
    let staging = root.path().join("staging");

    let (mut a, b, _fh) = loopback_pair(
        peer("android", PeerPlatform::Android),
        peer("ipad", PeerPlatform::Ipad),
    );
    // Flip a byte in the 8th frame the receiver sees. Frame 1 is the manifest,
    // so frame N corresponds to data chunk sequence N - 2 => sequence 6 here.
    let mut receiver_transport = CorruptOnce {
        inner: b,
        flip_at: 8,
        seen: 0,
    };
    let mut source = FileSource {
        paths: fixture.paths.clone(),
    };
    let mut sink = FileSink::new(&staging, fixture.chunk_sizes.clone());

    let (sres, rres) = tokio::join!(
        async {
            let mut s = Sender::new(&mut a);
            s.send_manifest(&fixture.manifest).await?;
            s.send_objects(
                &fixture.manifest,
                &mut source,
                &mut NullProgress,
                DEFAULT_MAX_RETRIES,
                &|_| 0,
            )
            .await
        },
        async {
            let mut r = Receiver::new(&mut receiver_transport);
            let m = r.receive_manifest().await?;
            let mut seed = ReceivedIndex::new();
            r.receive_objects_from(&m, &mut sink, &mut NullProgress, &mut seed)
                .await
        },
    );
    // The sender may have enqueued everything before the receiver noticed.
    let _ = &sres;
    match rres {
        Err(Error::IntegrityMismatch { chunk_index, .. }) => {
            assert_eq!(chunk_index, 6, "the flipped chunk must be rejected");
        }
        other => panic!("expected IntegrityMismatch, got {other:?}"),
    }
    // The bad chunk was never written: only chunks 0..5 of video-0 exist.
    assert_eq!(sink.staged("video-0"), 6);
    assert_eq!(sink.staged("image-0"), 0);
    let partial = read_all(&sink.path_for("video-0"));
    assert_eq!(
        partial.len(),
        6 * CHUNK_SIZE as usize,
        "the corrupt chunk must not have extended the staging file"
    );
    assert_eq!(
        partial,
        read_all(&fixture.paths["video-0"])[..partial.len()].to_vec(),
        "bytes before the corrupted chunk must still match the source"
    );

    // ---- A clean resume finishes the transfer ----
    let progress = sink.progress();
    let mut seed: ReceivedIndex = progress
        .iter()
        .map(|(id, last)| (id.clone(), (0..=*last).collect()))
        .collect();
    let mut resume_manifest = fixture.manifest.clone();
    resume_manifest.state = SessionState::Paused;
    let cp = CheckpointStore::snapshot_with(&resume_manifest, progress);

    let (mut a2, mut b2, _fh2) = loopback_pair(
        peer("android", PeerPlatform::Android),
        peer("ipad", PeerPlatform::Ipad),
    );
    let mut source2 = FileSource {
        paths: fixture.paths.clone(),
    };
    let mut resumed = SentCounter::default();

    let (sres2, rres2) = tokio::join!(
        async {
            let mut s = Sender::new(&mut a2);
            s.send_manifest(&resume_manifest).await?;
            s.send_objects(
                &resume_manifest,
                &mut source2,
                &mut resumed,
                DEFAULT_MAX_RETRIES,
                &|o| cp.resume_from(&o.id),
            )
            .await
        },
        async {
            let mut r = Receiver::new(&mut b2);
            let m = r.receive_manifest().await?;
            r.receive_objects_from(&m, &mut sink, &mut NullProgress, &mut seed)
                .await
        },
    );
    assert!(sres2.is_ok(), "resume sender: {sres2:?}");
    assert!(rres2.is_ok(), "resume receiver: {rres2:?}");
    assert_eq!(
        resumed.chunks,
        fixture.total_chunks - 6,
        "only the missing chunks should be re-sent"
    );
    for obj in &fixture.manifest.objects {
        assert!(
            read_all(&sink.path_for(&obj.id)) == read_all(&fixture.paths[&obj.id]),
            "{} must be byte-identical after recovering from corruption",
            obj.id
        );
    }
}
