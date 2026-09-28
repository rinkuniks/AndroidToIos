//! Real-socket transport tests (Plan Phase 1 "real Wi-Fi transport").
//!
//! These bind actual TCP and UDP sockets on the loopback interface, pair with a
//! pairing code, and run the transfer engine end to end — the same code path the
//! Android and iOS apps will use, minus the radio.
//!
//! Run:  cargo test --test wifi_transport

use std::collections::HashMap;
use std::io::Cursor;
use std::net::SocketAddr;
use std::time::Duration;

use wa_bridge_core::checkpoint;
use wa_bridge_core::engine::{
    MediaSource, ObjectSink, ProgressSink, ReceivedIndex, Receiver, ResumeTracker, Sender,
    DEFAULT_MAX_RETRIES,
};
use wa_bridge_core::manifest::{Category, Manifest};
use wa_bridge_core::transport::{Chunk, PeerPlatform, Transport};
use wa_bridge_core::wifi::discovery::{Advertisement, Discovery};
use wa_bridge_core::wifi::{WifiConfig, WifiListener, WifiTransport};
use wa_bridge_core::{Error, Object};

const CODE: &str = "4821";
const WRONG_CODE: &str = "0000";
const CHUNK: u32 = 32 * 1024;

/// Every step that can block on the peer is wrapped in this deadline, so a
/// protocol deadlock fails the suite in seconds instead of hanging CI.
const STEP_DEADLINE: Duration = Duration::from_secs(20);

async fn within<F: std::future::Future>(what: &str, f: F) -> F::Output {
    match tokio::time::timeout(STEP_DEADLINE, f).await {
        Ok(out) => out,
        Err(_) => panic!("{what} exceeded its deadline — the two sides deadlocked"),
    }
}

fn cfg(name: &str, platform: PeerPlatform) -> WifiConfig {
    // Fast KDF keeps the suite quick; production pairing uses KdfParams::default.
    WifiConfig::new(name, platform).with_fast_kdf()
}

struct NullProgress;
impl ProgressSink for NullProgress {}

#[derive(Clone)]
struct MemSource {
    data: HashMap<String, Vec<u8>>,
}

impl MediaSource for MemSource {
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
        let offset = (sequence as usize) * (object.chunk_size as usize);
        if offset >= all.len() {
            return Ok(0);
        }
        let end = (offset + buf.len()).min(all.len());
        buf[..end - offset].copy_from_slice(&all[offset..end]);
        Ok(end - offset)
    }
}

struct MemSink {
    chunk_sizes: HashMap<String, u32>,
    staged: HashMap<String, Vec<u8>>,
}

impl MemSink {
    fn new(chunk_sizes: HashMap<String, u32>) -> Self {
        Self {
            chunk_sizes,
            staged: HashMap::new(),
        }
    }
}

impl ObjectSink for MemSink {
    fn write_chunk(&mut self, object_id: &str, sequence: u64, data: &[u8]) -> std::io::Result<()> {
        let cs = self.chunk_sizes.get(object_id).copied().unwrap_or(0) as usize;
        let offset = (sequence as usize) * cs;
        let need = offset + data.len();
        let buf = self.staged.entry(object_id.to_string()).or_default();
        if buf.len() < need {
            buf.resize(need, 0);
        }
        buf[offset..need].copy_from_slice(data);
        Ok(())
    }

    fn complete_object(&mut self, _object_id: &str) -> std::io::Result<()> {
        Ok(())
    }

    fn verify_object(&mut self, object_id: &str, expected_sha256: &str) -> std::io::Result<bool> {
        let staged = match self.staged.get(object_id) {
            Some(bytes) => bytes,
            None => return Ok(false),
        };
        let actual = checkpoint::sha256_streaming(&mut Cursor::new(staged))
            .map_err(|e| std::io::Error::other(e.to_string()))?;
        Ok(actual == expected_sha256)
    }
}

fn blob(seed: &str, size: usize) -> Vec<u8> {
    let key = seed.as_bytes();
    (0..size)
        .map(|i| {
            key[i % key.len()]
                .wrapping_mul(17)
                .wrapping_add((i % 199) as u8)
        })
        .collect()
}

/// Three objects with per-chunk hashes, as a real inventory pass would produce.
fn build_manifest() -> (Manifest, MemSource, HashMap<String, u32>) {
    let specs = [
        ("video-0", Category::Video, 160 * 1024usize),
        ("image-0", Category::Image, 80 * 1024),
        ("doc-0", Category::Document, 5 * 1024),
    ];
    let mut manifest = Manifest::new("wifi-1".into(), "android".into(), "ipad".into());
    let mut data = HashMap::new();
    let mut chunk_sizes = HashMap::new();

    for (id, category, size) in specs {
        let bytes = blob(id, size);
        let (whole, per_chunk) =
            checkpoint::sha256_chunks(&mut Cursor::new(&bytes), CHUNK).unwrap();
        let object = manifest.add_object(id.into(), category, size as u64, CHUNK);
        object.content_hash = whole;
        manifest.set_chunk_hashes(id, per_chunk).unwrap();
        data.insert(id.to_string(), bytes);
        chunk_sizes.insert(id.to_string(), CHUNK);
    }

    (manifest, MemSource { data }, chunk_sizes)
}

/// A listener on an ephemeral loopback port, plus the address to dial.
async fn listening(name: &str, platform: PeerPlatform) -> (WifiListener, SocketAddr) {
    let listener = WifiListener::bind("127.0.0.1:0", cfg(name, platform))
        .await
        .unwrap();
    let addr = listener.local_addr();
    (listener, addr)
}

#[tokio::test]
async fn pairs_over_real_tcp_and_roundtrips_a_chunk() {
    let (listener, addr) = listening("ipad-10", PeerPlatform::Ipad).await;
    let server = tokio::spawn(async move { listener.accept(CODE).await });

    let mut client = within(
        "client pairing",
        WifiTransport::connect(addr, CODE, cfg("android-cmf", PeerPlatform::Android)),
    )
    .await
    .unwrap();
    let mut server = within("server pairing", server).await.unwrap().unwrap();
    assert_eq!(client.peer().device_name, "ipad-10");
    assert_eq!(client.peer().platform, PeerPlatform::Ipad);
    assert_eq!(server.peer().device_name, "android-cmf");
    assert_eq!(server.peer().platform, PeerPlatform::Android);

    let chunk = Chunk {
        object_id: "video-0".into(),
        sequence: 7,
        data: vec![0xA5; 4096],
    };

    // `send_chunk` does not return until the receiver has staged the chunk and
    // acked it, so the two sides must be driven concurrently — exactly as the
    // Android sender and the iOS receiver do in production. Driving them one
    // after the other here would deadlock by construction.
    let (sent, received) = within("chunk round trip", async {
        tokio::join!(client.send_chunk(chunk.clone()), server.receive_chunk())
    })
    .await;

    let acked = sent.expect("chunk should be acked");
    assert_eq!(acked.object_id, "video-0");
    assert_eq!(acked.sequence, 7);

    let received = received.expect("chunk should be received");
    assert_eq!(received.object_id, chunk.object_id);
    assert_eq!(received.sequence, chunk.sequence);
    assert_eq!(received.data, chunk.data);

    let health = client.health().await.unwrap();
    assert!(
        health.bytes_per_second > 0,
        "health should report throughput"
    );
    assert_eq!(health.loss_percent, 0);
    assert!(client.is_connected());
    assert_eq!(client.reconnects(), 0);
}

#[tokio::test]
async fn wrong_pairing_code_is_rejected_on_both_sides() {
    let (listener, addr) = listening("ipad-10", PeerPlatform::Ipad).await;
    let server = tokio::spawn(async move { listener.accept(CODE).await });

    let client =
        WifiTransport::connect(addr, WRONG_CODE, cfg("android-cmf", PeerPlatform::Android)).await;
    match client {
        Err(Error::PairingRejected) => {}
        other => panic!("expected PairingRejected on the client, got {other:?}"),
    }

    match server.await.unwrap() {
        Err(Error::PairingRejected) => {}
        other => panic!("expected PairingRejected on the listener, got {other:?}"),
    }
}

#[tokio::test]
async fn short_pairing_codes_are_refused() {
    let (listener, addr) = listening("ipad-10", PeerPlatform::Ipad).await;
    let client = WifiTransport::connect(addr, "12", cfg("android", PeerPlatform::Android)).await;
    assert!(matches!(client, Err(Error::Protocol(_))), "got {client:?}");
    drop(listener);
}

/// The headline: the real engine, the real TCP transport, the real AEAD framing
/// — every object byte-identical and hash-verified at the far end.
#[tokio::test]
async fn end_to_end_transfer_over_real_tcp_verifies_every_object() {
    let (manifest, mut source, chunk_sizes) = build_manifest();
    let expected = source.data.clone();
    let (listener, addr) = listening("ipad-10", PeerPlatform::Ipad).await;

    let server = tokio::spawn(async move {
        let mut transport = listener.accept(CODE).await?;
        let mut sink = MemSink::new(chunk_sizes);
        let mut receiver = Receiver::new(&mut transport);
        let received = receiver.receive_manifest().await?;
        receiver
            .receive_objects(&received, &mut sink, &mut NullProgress)
            .await?;
        Ok::<_, Error>(sink)
    });

    let mut client = WifiTransport::connect(addr, CODE, cfg("android-cmf", PeerPlatform::Android))
        .await
        .unwrap();
    let mut tracker = ResumeTracker::new();
    {
        let mut sender = Sender::new(&mut client);
        sender.send_manifest(&manifest).await.unwrap();
        sender
            .send_objects(
                &manifest,
                &mut source,
                &mut tracker,
                DEFAULT_MAX_RETRIES,
                &|_| 0,
            )
            .await
            .unwrap();
    }

    let sink = server.await.unwrap().expect("server-side transfer failed");
    assert_eq!(tracker.completed(), manifest.objects.len());

    for object in &manifest.objects {
        let staged = sink.staged.get(&object.id).expect("object staged");
        let want = expected.get(&object.id).unwrap();
        assert_eq!(staged.len() as u64, object.size, "size of {}", object.id);
        assert!(
            staged == want,
            "{} is not byte-identical after a real TCP transfer",
            object.id
        );
        assert_eq!(
            checkpoint::sha256_streaming(&mut &staged[..]).unwrap(),
            object.content_hash,
            "hash of {}",
            object.id
        );
    }

    let health = client.health().await.unwrap();
    println!(
        "TCP loopback: {} bytes/s, rtt {} ms",
        health.bytes_per_second, health.rtt_ms
    );
    assert!(
        health.bytes_per_second > 0,
        "health should report throughput"
    );
}

#[tokio::test]
async fn discovery_finds_an_advertised_peer() {
    let (listener, _addr) = listening("ipad-10", PeerPlatform::Ipad).await;
    let scout = Discovery::bind("127.0.0.1:0").await.unwrap();
    let beacon = Discovery::bind("127.0.0.1:0").await.unwrap();

    let advertisement = Advertisement::for_listener(listener.config(), &listener);
    beacon
        .advertise(&advertisement, scout.local_addr())
        .await
        .unwrap();

    let peer = scout
        .recv_peer(Duration::from_secs(5))
        .await
        .unwrap()
        .expect("beacon should be received");
    assert_eq!(peer.device_name, "ipad-10");
    assert_eq!(peer.platform, PeerPlatform::Ipad);
    assert!(
        peer.addresses[0].ends_with(&listener.local_addr().port().to_string()),
        "dialable address should carry the listener's port: {:?}",
        peer.addresses
    );
}

#[tokio::test]
async fn discovery_reports_nothing_on_a_quiet_network() {
    let scout = Discovery::bind("127.0.0.1:0").await.unwrap();
    // An empty result is a normal outcome, not an error.
    assert!(scout
        .recv_peer(Duration::from_millis(100))
        .await
        .unwrap()
        .is_none());
}

/// A dropped Wi-Fi link must not cost more than the chunks that were in flight:
/// both sides re-establish the connection (client re-dials, listener re-accepts,
/// pairing re-runs), and the transfer resumes from the last verified chunk.
#[tokio::test]
async fn reconnect_over_real_tcp_resumes_mid_object() {
    let (manifest, mut source, chunk_sizes) = build_manifest();
    let target = "video-0";
    let target_bytes = source.data.get(target).unwrap().clone();
    let (listener, addr) = listening("ipad-10", PeerPlatform::Ipad).await;

    let server_task = tokio::spawn(async move { listener.accept(CODE).await });
    let mut client = WifiTransport::connect(addr, CODE, cfg("android-cmf", PeerPlatform::Android))
        .await
        .unwrap();
    let mut server = server_task.await.unwrap().unwrap();

    // Run 1: the first three chunks of one object cross the real socket.
    let mut sink = MemSink::new(chunk_sizes);
    for sequence in 0..3u64 {
        let start = sequence as usize * CHUNK as usize;
        let end = (start + CHUNK as usize).min(target_bytes.len());
        let (sent, received) = within("pre-interruption chunk", async {
            tokio::join!(
                client.send_chunk(Chunk {
                    object_id: target.to_string(),
                    sequence,
                    data: target_bytes[start..end].to_vec(),
                }),
                server.receive_chunk()
            )
        })
        .await;
        sent.expect("chunk should be acked");
        let received = received.expect("chunk should be received");
        sink.write_chunk(&received.object_id, received.sequence, &received.data)
            .unwrap();
    }

    // The link drops: unpair and re-establish. Both halves run concurrently —
    // one dials, the other accepts the new connection.
    client.close().await.unwrap();
    let (client_reconnect, server_reconnect) = within("reconnect", async {
        tokio::join!(client.reconnect(), server.reconnect())
    })
    .await;
    client_reconnect.unwrap();
    server_reconnect.unwrap();
    assert_eq!(client.reconnects(), 1);
    assert!(client.is_connected());

    // Run 2: resume mid-object over the new connection.
    let mut seed: ReceivedIndex = ReceivedIndex::new();
    seed.insert(target.to_string(), (0..3).collect());

    let (sender_result, receiver_result) = within("resumed transfer", async {
        tokio::join!(
            async {
                let mut sender = Sender::new(&mut client);
                sender.send_manifest(&manifest).await?;
                sender
                    .send_objects(
                        &manifest,
                        &mut source,
                        &mut NullProgress,
                        DEFAULT_MAX_RETRIES,
                        &|object| if object.id == target { 3 } else { 0 },
                    )
                    .await
            },
            async {
                let mut receiver = Receiver::new(&mut server);
                let received = receiver.receive_manifest().await?;
                receiver
                    .receive_objects_from(&received, &mut sink, &mut NullProgress, &mut seed)
                    .await
            },
        )
    })
    .await;
    sender_result.expect("resumed send should succeed");
    receiver_result.expect("resumed receive should succeed");

    // Every object — the resumed one included — is byte-identical.
    for object in &manifest.objects {
        let staged = sink.staged.get(&object.id).expect("object staged");
        let want = source.data.get(&object.id).unwrap();
        assert_eq!(staged.len() as u64, object.size, "size of {}", object.id);
        assert!(
            staged == want,
            "{} is not byte-identical after resuming over a new connection",
            object.id
        );
    }
}
