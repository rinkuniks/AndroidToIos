//! Wire protocol: length-prefixed frames sealed with the session key
//! (Plan Phase 1; v1.1 §4, §7).
//!
//! ```text
//! magic(2)="WB" | version(1) | kind(1) | flags(1) | object_id_len(2 BE)
//!   | sequence(8 BE) | payload_len(4 BE) | object_id | payload
//! ```
//!
//! After pairing, every frame payload is sealed with AES-256-GCM under the
//! session key ([`crate::crypto`]), with the GCM additional authenticated data
//! derived from the header ([`frame_aad`]) — so a sealed chunk is bound to its
//! kind, object id and sequence and cannot be replayed into another slot.
//!
//! Two guard rails exist because a transport must never trust a peer's numbers:
//! lengths are validated **before** any allocation ([`MAX_PAYLOAD_LEN`],
//! [`MAX_OBJECT_ID_LEN`]), and the `sealed` flag must agree with the session's
//! expectations so a peer cannot negotiate a downgrade to plaintext.

use crate::crypto::{self, KeyPurpose, SecretKey};
use crate::{Error, Result};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

/// Wire format version. Bumping this is a breaking protocol change.
pub const PROTOCOL_VERSION: u8 = 1;
/// Frame magic — cheap sanity check that we are not reading garbage.
pub const MAGIC: [u8; 2] = *b"WB";
/// `magic(2) + version(1) + kind(1) + flags(1) + objlen(2) + seq(8) + paylen(4)`.
pub const HEADER_LEN: usize = 19;
/// Longest object id we will accept; chunk payloads carry the bulk, not ids.
pub const MAX_OBJECT_ID_LEN: usize = 4096;
/// Largest payload we will accept: one 8 MiB chunk plus AEAD overhead.
pub const MAX_PAYLOAD_LEN: usize = 8 * 1024 * 1024 + 4096;
/// Header flag: the payload is AEAD-sealed.
pub const FLAG_SEALED: u8 = 0x01;

/// Frame types carried by the session.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameKind {
    /// Client → listener: device identity + client nonce (unsealed).
    Hello = 1,
    /// Listener → client: device identity + server nonce (unsealed).
    HelloAck = 2,
    /// Manifest frame (reserved object id, sequence 0).
    Manifest = 3,
    /// One object chunk.
    Chunk = 4,
    /// Transport-level receipt for a chunk (see [`crate::transport::VerifiedChunk`]).
    Ack = 5,
    /// Polite session close.
    Bye = 6,
    /// Pairing proof: each side must seal this with the code-derived key, so
    /// possession of the code is proven without ever sending the code itself.
    Proof = 7,
}

impl FrameKind {
    pub const fn byte(self) -> u8 {
        self as u8
    }

    pub fn from_byte(byte: u8) -> Option<Self> {
        match byte {
            1 => Some(FrameKind::Hello),
            2 => Some(FrameKind::HelloAck),
            3 => Some(FrameKind::Manifest),
            4 => Some(FrameKind::Chunk),
            5 => Some(FrameKind::Ack),
            6 => Some(FrameKind::Bye),
            7 => Some(FrameKind::Proof),
            _ => None,
        }
    }

    /// Name for diagnostics and errors — never payload content or secrets (L7).
    pub const fn name(self) -> &'static str {
        match self {
            FrameKind::Hello => "hello",
            FrameKind::HelloAck => "hello_ack",
            FrameKind::Manifest => "manifest",
            FrameKind::Chunk => "chunk",
            FrameKind::Ack => "ack",
            FrameKind::Bye => "bye",
            FrameKind::Proof => "proof",
        }
    }
}

/// One protocol frame.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    pub kind: FrameKind,
    pub object_id: String,
    pub sequence: u64,
    pub payload: Vec<u8>,
}

impl Frame {
    /// A frame with no object binding (handshake, hello, bye).
    pub fn bare(kind: FrameKind, payload: Vec<u8>) -> Self {
        Self {
            kind,
            object_id: String::new(),
            sequence: 0,
            payload,
        }
    }

    /// A frame bound to one (object, sequence) slot.
    pub fn for_object(
        kind: FrameKind,
        object_id: impl Into<String>,
        sequence: u64,
        payload: Vec<u8>,
    ) -> Self {
        Self {
            kind,
            object_id: object_id.into(),
            sequence,
            payload,
        }
    }
}

/// AAD binding a sealed frame to its kind and (object, sequence) slot.
pub fn frame_aad(kind: FrameKind, object_id: &str, sequence: u64) -> Vec<u8> {
    let mut aad = Vec::with_capacity(20 + object_id.len());
    aad.extend_from_slice(b"wa-bridge/frame/v1");
    aad.push(0x00);
    aad.push(kind.byte());
    aad.push(0x00);
    aad.extend_from_slice(object_id.as_bytes());
    aad.push(0x00);
    aad.extend_from_slice(&sequence.to_be_bytes());
    aad
}

/// Encode `frame` into wire bytes, sealing the payload when `key` is `Some`.
pub fn encode(frame: &Frame, key: Option<&SecretKey>) -> Result<Vec<u8>> {
    if frame.object_id.len() > MAX_OBJECT_ID_LEN {
        return Err(Error::Protocol("object id too long".into()));
    }

    let (flags, payload) = match key {
        Some(key) => (
            FLAG_SEALED,
            crypto::seal(
                key,
                KeyPurpose::Session,
                &frame_aad(frame.kind, &frame.object_id, frame.sequence),
                &frame.payload,
            )?,
        ),
        None => (0u8, frame.payload.clone()),
    };

    if payload.len() > MAX_PAYLOAD_LEN {
        return Err(Error::Protocol("payload too large".into()));
    }

    let mut out = Vec::with_capacity(HEADER_LEN + frame.object_id.len() + payload.len());
    out.extend_from_slice(&MAGIC);
    out.push(PROTOCOL_VERSION);
    out.push(frame.kind.byte());
    out.push(flags);
    out.extend_from_slice(&(frame.object_id.len() as u16).to_be_bytes());
    out.extend_from_slice(&frame.sequence.to_be_bytes());
    out.extend_from_slice(&(payload.len() as u32).to_be_bytes());
    out.extend_from_slice(frame.object_id.as_bytes());
    out.extend_from_slice(&payload);
    Ok(out)
}

/// Write one frame.
pub async fn write_frame<W: AsyncWrite + Unpin>(
    w: &mut W,
    frame: &Frame,
    key: Option<&SecretKey>,
) -> Result<()> {
    let bytes = encode(frame, key)?;
    w.write_all(&bytes)
        .await
        .map_err(|e| Error::Transport(format!("frame write failed: {e}")))?;
    w.flush()
        .await
        .map_err(|e| Error::Transport(format!("frame flush failed: {e}")))
}

/// Read one frame, opening the payload when `key` is `Some`.
pub async fn read_frame<R: AsyncRead + Unpin>(r: &mut R, key: Option<&SecretKey>) -> Result<Frame> {
    let mut header = [0u8; HEADER_LEN];
    r.read_exact(&mut header)
        .await
        .map_err(|e| Error::Transport(format!("frame read failed: {e}")))?;

    if header[0..2] != MAGIC {
        return Err(Error::Protocol("bad frame magic".into()));
    }
    if header[2] != PROTOCOL_VERSION {
        return Err(Error::Protocol(format!(
            "unsupported protocol version {}",
            header[2]
        )));
    }
    let kind = FrameKind::from_byte(header[3])
        .ok_or_else(|| Error::Protocol(format!("unknown frame kind {}", header[3])))?;
    let flags = header[4];

    // Validate the declared lengths *before* allocating anything for them.
    let object_id_len = u16::from_be_bytes([header[5], header[6]]) as usize;
    if object_id_len > MAX_OBJECT_ID_LEN {
        return Err(Error::Protocol("object id too long".into()));
    }
    let sequence = u64::from_be_bytes(
        header[7..15]
            .try_into()
            .map_err(|_| Error::Protocol("malformed sequence field".into()))?,
    );
    let payload_len = u32::from_be_bytes(
        header[15..19]
            .try_into()
            .map_err(|_| Error::Protocol("malformed length field".into()))?,
    ) as usize;
    if payload_len > MAX_PAYLOAD_LEN {
        return Err(Error::Protocol("payload too large".into()));
    }

    let mut id_bytes = vec![0u8; object_id_len];
    r.read_exact(&mut id_bytes)
        .await
        .map_err(|e| Error::Transport(format!("frame read failed: {e}")))?;
    let object_id = String::from_utf8(id_bytes)
        .map_err(|_| Error::Protocol("object id is not valid utf-8".into()))?;

    let mut payload = vec![0u8; payload_len];
    r.read_exact(&mut payload)
        .await
        .map_err(|e| Error::Transport(format!("frame read failed: {e}")))?;

    let sealed = flags & FLAG_SEALED != 0;
    let payload = match (sealed, key) {
        (false, None) => payload,
        (true, Some(key)) => crypto::open(
            key,
            KeyPurpose::Session,
            &frame_aad(kind, &object_id, sequence),
            &payload,
        )?,
        (true, None) => {
            return Err(Error::Protocol(
                "sealed frame received but no session key is set".into(),
            ))
        }
        (false, Some(_)) => {
            return Err(Error::Protocol(
                "unsealed frame received where a sealed frame was expected".into(),
            ))
        }
    };

    Ok(Frame {
        kind,
        object_id,
        sequence,
        payload,
    })
}
