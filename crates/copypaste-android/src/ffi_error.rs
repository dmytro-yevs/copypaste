//! Shared FFI error type exported via UniFFI.
//!
//! `CopypasteError` is the single flat-error enum used by every `[Throws]`
//! function in the UDL.  It lives here so the core submodules can import it
//! with `use crate::CopypasteError` while the generated scaffolding still
//! resolves it at `crate::CopypasteError` (re-exported from `lib.rs`).

use crate::panic_boundary::PanicError;

// When using UDL-based scaffolding, uniffi::Error and uniffi::Record proc-macro
// derives conflict with the generated scaffolding. Only thiserror is needed here.
#[derive(Debug, thiserror::Error)]
pub enum CopypasteError {
    #[error("Encryption failed")]
    EncryptionFailed,
    #[error("Decryption failed: {reason}")]
    DecryptionFailed { reason: String },
    #[error("Database error: {reason}")]
    DatabaseError { reason: String },
    #[error("Invalid key length: expected 32")]
    InvalidKeyLength,
    /// P2P pairing / transport failure surfaced from `copypaste_p2p`
    /// (`TransportError`): TLS, socket, framing, or PAKE handshake errors —
    /// including a wrong pairing password or a channel-binding MitM abort. Also
    /// raised for a malformed `addr_hint` that cannot be parsed into a
    /// `SocketAddr`. The `reason` carries the underlying error's display form.
    #[error("P2P pairing failed: {reason}")]
    P2pError { reason: String },
    /// v0.3 (OI-7): a Rust panic was caught at the FFI boundary by
    /// [`panic_boundary::catch_result`]. Carries the panic message so Kotlin
    /// can log/surface it instead of seeing a JVM-killing abort.
    ///
    /// NOTE: the field is named `reason` (not `message`) on purpose — a UniFFI
    /// flat-error variant field named `message` collides with the Kotlin
    /// `Throwable.message` supertype property and produces "conflicting
    /// declarations" / missing-`override` codegen errors. See the generated
    /// `CopypasteException` binding.
    #[error("Panicked: {reason}")]
    Panicked { reason: String },
}

impl From<PanicError> for CopypasteError {
    fn from(p: PanicError) -> Self {
        match p {
            PanicError::Panicked(reason) => CopypasteError::Panicked { reason },
        }
    }
}
