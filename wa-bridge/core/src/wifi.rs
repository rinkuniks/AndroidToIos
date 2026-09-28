//! Real same-Wi-Fi transport: TCP + AEAD pairing handshake + UDP discovery
//! (Plan Phase 1 "real Wi-Fi transport"; unblocks Phases 2, 3 and 4).
//!
//! L3 makes same-Wi-Fi the default transport, so this is the real thing rather
//! than a simulation: the transfer engine drives a TCP socket through the same
//! [`Transport`] trait it already uses for the loopback.
//!
//! Pairing never puts the code on the wire:
//!
//! 1. client → `Hello { device, nonce_c }` — plaintext, no secrets
//! 2. listener → `HelloAck { device, nonce_s }` — plaintext, no secrets
//! 3. both derive `key = Argon2id(code, salt = nonce_c XOR nonce_s)`
//! 4. client → sealed `Proof(SHA-256(nonce_c ‖ nonce_s))`, listener verifies
//! 5. listener → sealed `Proof`, client verifies
//!
//! Everything after step 5 is AEAD-sealed and bound to its slot by the wire
//! protocol ([`crate::protocol`]), so an off-path attacker can neither read
//! payloads nor replay a chunk into another object/sequence. A wrong code makes
//! a sealed proof fail to authenticate and is reported as
//! [`Error::PairingRejected`].
//!
//! The pairing code is short by design (5–7 taps), so a listener must rate-limit
//! attempts — Phase 4/Phase 10 hardening. The handshake itself is deliberately
//! Argon2id-heavy to make online guessing expensive.

use crate::crypto::{self, KdfParams, KeyPurpose, SecretKey, SALT_LEN};
use crate::protocol::{self, Frame, FrameKind};
use crate::transport::{Chunk, LinkHealth, PeerInfo, PeerPlatform, Transport, VerifiedChunk};
use crate::{Error, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::{TcpListener, TcpStream, UdpSocket};
use zeroize::Zeroizing;

/// Protocol version advertised during discovery and exchanged in `Hello`.
pub const PROTOCOL_VERSION: u8 = protocol::PROTOCOL_VERSION;
/// Handshake nonce size contributed by each side.
pub const HANDSHAKE_NONCE_LEN: usize = 16;
/// Shortest accepted pairing code (the UX target is 5–7 digits).
pub const MIN_PAIRING_CODE_LEN: usize = 4;
/// Default UDP port for discovery beacons.
pub const DEFAULT_DISCOVERY_PORT: u16 = 47_807;

/// Session configuration carried by both ends.
#[derive(Debug, Clone)]
pub struct WifiConfig {
    pub device_name: String,
    pub platform: PeerPlatform,
    /// Argon2id cost for turning the pairing code into a session key.
    pub kdf: KdfParams,
}

impl Default for WifiConfig {
    fn default() -> Self {
        Self {
            device_name: "wa-bridge".into(),
            platform: PeerPlatform::Android,
            kdf: KdfParams::default(),
        }
    }
}

impl WifiConfig {
    pub fn new(device_name: impl Into<String>, platform: PeerPlatform) -> Self {
        Self {
            device_name: device_name.into(),
            platform,
            kdf: KdfParams::default(),
        }
    }

    /// Cheap KDF settings. For tests and local development only — production
    /// pairing must use [`KdfParams::default`].
    pub fn with_fast_kdf(mut self) -> Self {
        self.kdf = KdfParams {
            m_cost_kib: 8,
            t_cost: 1,
            p_cost: 1,
        };
        self
    }
}

/// Plaintext handshake frame body (never contains the pairing code).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct Hello {
    device_name: String,
    platform: PeerPlatform,
    protocol_version: u8,
    nonce: Vec<u8>,
}

/// A paired byte stream, before it is wrapped in a [`WifiTransport`].
struct Session {
    reader: OwnedReadHalf,
    writer: OwnedWriteHalf,
    peer: PeerInfo,
    key: SecretKey,
}

fn fresh_nonce() -> Result<[u8; HANDSHAKE_NONCE_LEN]> {
    crypto::random_bytes::<HANDSHAKE_NONCE_LEN>()
}

/// `salt = nonce_c XOR nonce_s`, so both sides derive the same key from the code.
fn session_key(code: &str, client: &[u8], server: &[u8], kdf: &KdfParams) -> Result<SecretKey> {
    let mut salt = [0u8; SALT_LEN];
    for (index, byte) in salt.iter_mut().enumerate() {
        *byte = client[index % client.len()] ^ server[index % server.len()];
    }
    crypto::derive_key(code, KeyPurpose::Session, &salt, kdf)
}

/// Value both sides must be able to seal: proves knowledge of the pairing code.
fn pairing_proof(client: &[u8], server: &[u8]) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(client);
    hasher.update(server);
    hasher.finalize().to_vec()
}

fn check_pairing_code(code: &str) -> Result<()> {
    if code.len() < MIN_PAIRING_CODE_LEN {
        return Err(Error::Protocol(format!(
            "pairing code must be at least {MIN_PAIRING_CODE_LEN} characters"
        )));
    }
    Ok(())
}

/// A failed AEAD open during pairing means the codes differed; anything else is
/// a genuine protocol/transport problem and is reported unchanged.
fn as_pairing_failure(error: Error) -> Error {
    match error {
        Error::Crypto(_) => Error::PairingRejected,
        other => other,
    }
}

fn peer_from(hello: &Hello) -> PeerInfo {
    PeerInfo {
        device_name: hello.device_name.clone(),
        platform: hello.platform,
        addresses: Vec::new(),
    }
}

fn hello_frame(hello: &Hello, kind: FrameKind) -> Result<Frame> {
    let payload = serde_json::to_vec(hello).map_err(|e| Error::Protocol(e.to_string()))?;
    Ok(Frame::bare(kind, payload))
}

/// Client half of the pairing handshake.
async fn client_handshake(stream: TcpStream, code: &str, cfg: &WifiConfig) -> Result<Session> {
    check_pairing_code(code)?;
    let _ = stream.set_nodelay(true);
    let (mut reader, mut writer) = stream.into_split();

    let client_nonce = fresh_nonce()?;
    let hello = Hello {
        device_name: cfg.device_name.clone(),
        platform: cfg.platform,
        protocol_version: PROTOCOL_VERSION,
        nonce: client_nonce.to_vec(),
    };
    protocol::write_frame(&mut writer, &hello_frame(&hello, FrameKind::Hello)?, None).await?;

    let ack = protocol::read_frame(&mut reader, None).await?;
    if ack.kind != FrameKind::HelloAck {
        return Err(Error::Protocol(format!(
            "expected hello_ack, got {}",
            ack.kind.name()
        )));
    }
    let server_hello: Hello =
        serde_json::from_slice(&ack.payload).map_err(|e| Error::Protocol(e.to_string()))?;
    if server_hello.protocol_version != PROTOCOL_VERSION
        || server_hello.nonce.len() != HANDSHAKE_NONCE_LEN
    {
        return Err(Error::Protocol("incompatible peer handshake".into()));
    }

    let key = session_key(code, &client_nonce, &server_hello.nonce, &cfg.kdf)?;
    let proof = pairing_proof(&client_nonce, &server_hello.nonce);

    // Prove we hold the code, then require the peer to prove the same.
    protocol::write_frame(
        &mut writer,
        &Frame::bare(FrameKind::Proof, proof.clone()),
        Some(&key),
    )
    .await?;
    let server_proof = match protocol::read_frame(&mut reader, Some(&key)).await {
        Ok(frame) => frame,
        // A listener that cannot verify our proof closes the connection without
        // replying, so a dropped read here means "the peer rejected us" — the
        // dominant cause by far is a mismatched code.
        Err(Error::Transport(_)) => return Err(Error::PairingRejected),
        Err(other) => return Err(as_pairing_failure(other)),
    };
    if server_proof.kind != FrameKind::Proof || server_proof.payload != proof {
        return Err(Error::PairingRejected);
    }

    Ok(Session {
        reader,
        writer,
        peer: peer_from(&server_hello),
        key,
    })
}

/// Listener half of the pairing handshake.
async fn server_handshake(stream: TcpStream, code: &str, cfg: &WifiConfig) -> Result<Session> {
    check_pairing_code(code)?;
    let _ = stream.set_nodelay(true);
    let (mut reader, mut writer) = stream.into_split();

    let hello = protocol::read_frame(&mut reader, None).await?;
    if hello.kind != FrameKind::Hello {
        return Err(Error::Protocol(format!(
            "expected hello, got {}",
            hello.kind.name()
        )));
    }
    let client_hello: Hello =
        serde_json::from_slice(&hello.payload).map_err(|e| Error::Protocol(e.to_string()))?;
    if client_hello.protocol_version != PROTOCOL_VERSION
        || client_hello.nonce.len() != HANDSHAKE_NONCE_LEN
    {
        return Err(Error::Protocol("incompatible peer handshake".into()));
    }

    let server_nonce = fresh_nonce()?;
    let ours = Hello {
        device_name: cfg.device_name.clone(),
        platform: cfg.platform,
        protocol_version: PROTOCOL_VERSION,
        nonce: server_nonce.to_vec(),
    };
    protocol::write_frame(&mut writer, &hello_frame(&ours, FrameKind::HelloAck)?, None).await?;

    let key = session_key(code, &client_hello.nonce, &server_nonce, &cfg.kdf)?;
    let proof = pairing_proof(&client_hello.nonce, &server_nonce);

    let client_proof = protocol::read_frame(&mut reader, Some(&key))
        .await
        .map_err(as_pairing_failure)?;
    if client_proof.kind != FrameKind::Proof || client_proof.payload != proof {
        return Err(Error::PairingRejected);
    }
    protocol::write_frame(
        &mut writer,
        &Frame::bare(FrameKind::Proof, proof),
        Some(&key),
    )
    .await?;

    Ok(Session {
        reader,
        writer,
        peer: peer_from(&client_hello),
        key,
    })
}

/// Listening endpoint that accepts paired sessions.
///
/// A rejected pairing attempt only drops that connection — the endpoint keeps
/// listening, so one wrong code cannot take a transfer down. Callers decide how
/// many attempts to allow (Phase 4 rate-limiting).
pub struct WifiListener {
    listener: Arc<TcpListener>,
    cfg: WifiConfig,
    local: SocketAddr,
}

impl WifiListener {
    /// Bind to `bind_addr` (use port `0` for an ephemeral port in tests).
    pub async fn bind(bind_addr: impl AsRef<str>, cfg: WifiConfig) -> Result<Self> {
        let bind_addr = bind_addr.as_ref();
        let listener = TcpListener::bind(bind_addr)
            .await
            .map_err(|e| Error::Transport(format!("cannot listen on {bind_addr}: {e}")))?;
        let local = listener
            .local_addr()
            .map_err(|e| Error::Transport(e.to_string()))?;
        Ok(Self {
            listener: Arc::new(listener),
            cfg,
            local,
        })
    }

    /// The address a peer must dial (with the resolved port).
    pub fn local_addr(&self) -> SocketAddr {
        self.local
    }

    pub fn config(&self) -> &WifiConfig {
        &self.cfg
    }

    /// Accept one connection and complete pairing.
    pub async fn accept(&self, pairing_code: &str) -> Result<WifiTransport> {
        let (stream, _addr) = self
            .listener
            .accept()
            .await
            .map_err(|e| Error::Transport(format!("accept failed: {e}")))?;
        let session = server_handshake(stream, pairing_code, &self.cfg).await?;
        Ok(WifiTransport::from_session(
            session,
            self.cfg.clone(),
            pairing_code,
            Some(self.listener.clone()),
            None,
        ))
    }
}

impl fmt::Debug for WifiListener {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WifiListener")
            .field("local_addr", &self.local)
            .field("device_name", &self.cfg.device_name)
            .field("platform", &self.cfg.platform)
            .finish_non_exhaustive()
    }
}

/// A paired session that speaks the real wire protocol over TCP.
///
/// Server and client roles differ only in how the next connection is obtained
/// during [`Transport::reconnect`]: the listener side re-accepts, the client side
/// re-dials. The pairing code is kept in memory for that purpose, zeroized on
/// drop, and never printed or logged.
pub struct WifiTransport {
    reader: OwnedReadHalf,
    writer: OwnedWriteHalf,
    /// Set when this side accepted the connection, so `reconnect` can re-accept.
    listener: Option<Arc<TcpListener>>,
    /// Set when this side dialled, so `reconnect` can re-dial.
    connect_addr: Option<SocketAddr>,
    pairing_code: Zeroizing<String>,
    key: SecretKey,
    peer: PeerInfo,
    cfg: WifiConfig,
    connected: bool,
    opened_at: Instant,
    bytes_written: u64,
    bytes_read: u64,
    last_rtt_us: u64,
    reconnects: u64,
}

impl WifiTransport {
    /// Dial `addr` and pair using `pairing_code`.
    pub async fn connect(addr: SocketAddr, pairing_code: &str, cfg: WifiConfig) -> Result<Self> {
        let stream = TcpStream::connect(addr)
            .await
            .map_err(|e| Error::Transport(format!("cannot connect to {addr}: {e}")))?;
        let session = client_handshake(stream, pairing_code, &cfg).await?;
        Ok(Self::from_session(
            session,
            cfg,
            pairing_code,
            None,
            Some(addr),
        ))
    }

    fn from_session(
        session: Session,
        cfg: WifiConfig,
        pairing_code: &str,
        listener: Option<Arc<TcpListener>>,
        connect_addr: Option<SocketAddr>,
    ) -> Self {
        Self {
            reader: session.reader,
            writer: session.writer,
            listener,
            connect_addr,
            pairing_code: Zeroizing::new(pairing_code.to_string()),
            key: session.key,
            peer: session.peer,
            cfg,
            connected: true,
            opened_at: Instant::now(),
            bytes_written: 0,
            bytes_read: 0,
            last_rtt_us: 0,
            reconnects: 0,
        }
    }

    /// Peer identity reported during the handshake.
    pub fn peer(&self) -> &PeerInfo {
        &self.peer
    }

    pub fn is_connected(&self) -> bool {
        self.connected
    }

    /// How many times this session has re-established its link.
    pub fn reconnects(&self) -> u64 {
        self.reconnects
    }

    /// Swap in a freshly handshaken stream (used by `reconnect`).
    fn adopt(&mut self, session: Session) {
        self.reader = session.reader;
        self.writer = session.writer;
        self.key = session.key;
        self.peer = session.peer;
        self.connected = true;
        self.opened_at = Instant::now();
        self.reconnects += 1;
    }

    fn ensure_connected(&self) -> Result<()> {
        if self.connected {
            Ok(())
        } else {
            Err(Error::Transport("not connected".into()))
        }
    }

    /// The reserved manifest object id travels as a manifest frame, so the
    /// receiver can distinguish it from media chunks without extra state.
    fn chunk_kind(object_id: &str) -> FrameKind {
        if object_id == crate::engine::MANIFEST_OBJECT_ID {
            FrameKind::Manifest
        } else {
            FrameKind::Chunk
        }
    }
}

impl fmt::Debug for WifiTransport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WifiTransport")
            .field("peer", &self.peer)
            .field("connected", &self.connected)
            .field("bytes_written", &self.bytes_written)
            .field("bytes_read", &self.bytes_read)
            .field("reconnects", &self.reconnects)
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl Transport for WifiTransport {
    async fn discover(&mut self) -> Result<Vec<PeerInfo>> {
        // Network-wide discovery belongs to the listener/app ([`discovery`]); a
        // connected transport already knows exactly who it is paired with.
        self.ensure_connected()?;
        Ok(vec![self.peer.clone()])
    }

    async fn authenticate(&mut self, peer: &PeerInfo) -> Result<()> {
        self.ensure_connected()?;
        // Pairing already authenticated the channel; this checks the caller is
        // still talking to the peer it paired with.
        if peer.device_name != self.peer.device_name || peer.platform != self.peer.platform {
            return Err(Error::Protocol(
                "peer identity does not match the paired session".into(),
            ));
        }
        Ok(())
    }

    async fn open_session(&mut self) -> Result<()> {
        self.ensure_connected()
    }

    /// Send one chunk and wait for the peer's transport-level receipt.
    ///
    /// The returned [`VerifiedChunk`] means "the peer acknowledged this exact
    /// sealed frame" — content integrity is decided above, by the engine's
    /// per-chunk hash check (`crate::engine`).
    async fn send_chunk(&mut self, chunk: Chunk) -> Result<VerifiedChunk> {
        self.ensure_connected()?;
        let frame = Frame::for_object(
            Self::chunk_kind(&chunk.object_id),
            chunk.object_id.clone(),
            chunk.sequence,
            chunk.data.clone(),
        );
        protocol::write_frame(&mut self.writer, &frame, Some(&self.key)).await?;
        self.bytes_written += chunk.data.len() as u64;

        let started = Instant::now();
        let ack = protocol::read_frame(&mut self.reader, Some(&self.key)).await?;
        self.last_rtt_us = started.elapsed().as_micros() as u64;

        if ack.kind != FrameKind::Ack
            || ack.object_id != chunk.object_id
            || ack.sequence != chunk.sequence
        {
            return Err(Error::Protocol(format!(
                "expected an ack for {}@{}",
                chunk.object_id, chunk.sequence
            )));
        }
        Ok(VerifiedChunk {
            object_id: chunk.object_id,
            sequence: chunk.sequence,
        })
    }

    async fn receive_chunk(&mut self) -> Result<Chunk> {
        self.ensure_connected()?;
        let frame = protocol::read_frame(&mut self.reader, Some(&self.key)).await?;
        match frame.kind {
            FrameKind::Chunk | FrameKind::Manifest => {
                let ack = Frame::for_object(
                    FrameKind::Ack,
                    frame.object_id.clone(),
                    frame.sequence,
                    Vec::new(),
                );
                protocol::write_frame(&mut self.writer, &ack, Some(&self.key)).await?;
                self.bytes_read += frame.payload.len() as u64;
                Ok(Chunk {
                    object_id: frame.object_id,
                    sequence: frame.sequence,
                    data: frame.payload,
                })
            }
            FrameKind::Bye => {
                self.connected = false;
                Err(Error::Transport("peer closed the session".into()))
            }
            other => Err(Error::Protocol(format!(
                "unexpected {} frame while receiving chunks",
                other.name()
            ))),
        }
    }

    async fn health(&self) -> Result<LinkHealth> {
        if !self.connected {
            return Ok(LinkHealth {
                bytes_per_second: 0,
                rtt_ms: 0,
                loss_percent: 100,
            });
        }
        let elapsed = self.opened_at.elapsed().as_secs_f64().max(0.001);
        Ok(LinkHealth {
            bytes_per_second: (self.bytes_written as f64 / elapsed) as u64,
            rtt_ms: (self.last_rtt_us / 1_000) as u32,
            // TCP hides retransmits from us, so we cannot honestly claim loss.
            loss_percent: 0,
        })
    }

    /// Re-establish the link: the client re-dials, the listener re-accepts, and
    /// both re-run the pairing handshake. The engine then resumes from its
    /// checkpoint, so only missing chunks cross the new connection.
    async fn reconnect(&mut self) -> Result<()> {
        let session = if let Some(addr) = self.connect_addr {
            let stream = TcpStream::connect(addr)
                .await
                .map_err(|e| Error::Transport(format!("reconnect to {addr} failed: {e}")))?;
            client_handshake(stream, &self.pairing_code, &self.cfg).await?
        } else if let Some(listener) = self.listener.clone() {
            let (stream, _) = listener
                .accept()
                .await
                .map_err(|e| Error::Transport(format!("re-accept failed: {e}")))?;
            server_handshake(stream, &self.pairing_code, &self.cfg).await?
        } else {
            return Err(Error::Transport(
                "no reconnect path for this session".into(),
            ));
        };
        self.adopt(session);
        Ok(())
    }

    async fn close(&mut self) -> Result<()> {
        if self.connected {
            // Best effort: the peer may already be gone.
            let _ = protocol::write_frame(
                &mut self.writer,
                &Frame::bare(FrameKind::Bye, Vec::new()),
                Some(&self.key),
            )
            .await;
        }
        self.connected = false;
        Ok(())
    }
}

/// UDP discovery beacons (the Phase 4 "find a peer on the same Wi-Fi" step).
///
/// A beacon is deliberately tiny and content-free: device name, platform, the
/// TCP port to dial, and the protocol version. No media, no inventory, no keys
/// (L7). Because the TCP session is separately AEAD-authenticated, a forged or
/// replayed beacon can only waste a pairing attempt — it can never leak data or
/// change what a transfer does.
///
/// One UDP datagram instead of full mDNS keeps the core dependency-free and
/// deterministically testable; the apps can later swap in Bonjour/NSD behind
/// [`Discovery`] without touching the engine.
pub mod discovery {
    use super::*;

    /// Largest beacon we will send or accept.
    pub const MAX_BEACON_LEN: usize = 512;

    /// What a peer advertises. Contains no private content (L7).
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub struct Advertisement {
        pub device_name: String,
        pub platform: PeerPlatform,
        pub tcp_port: u16,
        pub protocol_version: u8,
    }

    impl Advertisement {
        /// Describe a listener so peers can dial it.
        pub fn for_listener(cfg: &WifiConfig, listener: &WifiListener) -> Self {
            Self {
                device_name: cfg.device_name.clone(),
                platform: cfg.platform,
                tcp_port: listener.local_addr().port(),
                protocol_version: PROTOCOL_VERSION,
            }
        }

        /// Turn a received beacon into a peer record the UI can show.
        pub fn to_peer(&self, from: SocketAddr) -> PeerInfo {
            PeerInfo {
                device_name: self.device_name.clone(),
                platform: self.platform,
                addresses: vec![format!("{}:{}", from.ip(), self.tcp_port)],
            }
        }
    }

    /// A UDP socket that can broadcast beacons and listen for them.
    pub struct Discovery {
        socket: UdpSocket,
        local: SocketAddr,
    }

    impl Discovery {
        /// Bind to `addr` (use port `0` for an ephemeral port in tests).
        pub async fn bind(addr: impl AsRef<str>) -> Result<Self> {
            let addr = addr.as_ref();
            let socket = UdpSocket::bind(addr).await.map_err(|e| {
                Error::Transport(format!("cannot bind discovery socket on {addr}: {e}"))
            })?;
            let local = socket
                .local_addr()
                .map_err(|e| Error::Transport(e.to_string()))?;
            Ok(Self { socket, local })
        }

        pub fn local_addr(&self) -> SocketAddr {
            self.local
        }

        /// Send one beacon to `target` — typically the subnet broadcast address
        /// `255.255.255.255:47807`, or a specific address in tests.
        pub async fn advertise(&self, adv: &Advertisement, target: SocketAddr) -> Result<()> {
            let payload = serde_json::to_vec(adv).map_err(|e| Error::Protocol(e.to_string()))?;
            if payload.len() > MAX_BEACON_LEN {
                return Err(Error::Protocol("discovery beacon too large".into()));
            }
            self.socket
                .send_to(&payload, target)
                .await
                .map_err(|e| Error::Transport(format!("beacon send failed: {e}")))?;
            Ok(())
        }

        /// Wait up to `timeout` for one beacon. `Ok(None)` means "nothing yet",
        /// which is a normal result on a quiet network, not an error.
        pub async fn recv_peer(&self, timeout: Duration) -> Result<Option<PeerInfo>> {
            let mut buf = [0u8; MAX_BEACON_LEN];
            match tokio::time::timeout(timeout, self.socket.recv_from(&mut buf)).await {
                Err(_) => Ok(None),
                Ok(Err(e)) => Err(Error::Transport(format!("beacon receive failed: {e}"))),
                Ok(Ok((len, from))) => {
                    let adv: Advertisement = serde_json::from_slice(&buf[..len])
                        .map_err(|_| Error::Protocol("malformed discovery beacon".into()))?;
                    // A peer speaking a different protocol version is simply not
                    // offered, rather than being shown and then failing to pair.
                    if adv.protocol_version != PROTOCOL_VERSION {
                        return Ok(None);
                    }
                    Ok(Some(adv.to_peer(from)))
                }
            }
        }
    }
}
