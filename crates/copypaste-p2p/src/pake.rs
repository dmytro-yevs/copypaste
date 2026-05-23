//! PAKE (Password-Authenticated Key Exchange) for device pairing.
//!
//! This module implements an augmented PAKE handshake on top of
//! [`opaque_ke`] 3.0 (Ristretto255 + Argon2 ciphersuite). Two devices that
//! share a short pairing code (e.g. `6-digit` or `4-word` passphrase) can
//! derive a 32-byte shared [`SessionKey`] without ever transmitting the
//! pairing code or anything useful to an offline brute-forcer.
//!
//! See **ADR-008** (`docs/adr/ADR-008-pake-protocol-choice.md`) for the
//! protocol decision, wire-format, and storage rationale.
//!
//! # Wire format (3-message handshake)
//!
//! ```text
//! Client (initiator)                          Server (responder)
//!   | --- 1. ClientLogin start            -->  |
//!   | <-- 2. ServerLogin start             --- |
//!   | --- 3. ClientLogin finish           -->  |
//!   | == both sides hold the same SessionKey == |
//! ```
//!
//! Real implementation lands in **Wave 2.4**. This file ships the public API
//! skeleton + ciphersuite type so downstream crates can start wiring
//! pairing UX against stable type signatures.

use thiserror::Error;

/// Errors that can occur during a PAKE handshake.
#[derive(Debug, Error)]
pub enum PakeError {
    /// Peer presented a credential that did not validate against the stored
    /// `PasswordFile`. Returned to both sides; never reveals which side was
    /// wrong (per OPAQUE design).
    #[error("invalid password")]
    InvalidPassword,

    /// Underlying opaque-ke / cryptography failure. The string is intended
    /// for logging only — never surface it to end-users verbatim.
    #[error("protocol error: {0}")]
    Protocol(String),

    /// Message could not be decoded from the wire format (wrong length,
    /// version tag mismatch, etc.).
    #[error("wire format error: {0}")]
    WireFormat(String),

    /// Caller invoked a step out of order (e.g. `finish` before `respond`).
    #[error("handshake state error: {0}")]
    State(&'static str),
}

/// 32-byte session key derived by both sides on successful handshake.
///
/// This is the seed for HKDF expansion to the XChaCha20-Poly1305 key used by
/// the envelope (ADR-001). Wrapped in a newtype so it does not implement
/// `Debug` / `Display` / `Serialize` by accident.
pub struct SessionKey(pub [u8; 32]);

impl SessionKey {
    /// Borrow the raw bytes. Caller is responsible for `zeroize` if needed.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// Server-side password material derived during initial registration.
///
/// Persisted in SQLCipher (`paired_peers.pake_password_file BLOB`). First
/// byte is a version tag (`0x01` for opaque-ke 3.0 / Ristretto255-Argon2)
/// so future PAKE migrations can co-exist on the same row.
pub struct PasswordFile {
    /// Versioned serialised opaque-ke `ServerRegistration` blob.
    pub serialized: Vec<u8>,
}

/// Initiator (client) side of the PAKE handshake.
///
/// Holds the in-flight `opaque_ke::ClientLogin` state between `new` and
/// `finish`. Drop the value to abort the handshake (state is zeroized).
pub struct PakeInitiator {
    // Wave 2.4: `state: ClientLogin<DefaultCipherSuite>`
    _private: (),
}

impl PakeInitiator {
    /// Step 1: client begins the handshake with the shared pairing
    /// password. Returns `(Self, message_to_send)` — send the bytes to the
    /// responder over the framed transport, then call [`Self::finish`] with
    /// the response.
    pub fn new(_password: &str) -> Result<(Self, Vec<u8>), PakeError> {
        unimplemented!("Wave 2.4: implement full opaque-ke ClientLogin::start flow")
    }

    /// Step 3: client receives the server's response and derives the
    /// session key. Consumes `self` because the handshake state is
    /// single-use.
    pub fn finish(self, _server_message: &[u8]) -> Result<SessionKey, PakeError> {
        unimplemented!("Wave 2.4: implement opaque-ke ClientLogin::finish flow")
    }
}

/// Responder (server) side of the PAKE handshake.
///
/// Holds the in-flight `opaque_ke::ServerLogin` state between `respond` and
/// `finish`. Drop the value to abort the handshake (state is zeroized).
pub struct PakeResponder {
    // Wave 2.4: `state: ServerLogin<DefaultCipherSuite>`
    _private: (),
}

impl PakeResponder {
    /// Step 2: server receives the client's opening message and responds.
    /// Requires the persisted [`PasswordFile`] for the peer being paired.
    /// Returns `(Self, message_to_send)`.
    pub fn respond(
        _password_file: &PasswordFile,
        _client_message: &[u8],
    ) -> Result<(Self, Vec<u8>), PakeError> {
        unimplemented!("Wave 2.4: implement opaque-ke ServerLogin::start flow")
    }

    /// Step 4 (server side): after receiving the client's final
    /// authenticator, finalise and derive the session key.
    pub fn finish(self, _client_final: &[u8]) -> Result<SessionKey, PakeError> {
        unimplemented!("Wave 2.4: implement opaque-ke ServerLogin::finish flow")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pake_module_compiles() {
        // Smoke test — confirms the public API surface is well-formed.
        // Full handshake round-trip lands in Wave 2.4.
        let key = SessionKey([0u8; 32]);
        assert_eq!(key.as_bytes().len(), 32);

        let pf = PasswordFile {
            serialized: vec![0x01],
        };
        assert_eq!(pf.serialized[0], 0x01, "version tag must be 0x01");
    }

    #[test]
    fn pake_error_displays() {
        let err = PakeError::InvalidPassword;
        assert_eq!(err.to_string(), "invalid password");

        let err = PakeError::Protocol("oprf failed".into());
        assert!(err.to_string().contains("oprf failed"));
    }
}
