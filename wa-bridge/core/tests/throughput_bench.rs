//! Phase 1 throughput baselining harness.
//!
//! Measures end-to-end transfer rate (sender -> receiver over `LoopbackTransport`)
//! across object sizes and chunk sizes, plus the performance cost of a single
//! link fault (cut after N verified chunks) with automatic reconnect.
//!
//! Run:  cargo test --release --test throughput_bench
//!        cargo test --release --test throughput_bench -- --ignored
//!
//! Results are printed to stdout so they can inform DEFAULT_CHUNK_SIZE and
//! the v1.2 performance gate.

use std::collections::HashMap;
use std::io::Cursor;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;

use wa_bridge_core::checkpoint;
use wa_bridge_core::engine::{
    MediaSource, ObjectSink, ProgressSink, Receiver, Sender, DEFAULT_MAX_RETRIES,
};
use wa_bridge_core::manifest::{Category, Manifest};
use wa_bridge_core::transport::{FaultHandle, PeerInfo, PeerPlatform};
use wa_bridge_core::{loopback_pair, Object};

/// No-op progress sink — we only care about throughput, not progress callbacks.
struct NoopProgress;
impl ProgressSink for NoopProgress {}

/// Simple in-memory blob source (deterministic bytes, reproducible across runs).
#[derive(Clone)]
struct BenchSource {
    data: HashMap<String, Vec<u8>>,
}

impl MediaSource for BenchSource {
    fn read_at(
        &mut self,
        object: &Object,
        sequence: u64,
        buf: &mut [u8],
    ) -> std::io::Result<usize> {
        let all = self
            .data
            .get(&object.id)
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, object.id.clone()))?;
        let off = (sequence as usize) * (object.chunk_size as usize);
        if off >= all.len() {
            return Ok(0);
        }
        let end = (off + buf.len()).min(all.len());
        buf[..end - off].copy_from_slice(&all[off..end]);
        Ok(end - off)
    }
}

/// In-memory sink that collects bytes (we hash-check for correctness + rate).
#[derive(Clone)]
struct BenchSink {
    cs: HashMap<String, u32>,
    staged: HashMap<String, Vec<u8>>,
}

impl ObjectSink for BenchSink {
    fn write_chunk(&mut self, object_id: &str, sequence: u64, data: &[u8]) -> std::io::Result<()> {
        let cs = *self.cs.get(object_id).unwrap_or(&0) as usize;
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

    fn verify_object(&mut self, object_id: &str, expected_sha256: &str) -> std::io::Result<bool> {
        let staged = match self.staged.get(object_id) {
            Some(s) => s,
            None => return Ok(false),
        };
        let actual = checkpoint::sha256_streaming(&mut Cursor::new(staged))
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        Ok(actual == expected_sha256)
    }
}

/// Progress sink that records chunks verified + reconnects; optionally cuts the link.
struct CountingProgress {
    verified: Arc<AtomicU64>,
    reconnects: Arc<AtomicU64>,
    fault: FaultHandle,
    cut_after: u64,
}

impl ProgressSink for CountingProgress {
    fn on_chunk_verified(&mut self, _id: &str, _seq: u64) {
        let n = self.verified.fetch_add(1, Ordering::SeqCst);
        if n == self.cut_after && self.cut_after > 0 {
            self.fault.cut();
        }
    }
    fn on_reconnect(&mut self, _object_id: &str, _sequence: u64) {
        self.reconnects.fetch_add(1, Ordering::SeqCst);
    }
}

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

fn build_manifest(
    sizes: &[(&str, Category, u64)],
    chunk_size: u32,
) -> (Manifest, BenchSource, BenchSink) {
    let mut m = Manifest::new("bench".into(), "android".into(), "ios".into());
    let mut data = HashMap::new();
    let mut cs = HashMap::new();
    for (id, cat, size) in sizes {
        let bytes = gen_bytes(id, *size);
        let hash = checkpoint::sha256_streaming(&mut Cursor::new(&bytes)).unwrap();
        let o = m.add_object(id.to_string(), *cat, *size, chunk_size);
        o.content_hash = hash;
        data.insert(id.to_string(), bytes);
        cs.insert(id.to_string(), chunk_size);
    }
    let sink = BenchSink {
        cs,
        staged: HashMap::new(),
    };
    (m, BenchSource { data }, sink)
}

/// Run a full E2E transfer over a fresh loopback pair.
/// Returns (bytes_transferred, reconnect_count, throughput_mbs, elapsed_ms).
async fn run_transfer(
    manifest: &Manifest,
    source: &mut BenchSource,
    sink: &mut BenchSink,
    cut_after: u64,
) -> (u64, u64, f64, u128) {
    let total_bytes: u64 = manifest.objects.iter().map(|o| o.size).sum();
    let verified = Arc::new(AtomicU64::new(0));
    let reconnects = Arc::new(AtomicU64::new(0));

    let (mut a, mut b, fh) = loopback_pair(
        peer("A", PeerPlatform::Android),
        peer("B", PeerPlatform::Iphone),
    );

    let mut cp = CountingProgress {
        verified: verified.clone(),
        reconnects: reconnects.clone(),
        fault: fh.clone(),
        cut_after,
    };

    let start = Instant::now();
    let (sres, rres) = tokio::join! {
        async {
            let mut s = Sender::new(&mut a);
            s.send_manifest(manifest).await?;
            s.send_objects(
                manifest,
                source,
                &mut cp,
                DEFAULT_MAX_RETRIES,
                &|_| 0,
            )
            .await
        },
        async {
            let mut r = Receiver::new(&mut b);
            let m = r.receive_manifest().await?;
            r.receive_objects(&m, sink, &mut NoopProgress).await
        },
    };
    let elapsed = start.elapsed();

    assert!(sres.is_ok(), "sender error: {:?}", sres);
    assert!(rres.is_ok(), "receiver error: {:?}", rres);
    assert!(!fh.is_cut(), "link should be restored after transfer");

    let throughput_mbs = (total_bytes as f64 / elapsed.as_secs_f64()) / (1024.0 * 1024.0);
    let rcount = reconnects.load(Ordering::SeqCst);
    (total_bytes, rcount, throughput_mbs, elapsed.as_millis())
}

/// Verify integrity of transferred data in the sink against known hashes.
fn verify_integrity(manifest: &Manifest, sink: &BenchSink) {
    for obj in &manifest.objects {
        let staged = sink.staged.get(&obj.id).expect("object should be in sink");
        let hash = checkpoint::sha256_streaming(&mut Cursor::new(staged)).unwrap();
        assert_eq!(
            hash, obj.content_hash,
            "hash mismatch for object {}",
            obj.id
        );
        assert_eq!(
            staged.len() as u64,
            obj.size,
            "size mismatch for {}",
            obj.id
        );
    }
}

/// Quick smoke test that verifies the bench harness works (runs by default).
#[tokio::test]
async fn throughput_smoke_test() {
    let (manifest, mut src, mut sink) = build_manifest(
        &[
            ("s1", Category::Image, 4096u64),
            ("s2", Category::Audio, 8192),
        ],
        1024,
    );
    let (_, _, mbps, _) = run_transfer(&manifest, &mut src, &mut sink, 0).await;
    verify_integrity(&manifest, &sink);
    println!("Smoke test: {:.1} MB/s", mbps);
}

/// Full benchmark suite — use --ignored to run the slow benchmarks.
#[tokio::test]
#[ignore]
async fn throughput_benchmarks() {
    println!("\n=== WA Bridge Throughput Baselines (Phase 1) ===\n");

    let chunk_sizes = [1024u32, 64 * 1024, 4 * 1024 * 1024]; // 1 KiB, 64 KiB, 4 MiB
    let mut results: Vec<(String, u32, u64, f64, u128, u64)> = Vec::new();

    let sizes_small: &[(&str, Category, u64)] = &[
        ("o0", Category::Image, 300),
        ("o1", Category::Video, 300),
        ("o2", Category::Image, 256),
    ];
    let sizes_med: &[(&str, Category, u64)] = &[
        ("o0", Category::Image, 128 * 1024),
        ("o1", Category::Video, 128 * 1024),
        ("o2", Category::Audio, 128 * 1024),
    ];
    let sizes_large: &[(&str, Category, u64)] = &[("o0", Category::Video, 10 * 1024 * 1024)];
    let sizes_huge: &[(&str, Category, u64)] = &[("o0", Category::Video, 50 * 1024 * 1024)];

    for (label, sizes) in [
        ("Small objects (756 B)", sizes_small),
        ("Medium objects (384 KB)", sizes_med),
        ("Large object (10 MB)", sizes_large),
        ("Large object (50 MB)", sizes_huge),
    ] {
        for &cs in &chunk_sizes {
            // Skip tiny chunk sizes for huge objects (would create tens of thousands of chunks)
            let skip = sizes.iter().any(|(_, _, s)| *s >= 1_000_000) && cs < 64 * 1024;
            if skip {
                continue;
            }

            let (manifest, mut src, mut sink) = build_manifest(sizes, cs);

            // Warm-up (not measured)
            let (_, _, _, _) = run_transfer(
                &manifest,
                &mut src.clone(),
                &mut BenchSink {
                    cs: sink.cs.clone(),
                    staged: HashMap::new(),
                },
                0,
            )
            .await;

            // Measured run
            let (bytes, reconnects, mbps, ms) =
                run_transfer(&manifest, &mut src, &mut sink, 0).await;
            results.push((label.to_string(), cs, bytes, mbps, ms, reconnects));
            println!(
                "{:<30} | cs={:>8} | {:>10.0} B | {:>8.1} ms | {:>8.1} MB/s | reconnects={}",
                label, cs, bytes as f64, ms, mbps, reconnects
            );
            verify_integrity(&manifest, &sink);
        }
    }

    // --- Fault scenario: 50 MB object, 4 MiB chunks, cut after 3 verified chunks ---
    println!("\n--- Fault scenario (50MB, 4MiB chunks, cut after 3) ---");
    let (manifest, mut src, mut sink) = build_manifest(sizes_huge, 4 * 1024 * 1024);
    let (bytes, reconnects, mbps, ms) = run_transfer(&manifest, &mut src, &mut sink, 3).await;
    results.push((
        "50MB + 1 fault (cs=4MiB)".to_string(),
        4 * 1024 * 1024,
        bytes,
        mbps,
        ms,
        reconnects,
    ));
    println!(
        "{:<30} | {:>10.0} B | {:>8.1} ms | {:>8.1} MB/s | reconnects={}",
        "50MB + 1 fault", bytes as f64, ms, mbps, reconnects
    );
    verify_integrity(&manifest, &sink);
    assert!(reconnects >= 1, "should have reconnected at least once");

    // --- Summary ---
    println!("\n=== Summary (sorted by throughput MB/s) ===");
    println!(
        "{:<32} | {:>10} | {:>10} | {:>8} | {:>10} | {:>10}",
        "Scenario", "total_bytes", "chunk_size", "MB/s", "elapsed_ms", "reconnects"
    );
    let mut sorted = results.clone();
    sorted.sort_by(|a, b| b.3.partial_cmp(&a.3).unwrap());
    for (desc, cs, total, mbps, ms, rcount) in &sorted {
        println!(
            "{:<32} | {:>10} | {:>10} | {:>8.1} | {:>10} | {:>10}",
            desc, total, cs, mbps, ms, rcount
        );
    }

    // --- Performance gate: 50MB no-fault transfer should exceed 10 MB/s ---
    let best_50mb = results
        .iter()
        .filter(|(d, cs, _, _, _, _)| d == "Large object (50 MB)" && *cs == 4 * 1024 * 1024)
        .max_by(|a, b| a.3.partial_cmp(&b.3).unwrap())
        .map(|r| r.3);

    if let Some(mbps) = best_50mb {
        println!(
            "\n50MB transfer throughput: {:.1} MB/s (gate: >10 MB/s)",
            mbps
        );
        assert!(
            mbps > 10.0,
            "50MB loopback transfer fell below 10 MB/s performance gate"
        );
    }

    println!(
        "\n✅ All throughput baselines captured, integrity verified, performance gate passed."
    );
}
