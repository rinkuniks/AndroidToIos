//! Transport abstraction (Plan Phase 1; v1.1 §4.1).
//!
//! The transfer engine must not care which transport is in use. Wi-Fi is the
//! default (L3); USB-C and desktop-bridge implement this same trait.

use crate::Result;
use async_trait::async_trait;

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
