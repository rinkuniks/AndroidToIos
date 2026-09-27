//! WA Bridge shared transfer core.
//!
//! Transport-agnostic engine: session, manifest, chunking, hashing,
//! checkpoint/resume. Shared by Android (JNI), iOS/iPadOS (FFI), and desktop
//! (native bindings). Scope locks L1–L8 apply (see repo README).

pub mod checkpoint;
pub mod crypto;
pub mod engine;
pub mod error;
pub mod manifest;
pub mod session;
pub mod transport;

pub use crypto::{KeyPurpose, SecretKey};
pub use error::{Error, Result};
pub use manifest::*;
pub use session::Session;
pub use transport::*;
