//! Shared network-service constants.
//!
//! CopyPaste-8ebg.60: the STUN server used for public/WAN IP discovery was
//! hand-copied (with no fallback) into both `copypaste-daemon::public_ip`
//! (macOS) and `copypaste-android::stun` (Android FFI). Both crates already
//! depend on `copypaste-core`, so the server list is consolidated here as
//! the single source of truth, with a short fallback list in case the
//! primary server is unreachable.

/// STUN servers to try for reflexive (public/WAN) IPv4 discovery, in
/// priority order. Google's STUN servers are stable, publicly documented,
/// and require no auth.
pub const STUN_SERVERS: &[&str] = &["stun.l.google.com:19302", "stun1.l.google.com:19302"];
