//! Transport abstraction (Plan Phase 1; v1.1 §4.1).
//!
//! The transfer engine must not care which transport is in use. Wi-Fi is the
//! default (L3); USB-C and desktop-bridge implement this same trait.

use crate::{Error, Result};
use async_trait::async_trait;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::mpsc;

/// Discovery record for a peer device.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerInfo {
    pub device_name: String,
    pub platform: PeerPlatform,
    pub addresses: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PeerPlatform {
    Android,
    Iphone,
    Ipad,
}

/// Health snapshot used by Smart Connect (Phase 4) to decide fallback UX.
#[derive(Debug, Clone, Copy)]
pub struct LinkHealth {
    pub bytes_per_second: u64,
    pub rtt_ms: u32,
    pub loss_percent: u8,
}

/// Chunk envelope — wire format is transport-independent.
#[derive(Debug, Clone)]
pub struct Chunk {
    pub object_id: String,
    pub sequence: u64,
    pub data: Vec<u8>,
}

/// A chunk acknowledged as **verified** (hash-checked) by the receiver.
/// Only verified chunks advance the resume point (Plan Phase 1 gate).
#[derive(Debug, Clone)]
pub struct VerifiedChunk {
    pub object_id: String,
    pub sequence: u64,
}

#[async_trait]
pub trait Transport: Send + Sync {
    /// Find peers on the local network (or cable). Must never use mobile data.
    async fn discover(&mut self) -> Result<Vec<PeerInfo>>;

    /// Establish a mutually authenticated, encrypted session with the peer.
    async fn authenticate(&mut self, peer: &PeerInfo) -> Result<()>;

    async fn open_session(&mut self) -> Result<()>;

    async fn send_chunk(&mut self, chunk: Chunk) -> Result<VerifiedChunk>;

    async fn receive_chunk(&mut self) -> Result<Chunk>;

    async fn health(&self) -> Result<LinkHealth>;

    /// Attempt transparent reconnection; on success the session resumes from
    /// the last verified checkpoint.
    async fn reconnect(&mut self) -> Result<()>;

    async fn close(&mut self) -> Result<()>;
}

/// In-memory loopback transport pair — for tests and the interruption harness
/// (Plan Phase 1). Delivers chunks through an unbounded async channel and lets
/// tests cut/restore the link via [`FaultHandle`] to exercise retry logic.
pub struct LoopbackTransport {
    tx: mpsc::UnboundedSender<Chunk>,
    rx: mpsc::UnboundedReceiver<Chunk>,
    peer: PeerInfo,
    connected: bool,
    broken: std::sync::Arc<AtomicBool>,
}

/// Handle that lets a test simulate a cable being unplugged or re-plugged.
#[derive(Clone)]
pub struct FaultHandle {
    broken: std::sync::Arc<AtomicBool>,
}

impl FaultHandle {
    pub fn cut(&self) {
        self.broken.store(true, Ordering::SeqCst);
    }
    pub fn restore(&self) {
        self.broken.store(false, Ordering::SeqCst);
    }
    pub fn is_cut(&self) -> bool {
        self.broken.load(Ordering::SeqCst)
    }
}

/// Create two connected loopback endpoints + a shared fault handle.
pub fn loopback_pair(
    peer_a: PeerInfo,
    peer_b: PeerInfo,
) -> (LoopbackTransport, LoopbackTransport, FaultHandle) {
    let (tx_a, rx_a) = mpsc::unbounded_channel();
    let (tx_b, rx_b) = mpsc::unbounded_channel();
    let broken = std::sync::Arc::new(AtomicBool::new(false));
    let a = LoopbackTransport {
        tx: tx_b,
        rx: rx_a,
        peer: peer_a,
        connected: true,
        broken: broken.clone(),
    };
    let b = LoopbackTransport {
        tx: tx_a,
        rx: rx_b,
        peer: peer_b,
        connected: true,
        broken: broken.clone(),
    };
    let fault = FaultHandle { broken };
    let mut a = a;
    a.broken = fault.broken.clone();
    (a, b, fault)
}

impl LoopbackTransport {
    fn link_ok(&self) -> Result<()> {
        if !self.connected {
            return Err(Error::Transport("not connected".into()));
        }
        if self.broken.load(Ordering::SeqCst) {
            return Err(Error::Transport("connection lost".into()));
        }
        Ok(())
    }
}

#[async_trait]
impl Transport for LoopbackTransport {
    async fn discover(&mut self) -> Result<Vec<PeerInfo>> {
        self.link_ok()?;
        Ok(vec![self.peer.clone()])
    }

    async fn authenticate(&mut self, _peer: &PeerInfo) -> Result<()> {
        self.link_ok()
    }

    async fn open_session(&mut self) -> Result<()> {
        self.link_ok()
    }

    async fn send_chunk(&mut self, chunk: Chunk) -> Result<VerifiedChunk> {
        self.link_ok()?;
        self.tx
            .send(chunk.clone())
            .map_err(|_| Error::Transport("peer gone".into()))?;
        Ok(VerifiedChunk {
            object_id: chunk.object_id,
            sequence: chunk.sequence,
        })
    }

    async fn receive_chunk(&mut self) -> Result<Chunk> {
        self.link_ok()?;
        self.rx
            .recv()
            .await
            .ok_or_else(|| Error::Transport("channel closed".into()))
    }

    async fn health(&self) -> Result<LinkHealth> {
        if self.connected && !self.broken.load(Ordering::SeqCst) {
            Ok(LinkHealth {
                bytes_per_second: 1_000_000_000, // unconstrained in-memory
                rtt_ms: 0,
                loss_percent: 0,
            })
        } else {
            Ok(LinkHealth {
                bytes_per_second: 0,
                rtt_ms: 0,
                loss_percent: 100,
            })
        }
    }

    async fn reconnect(&mut self) -> Result<()> {
        if !self.connected {
            return Err(Error::Transport("not connected".into()));
        }
        self.broken.store(false, Ordering::SeqCst);
        Ok(())
    }

    async fn close(&mut self) -> Result<()> {
        self.connected = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peer(kind: PeerPlatform) -> PeerInfo {
        PeerInfo {
            device_name: "tester".into(),
            platform: kind,
            addresses: vec![],
        }
    }

    #[tokio::test]
    async fn loopback_roundtrips_and_cut_breaks_send() {
        let (mut a, mut b, fault) =
            loopback_pair(peer(PeerPlatform::Android), peer(PeerPlatform::Iphone));

        let chunk = Chunk {
            object_id: "o".into(),
            sequence: 0,
            data: b"ping".to_vec(),
        };
        let ack = a.send_chunk(chunk).await.unwrap();
        assert_eq!(ack.sequence, 0);

        let received = b.receive_chunk().await.unwrap();
        assert_eq!(received.data, b"ping");
        assert!(!fault.is_cut());

        // Cutting the link makes the next send fail until reconnect.
        fault.cut();
        let cut_chunk = Chunk {
            object_id: "o".into(),
            sequence: 1,
            data: b"x".into(),
        };
        assert!(a.send_chunk(cut_chunk).await.is_err());
        a.reconnect().await.unwrap();
        let ack = a
            .send_chunk(Chunk {
                object_id: "o".into(),
                sequence: 2,
                data: b"x".into(),
            })
            .await
            .unwrap();
        assert_eq!(ack.sequence, 2);
    }
}
