//! WA Bridge shared transfer core.
//!
//! Transport-agnostic engine: session, manifest, chunking, hashing,
//! checkpoint/resume. Shared by Android (JNI), iOS/iPadOS (FFI), and desktop
//! (native bindings). Scope locks L1–L8 apply (see repo README).

pub mod checkpoint;
pub mod error;
pub mod manifest;
pub mod transport;

pub use error::{Error, Result};
