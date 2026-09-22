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
    fn read_at(&mut self, object: &Object, sequence: u64, buf: &mut [u8]) -> std::io::Result<usize>;
}
/// Sink that stages received object bytes (iOS/iPadOS writes to a staging dir).
pub trait ObjectSink {
    fn write_chunk(&mut self, object_id: &str, sequence: u64, data: &[u8]) -> std::io::Result<()>;
    fn complete_object(&mut self, object_id: &str) -> std::io::Result<()>;
}
/// Progress + checkpoint callbacks. All methods are no-ops by default.
pub trait ProgressSink {
        fn on_object_progress(&mut self, _object_id: &str, _bytes: u64, _size: u64) {}
    fn on_object_complete(&mut self, _object_id: &str) {}
    fn on_chunk_verified(&mut self, _object_id: &str, _sequence: u64) {}
    fn on_reconnect(&mut self, _object_id: &str, _sequence: u64) {}
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
            // Whole-object hash is only computed from sequence 0 (cross-process
            // reconstruction is deferred — ADR-002).
            let mut hasher = if sequence == 0 { Some(Sha256::new()) } else { None };
            loop {
                if remaining == 0 {
                    break;
                }
                let want = (chunk_size.min(remaining)) as usize;
                let mut buf = vec![0u8; want];
                let n = source
                    .read_at(obj, sequence, &mut buf)
                    .map_err(|e| Error::SourceRead { object_id: obj.id.clone(), source: e })?;
                if n == 0 {
                    return Err(Error::SizeMismatch { object_id: obj.id.clone() });
                }
                if let Some(h) = hasher.as_mut() {
                    h.update(&buf[..n]);
                }
                let chunk = Chunk { object_id: obj.id.clone(), sequence, data: buf[..n].to_vec() };
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
                return Err(Error::SizeMismatch { object_id: obj.id.clone() });
            }
            if let Some(h) = hasher {
                let digest = hex::encode(h.clone().finalize());
                if digest != obj.content_hash {
                    return Err(Error::HashMismatch { object_id: obj.id.clone() });
                }
            }
            progress.on_object_complete(&obj.id);
        }
        Ok(())
    }
}
/// Chunked receiver with streaming SHA-256 verification (v1.2 §9 Levels 1 & 2).
pub struct Receiver<'a, T: Transport + ?Sized> {
    transport: &'a mut T,
}

impl<'a, T: Transport + ?Sized> Receiver<'a, T> {
    pub fn new(transport: &'a mut T) -> Self {
        Self { transport }
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

    /// Receive objects until every manifest object is hash-verified.
    pub async fn receive_objects(
        &mut self,
        manifest: &Manifest,
        sink: &mut dyn ObjectSink,
        progress: &mut dyn ProgressSink,
    ) -> Result<()> {
        let total = manifest.objects.len() as u32;
        let mut hashers: HashMap<String, Sha256> = HashMap::new();
        let mut bytes_seen: HashMap<String, u64> = HashMap::new();
        // `object_id -> written sequences` makes reception idempotent for
        // re-delivered chunks during a retry (duplicate suppression).
        let mut written: HashMap<String, HashSet<u64>> = HashMap::new();
        let mut completed: u32 = 0;

        // Pre-mark objects already verified in the manifest (resume skip).
        for obj in &manifest.objects {
            if obj.transfer_state == TransferState::Verified {
                sink.complete_object(&obj.id)
                    .map_err(|e| Error::SourceRead { object_id: obj.id.clone(), source: e })?;
                completed += 1;
                progress.on_object_complete(&obj.id);
            }
        }

        while completed < total {
            let chunk = self
                .transport
                .receive_chunk()
                .await
                .map_err(|e| Error::Transport(e.to_string()))?;
            let obj = manifest
                .objects
                .iter()
                .find(|o| o.id == chunk.object_id)
                .ok_or_else(|| Error::Engine(format!("unknown object {}", chunk.object_id)))?;
            let object_id = chunk.object_id.clone();

            if !written.entry(object_id.clone()).or_default().insert(chunk.sequence) {
                continue;
            }
            if obj.transfer_state == TransferState::Verified {
                continue;
            }

            sink.write_chunk(&object_id, chunk.sequence, &chunk.data)
                .map_err(|e| Error::SourceRead { object_id: object_id.clone(), source: e })?;

            {
                let h = hashers.entry(object_id.clone()).or_default();
                h.update(&chunk.data);
            }
            let seen = {
                let e = bytes_seen.entry(object_id.clone()).or_insert(0);
                *e += chunk.data.len() as u64;
                *e
            };
            progress.on_object_progress(&object_id, seen, obj.size);

            if seen >= obj.size {
                let digest = {
                    let h = hashers.get_mut(&object_id).expect("hasher present");
                    hex::encode(h.clone().finalize())
                };
                if digest != obj.content_hash {
                    return Err(Error::HashMismatch { object_id });
                }
                hashers.remove(&object_id);
                bytes_seen.remove(&object_id);
                written.remove(&object_id);
                sink.complete_object(&object_id)
                    .map_err(|e| Error::SourceRead { object_id: object_id.clone(), source: e })?;
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
        PeerInfo { device_name: name.into(), platform, addresses: vec!["127.0.0.1".into()] }
    }

    fn gen_bytes(id: &str, size: u64) -> Vec<u8> {
        let b = id.as_bytes();
        (0..size).map(|i| b[(i as usize) % b.len()]).collect()
    }

    fn build(chunk_size: u32) -> (Manifest, MemorySource, HashMap<String, u32>) {
        let specs = [("o0", Category::Image, 3000u64), ("o1", Category::Video, 7000), ("o2", Category::Image, 512)];
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
    struct MemorySource { data: HashMap<String, Vec<u8>> }
    impl MediaSource for MemorySource {
        fn read_at(&mut self, object: &Object, sequence: u64, buf: &mut [u8]) -> std::io::Result<usize> {
            let all = self.data.get(&object.id).ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, object.id.clone()))?;
            let off = (sequence as usize) * (object.chunk_size as usize);
            if off >= all.len() { return Ok(0); }
            let end = (off + buf.len()).min(all.len());
            buf[..end - off].copy_from_slice(&all[off..end]);
            Ok(end - off)
        }
    }

    struct VecSink { staged: HashMap<String, Vec<u8>>, cs: HashMap<String, u32> }
    impl ObjectSink for VecSink {
        fn write_chunk(&mut self, object_id: &str, sequence: u64, data: &[u8]) -> std::io::Result<()> {
            let cs = self.cs.get(object_id).copied().unwrap_or(0) as usize;
            let off = (sequence as usize) * cs;
            let need = off + data.len();
            let v = self.staged.entry(object_id.to_string()).or_default();
            if v.len() < need { v.resize(need, 0); }
            v[off..need].copy_from_slice(data);
            Ok(())
        }
        fn complete_object(&mut self, _object_id: &str) -> std::io::Result<()> { Ok(()) }
    }

    struct NullProgress;
    impl ProgressSink for NullProgress {}

    #[tokio::test]
    async fn manifest_is_first_frame() {
        let (mut a, mut b, _fh) = loopback_pair(peer("A", PeerPlatform::Android), peer("B", PeerPlatform::Iphone));
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
        let (mut a, mut b, _fh) = loopback_pair(peer("A", PeerPlatform::Android), peer("B", PeerPlatform::Iphone));
        let (manifest, mut src, cs) = build(512);
        let mut sink = VecSink { staged: HashMap::new(), cs };
        let mut sp = NullProgress;
        let mut rp = NullProgress;

        let (sres, rres) = tokio::join!(
            async {
                let mut s = Sender::new(&mut a);
                s.send_manifest(&manifest).await?;
                s.send_objects(&manifest, &mut src, &mut sp, DEFAULT_MAX_RETRIES, &|_| 0).await
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
        let (mut a, mut b, fault) = loopback_pair(peer("A", PeerPlatform::Android), peer("B", PeerPlatform::Iphone));
        let (manifest, mut src, cs) = build(512);
        let mut sink = VecSink { staged: HashMap::new(), cs };
        let cut_after = Arc::new(AtomicU64::new(0));
        let reconnects = Arc::new(AtomicU64::new(0));

        struct CutProgress { cut_after: Arc<AtomicU64>, reconnects: Arc<AtomicU64>, fault: FaultHandle }
        impl ProgressSink for CutProgress {
            fn on_chunk_verified(&mut self, _id: &str, _seq: u64) {
                if self.cut_after.fetch_add(1, Ordering::SeqCst) == 0 { self.fault.cut(); }
            }
            fn on_reconnect(&mut self, _id: &str, _seq: u64) { self.reconnects.fetch_add(1, Ordering::SeqCst); }
        }

        let mut sp = CutProgress { cut_after: cut_after.clone(), reconnects: reconnects.clone(), fault: fault.clone() };
        let mut rp = NullProgress;

        let (sres, rres) = tokio::join!(
            async {
                let mut s = Sender::new(&mut a);
                s.send_manifest(&manifest).await?;
                s.send_objects(&manifest, &mut src, &mut sp, DEFAULT_MAX_RETRIES, &|_| 0).await
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
}




