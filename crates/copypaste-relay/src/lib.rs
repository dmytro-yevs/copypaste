//! Self-hosted relay for CopyPaste sync across networks.
//!
//! The relay connects two devices that share a pairing and forwards opaque
//! frames between them. Devices run their end-to-end Noise session through
//! it, so the relay never sees clipboard content, metadata or keys, and it
//! stores nothing.

#![forbid(unsafe_code)]

pub mod proto;
pub mod server;

pub use server::{observe, serve, Config};
