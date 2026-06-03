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
//! # Persisted state
//!
//! Successful pairing produces a [`PasswordFile`] that the responder must
//! persist (encrypted at rest in SQLCipher: `paired_peers.pake_password_file
//! BLOB`). The first byte is a version tag (`0x01`) so future PAKE migrations
//! can co-exist on the same row. The remaining bytes are the concatenation
//! of `ServerSetup` (server's long-term OPAQUE key) and `ServerRegistration`
//! (per-peer envelope) — both required to run `ServerLogin::start` later.

use argon2::Argon2;
use generic_array::GenericArray;
use hkdf::Hkdf;
use opaque_ke::ciphersuite::CipherSuite;
use opaque_ke::{
    ClientLogin, ClientLoginFinishParameters, ClientRegistration,
    ClientRegistrationFinishParameters, CredentialFinalization, CredentialRequest,
    CredentialResponse, RegistrationRequest, RegistrationResponse, RegistrationUpload, ServerLogin,
    ServerLoginStartParameters, ServerRegistration, ServerSetup,
};
use rand::rngs::OsRng;
use sha2::Sha256;
use thiserror::Error;

/// Wire-format version tag for [`PasswordFile`] serialisation.
///
/// `0x01` = opaque-ke 3.0, Ristretto255 + Argon2 (default) + TripleDH KE.
/// Bump on any ciphersuite / serialisation change.
const PASSWORD_FILE_VERSION: u8 = 0x01;

/// Stable per-pairing "user" identifier passed into OPAQUE.
///
/// OPAQUE binds a `username` (server-side credential identifier) into the
/// envelope. In CopyPaste pairing there is exactly one credential per
/// `PasswordFile`, so the identifier is a fixed sentinel — peer identity is
/// already enforced by the surrounding TLS certificate-fingerprint pinning.
const PAIRING_USERNAME: &[u8] = b"copypaste-pair";

/// OPAQUE ciphersuite — Ristretto255 OPRF + KeGroup, TripleDH key exchange,
/// Argon2id KSF. Chosen in ADR-008.
struct CopypasteCipherSuite;

impl CipherSuite for CopypasteCipherSuite {
    type OprfCs = opaque_ke::Ristretto255;
    type KeGroup = opaque_ke::Ristretto255;
    type KeyExchange = opaque_ke::key_exchange::tripledh::TripleDh;
    type Ksf = Argon2<'static>;
}

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
#[derive(zeroize::ZeroizeOnDrop)]
pub struct SessionKey(pub [u8; 32]);

impl SessionKey {
    /// Borrow the raw bytes. Caller is responsible for `zeroize` if needed.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Derive a 32-byte XChaCha20-Poly1305 key from this session key via
    /// HKDF-SHA256, mixing in a caller-supplied `salt` (per ADR-001 chunked
    /// encryption — each chunk / envelope gets a fresh subkey).
    ///
    /// The info string `"copypaste-xchacha20-key-v1"` domain-separates this
    /// derivation from any future use of the same `SessionKey`.
    ///
    /// Returns a [`zeroize::Zeroizing`]-wrapped key so the sensitive material is
    /// wiped on drop at call sites (parity with [`Self::bind_to_tls_channel`]).
    /// `Zeroizing<[u8; 32]>` derefs to `[u8; 32]` / `&[u8]`, so passing it where
    /// a key slice is expected needs no change; only a by-value `[u8; 32]` move
    /// requires an explicit deref (`*key`).
    pub fn derive_xchacha_key(&self, salt: &[u8]) -> zeroize::Zeroizing<[u8; 32]> {
        let hk = Hkdf::<Sha256>::new(Some(salt), &self.0);
        let mut out = [0u8; 32];
        hk.expand(b"copypaste-xchacha20-key-v1", &mut out)
            .expect("32 bytes is well within HKDF-SHA256 output limit");
        zeroize::Zeroizing::new(out)
    }

    /// Derive a 32-byte *channel-bound* session key by mixing in a TLS
    /// channel-binding token (RFC 5705 `export_keying_material`).
    ///
    /// # Protocol (S3 — PAKE/TLS channel binding)
    ///
    /// After the PAKE handshake both sides hold the same raw `SessionKey`.
    /// However, an active MitM that can relay PAKE messages over separate TLS
    /// connections could bridge two sessions and learn the shared key. Binding
    /// the key to the specific TLS channel prevents this: even if the PAKE
    /// completes, the derived key is useless on any other channel.
    ///
    /// ```text
    /// tls_binder = TlsStream::export_keying_material(
    ///     label   = "EXPORTER-copypaste-channel-binding",
    ///     context = None,          // RFC 5705 §4 — context omitted
    ///     len     = 32,
    /// )
    /// session_binding = HKDF-SHA256(
    ///     salt = tls_binder,
    ///     ikm  = self.0 (raw PAKE session key),
    ///     info = b"copypaste/p2p/channel-binding/v1",
    /// )
    /// ```
    ///
    /// The `context = None` form is intentional: passing the session key as
    /// context would create a circular dependency (the binder would depend on
    /// the key we are trying to protect). RFC 5705 §4 explicitly allows the
    /// no-context form.
    ///
    /// # Arguments
    ///
    /// * `tls_binder` — 32 bytes from `export_keying_material` on the
    ///   completed TLS handshake. Both sides must use identical label +
    ///   context; the TLS record layer guarantees they will get the same
    ///   bytes when connected to each other, and different bytes on any
    ///   other channel.
    ///
    /// # Panics
    ///
    /// Panics if `tls_binder` is empty (programming error — callers must
    /// not pass a zero-length slice).
    pub fn bind_to_tls_channel(&self, tls_binder: &[u8]) -> zeroize::Zeroizing<[u8; 32]> {
        assert!(!tls_binder.is_empty(), "tls_binder must not be empty");
        let hk = Hkdf::<Sha256>::new(Some(tls_binder), &self.0);
        let mut out = [0u8; 32];
        hk.expand(b"copypaste/p2p/channel-binding/v1", &mut out)
            .expect("32 bytes is well within HKDF-SHA256 output limit");
        zeroize::Zeroizing::new(out)
    }
}

/// Which endpoint a [`channel_confirmation_tag`] belongs to.
///
/// The two roles derive *different* tags from the same channel-bound key so a
/// relay cannot simply reflect one side's tag back to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfirmRole {
    /// The PAKE initiator (bootstrap TLS client).
    Initiator,
    /// The PAKE responder (bootstrap TLS server).
    Responder,
}

/// Length of a channel-binding confirmation tag, in bytes.
pub const CONFIRM_TAG_LEN: usize = 32;

/// Derive a 32-byte confirmation tag from a TLS-channel-bound session key.
///
/// # Protocol (S3 — bootstrap pairing confirmation)
///
/// After both sides bind the PAKE `SessionKey` to their TLS channel (via
/// [`SessionKey::bind_to_tls_channel`]) they exchange these tags as the final
/// step of bootstrap pairing. Each side sends its own-role tag and verifies the
/// peer's opposite-role tag with a **constant-time** compare. A match proves the
/// peer holds *both* the same PAKE key *and* the same TLS channel binder.
///
/// A relay MitM that bridges PAKE over two distinct TLS sessions ends up with a
/// different channel binder on each leg, so the bound key (and therefore every
/// derived tag) differs on the two legs and the compare fails — aborting pairing.
///
/// The tag is derived by HKDF-SHA256 over the channel-bound key with a
/// role-separated info string, so the initiator's and responder's tags differ
/// and cannot be reflected.
pub fn channel_confirmation_tag(bound_key: &[u8; 32], role: ConfirmRole) -> [u8; CONFIRM_TAG_LEN] {
    let info: &[u8] = match role {
        ConfirmRole::Initiator => b"copypaste/p2p/channel-confirm/v1/initiator",
        ConfirmRole::Responder => b"copypaste/p2p/channel-confirm/v1/responder",
    };
    let hk = Hkdf::<Sha256>::new(None, bound_key);
    let mut tag = [0u8; CONFIRM_TAG_LEN];
    hk.expand(info, &mut tag)
        .expect("32 bytes is well within HKDF-SHA256 output limit");
    tag
}

/// Number of decimal digits in a Short Authentication String (SAS).
///
/// 6 digits ≈ 20 bits of entropy. Combined with single-shot ephemeral keys and
/// abort-on-mismatch this caps an online MitM forgery at ~1-in-10^6 per attempt
/// (Bluetooth numeric-comparison / Magic-Wormhole verifier pattern).
pub const SAS_DIGITS: usize = 6;

/// Derive a human-comparable 6-digit Short Authentication String (SAS) from a
/// TLS-channel-bound session key.
///
/// # Protocol (LAN/SAS pairing)
///
/// On the discovery pairing path there is NO pre-shared secret: bootstrap runs
/// with an ephemeral password the initiator sends in-clear inside the bootstrap
/// TLS channel, and authentication is provided ENTIRELY by the human comparing
/// the SAS on both screens. The SAS MUST derive from `bound_key` (post-PAKE,
/// post-channel-binding) so a relay/MitM substituting its own password per leg
/// derives a different `bound_key` per leg → a different SAS per leg → the user
/// sees a mismatch and aborts.
///
/// The info string is domain-separated from
/// [`channel_confirmation_tag`]'s strings so the displayed SAS is not a
/// truncation of either confirmation tag.
///
/// # Digit math
///
/// HKDF-SHA256 (no salt, IKM = `bound_key`, info = `b"copypaste/p2p/sas/v1"`)
/// expands to 4 bytes; those are interpreted big-endian as a `u32`, reduced
/// `% 1_000_000`, and zero-padded to [`SAS_DIGITS`] decimal digits. The
/// reduction introduces a negligible modulo bias (2^32 mod 10^6) that does not
/// meaningfully weaken the ~20-bit single-shot guarantee.
pub fn derive_sas(bound_key: &[u8; 32]) -> String {
    let hk = Hkdf::<Sha256>::new(None, bound_key);
    let mut out = [0u8; 4];
    hk.expand(b"copypaste/p2p/sas/v1", &mut out)
        .expect("4 bytes is well within HKDF-SHA256 output limit");
    let n = u32::from_be_bytes(out) % 1_000_000;
    format!("{n:06}")
}

/// Server-side password material derived during initial registration.
///
/// Persisted in SQLCipher (`paired_peers.pake_password_file BLOB`). First
/// byte is a version tag (`0x01` for opaque-ke 3.0 / Ristretto255-Argon2)
/// followed by a 2-byte big-endian length of the `ServerSetup` blob, then
/// `ServerSetup` bytes, then `ServerRegistration` bytes (length implied by
/// remaining slice). Both are required to drive `ServerLogin::start`.
///
/// # Zeroization
///
/// `serialized` contains the OPAQUE server long-term key material
/// (`ServerSetup`) concatenated with the per-peer envelope
/// (`ServerRegistration`). Both are sensitive: `ServerSetup` is a long-lived
/// private key; `ServerRegistration` encodes the verifier that protects the
/// pairing password. `ZeroizeOnDrop` ensures the heap buffer is wiped when
/// the `PasswordFile` is dropped, including on panic / early-return paths.
#[derive(Clone, zeroize::ZeroizeOnDrop)]
pub struct PasswordFile {
    /// Versioned serialised blob — see struct docs for layout.
    pub serialized: Vec<u8>,
}

impl PasswordFile {
    /// Perform a one-time OPAQUE registration for `password` and produce the
    /// persistable [`PasswordFile`]. Runs the full 3-message registration
    /// flow locally (single party plays both roles) because pairing UX is
    /// "set the same code on both devices, then handshake".
    pub fn register(password: &str) -> Result<Self, PakeError> {
        let mut rng = OsRng;

        // 1. Server long-term setup (per-peer in our model — small enough,
        //    and rotating it on re-pair is the desired behaviour).
        let server_setup = ServerSetup::<CopypasteCipherSuite>::new(&mut rng);

        // 2. Client registration start.
        let client_start =
            ClientRegistration::<CopypasteCipherSuite>::start(&mut rng, password.as_bytes())
                .map_err(|e| PakeError::Protocol(format!("client reg start: {e}")))?;
        let reg_req_bytes = client_start.message.serialize();

        // 3. Server registration start.
        let reg_req = RegistrationRequest::deserialize(&reg_req_bytes)
            .map_err(|e| PakeError::WireFormat(format!("reg request: {e}")))?;
        let server_start = ServerRegistration::<CopypasteCipherSuite>::start(
            &server_setup,
            reg_req,
            PAIRING_USERNAME,
        )
        .map_err(|e| PakeError::Protocol(format!("server reg start: {e}")))?;
        let reg_resp_bytes = server_start.message.serialize();

        // 4. Client registration finish.
        let reg_resp = RegistrationResponse::deserialize(&reg_resp_bytes)
            .map_err(|e| PakeError::WireFormat(format!("reg response: {e}")))?;
        let client_finish = client_start
            .state
            .finish(
                &mut rng,
                password.as_bytes(),
                reg_resp,
                ClientRegistrationFinishParameters::default(),
            )
            .map_err(|e| PakeError::Protocol(format!("client reg finish: {e}")))?;
        let upload_bytes = client_finish.message.serialize();

        // 5. Server registration finalise.
        let upload = RegistrationUpload::<CopypasteCipherSuite>::deserialize(&upload_bytes)
            .map_err(|e| PakeError::WireFormat(format!("reg upload: {e}")))?;
        let password_file_inner = ServerRegistration::<CopypasteCipherSuite>::finish(upload);

        // 6. Pack: [version u8][setup_len u16 BE][setup][registration]
        let setup_bytes = server_setup.serialize();
        let reg_bytes = password_file_inner.serialize();
        let setup_len: u16 = setup_bytes
            .len()
            .try_into()
            .map_err(|_| PakeError::Protocol("server setup > 64 KiB".into()))?;

        let mut serialized = Vec::with_capacity(1 + 2 + setup_bytes.len() + reg_bytes.len());
        serialized.push(PASSWORD_FILE_VERSION);
        serialized.extend_from_slice(&setup_len.to_be_bytes());
        serialized.extend_from_slice(&setup_bytes);
        serialized.extend_from_slice(&reg_bytes);

        Ok(Self { serialized })
    }

    /// Parse the persisted blob back into the two opaque-ke components.
    fn decode(
        &self,
    ) -> Result<
        (
            ServerSetup<CopypasteCipherSuite>,
            ServerRegistration<CopypasteCipherSuite>,
        ),
        PakeError,
    > {
        if self.serialized.is_empty() {
            return Err(PakeError::WireFormat("empty password file".into()));
        }
        if self.serialized[0] != PASSWORD_FILE_VERSION {
            return Err(PakeError::WireFormat(format!(
                "unsupported version: 0x{:02x}",
                self.serialized[0]
            )));
        }
        if self.serialized.len() < 3 {
            return Err(PakeError::WireFormat("truncated header".into()));
        }
        let setup_len = u16::from_be_bytes([self.serialized[1], self.serialized[2]]) as usize;
        let body = &self.serialized[3..];
        if body.len() < setup_len {
            return Err(PakeError::WireFormat("truncated ServerSetup".into()));
        }
        let (setup_bytes, reg_bytes) = body.split_at(setup_len);

        let server_setup = ServerSetup::<CopypasteCipherSuite>::deserialize(setup_bytes)
            .map_err(|e| PakeError::WireFormat(format!("ServerSetup deserialize: {e}")))?;
        let server_reg = ServerRegistration::<CopypasteCipherSuite>::deserialize(reg_bytes)
            .map_err(|e| PakeError::WireFormat(format!("ServerRegistration deserialize: {e}")))?;
        Ok((server_setup, server_reg))
    }
}

/// Initiator (client) side of the PAKE handshake.
///
/// Holds the in-flight `opaque_ke::ClientLogin` state between `new` and
/// `finish`. Drop the value to abort the handshake (state is zeroized on
/// drop by opaque-ke).
pub struct PakeInitiator {
    state: ClientLogin<CopypasteCipherSuite>,
    /// Password is needed again at `finish` time (OPAQUE PRF re-evaluation).
    /// Zeroed on every drop path via the [`Drop`] impl below (security
    /// MED #9) — covers the panic / early-return cases where `finish`
    /// is never reached. The `state` field zeroizes itself on drop via
    /// opaque-ke's `derive-where(zeroize-on-drop)` attribute.
    password: Vec<u8>,
}

impl Drop for PakeInitiator {
    fn drop(&mut self) {
        // Zeroize the password buffer regardless of whether the handshake
        // completed (success), failed (deserialize / protocol error), or
        // was aborted by an unwinding panic on a higher stack frame. The
        // `Vec`'s heap allocation is the only sensitive copy held here —
        // `state` self-zeroizes via opaque-ke.
        use zeroize::Zeroize;
        self.password.zeroize();
    }
}

impl PakeInitiator {
    /// Step 1: client begins the handshake with the shared pairing
    /// password. Returns `(Self, message_to_send)` — send the bytes to the
    /// responder over the framed transport, then call [`Self::finish`] with
    /// the response.
    pub fn new(password: &str) -> Result<(Self, Vec<u8>), PakeError> {
        let mut rng = OsRng;
        let start = ClientLogin::<CopypasteCipherSuite>::start(&mut rng, password.as_bytes())
            .map_err(|e| PakeError::Protocol(format!("client login start: {e}")))?;
        let msg = start.message.serialize().to_vec();
        Ok((
            Self {
                state: start.state,
                password: password.as_bytes().to_vec(),
            },
            msg,
        ))
    }

    /// Step 3: client receives the server's response and derives the
    /// session key. Returns `(session_key, message_to_send_to_server)` — the
    /// server needs the final message to confirm and reach the same key.
    /// Consumes `self` because the handshake state is single-use.
    pub fn finish(mut self, server_message: &[u8]) -> Result<(SessionKey, Vec<u8>), PakeError> {
        use zeroize::Zeroize;

        let resp = CredentialResponse::deserialize(server_message)
            .map_err(|e| PakeError::WireFormat(format!("CredentialResponse: {e}")))?;

        // Take the password out so we can both feed it to `state.finish`
        // and zeroize it eagerly. The drained `self.password` (an empty
        // Vec) is later visited by `Drop`, which is a no-op on empty.
        let password = std::mem::take(&mut self.password);
        let result =
            self.state
                .clone()
                .finish(&password, resp, ClientLoginFinishParameters::default());
        let mut pw = password;
        pw.zeroize();

        let finish = result.map_err(|e| match e {
            opaque_ke::errors::ProtocolError::InvalidLoginError => PakeError::InvalidPassword,
            other => PakeError::Protocol(format!("client login finish: {other}")),
        })?;

        let key = session_key_to_array(finish.session_key.as_slice())?;
        let outbound = finish.message.serialize().to_vec();
        Ok((SessionKey(key), outbound))
    }
}

/// Responder (server) side of the PAKE handshake.
///
/// Holds the in-flight `opaque_ke::ServerLogin` state between `respond` and
/// `finish`. Drop the value to abort the handshake (state is zeroized).
pub struct PakeResponder {
    state: ServerLogin<CopypasteCipherSuite>,
}

impl PakeResponder {
    /// Step 2: server receives the client's opening message and responds.
    /// Requires the persisted [`PasswordFile`] for the peer being paired.
    /// Returns `(Self, message_to_send)`.
    pub fn respond(
        password_file: &PasswordFile,
        client_message: &[u8],
    ) -> Result<(Self, Vec<u8>), PakeError> {
        let (server_setup, server_reg) = password_file.decode()?;
        let mut rng = OsRng;
        let cred_req = CredentialRequest::deserialize(client_message)
            .map_err(|e| PakeError::WireFormat(format!("CredentialRequest: {e}")))?;
        let start = ServerLogin::start(
            &mut rng,
            &server_setup,
            Some(server_reg),
            cred_req,
            PAIRING_USERNAME,
            ServerLoginStartParameters::default(),
        )
        .map_err(|e| PakeError::Protocol(format!("server login start: {e}")))?;
        let msg = start.message.serialize().to_vec();
        Ok((Self { state: start.state }, msg))
    }

    /// Step 4 (server side): after receiving the client's final
    /// authenticator, finalise and derive the session key.
    pub fn finish(self, client_final: &[u8]) -> Result<SessionKey, PakeError> {
        let fin = CredentialFinalization::deserialize(client_final)
            .map_err(|e| PakeError::WireFormat(format!("CredentialFinalization: {e}")))?;
        let result = self.state.finish(fin).map_err(|e| match e {
            opaque_ke::errors::ProtocolError::InvalidLoginError => PakeError::InvalidPassword,
            other => PakeError::Protocol(format!("server login finish: {other}")),
        })?;
        let key = session_key_to_array(result.session_key.as_slice())?;
        Ok(SessionKey(key))
    }
}

/// Convert opaque-ke's variable-width `session_key` `GenericArray` view into
/// our fixed 32-byte key. For the Ristretto255-TripleDH ciphersuite the
/// output is exactly 64 bytes (two SHA-512 chaining values); we HKDF-extract
/// it down to 32 bytes for storage uniformity.
fn session_key_to_array(raw: &[u8]) -> Result<[u8; 32], PakeError> {
    if raw.is_empty() {
        return Err(PakeError::Protocol("empty session key".into()));
    }
    let hk = Hkdf::<Sha256>::new(Some(b"copypaste-pake-session-v1"), raw);
    let mut out = [0u8; 32];
    hk.expand(b"session-key", &mut out)
        .map_err(|e| PakeError::Protocol(format!("hkdf expand: {e}")))?;
    // Touch the `generic-array` import so it's kept for future direct
    // `GenericArray<u8, N>` interop without re-adding the dep.
    let _ = core::marker::PhantomData::<GenericArray<u8, generic_array::typenum::U32>>;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// End-to-end OPAQUE handshake — both sides must converge on the same
    /// 32-byte SessionKey when the password matches.
    #[test]
    fn pake_full_handshake_succeeds_with_correct_password() {
        let password = "correct horse battery staple";
        let pf = PasswordFile::register(password).expect("register");

        let (client, msg1) = PakeInitiator::new(password).expect("client new");
        let (server, msg2) = PakeResponder::respond(&pf, &msg1).expect("server respond");
        let (client_key, msg3) = client.finish(&msg2).expect("client finish");
        let server_key = server.finish(&msg3).expect("server finish");

        assert_eq!(
            client_key.as_bytes(),
            server_key.as_bytes(),
            "both sides must derive the same session key"
        );
    }

    /// Wrong-password attempt must fail — and must fail on the client's side
    /// (the OPAQUE security guarantee).
    #[test]
    fn pake_fails_with_wrong_password() {
        let pf = PasswordFile::register("the-right-one").expect("register");

        let (client, msg1) = PakeInitiator::new("the-wrong-one").expect("client new");
        let (server, msg2) = PakeResponder::respond(&pf, &msg1).expect("server respond proceeds");
        // Client `finish` is the first place OPAQUE detects mismatch — it
        // surfaces `InvalidPassword` because the OPRF output doesn't decrypt
        // the envelope.
        let client_res = client.finish(&msg2);
        assert!(
            matches!(client_res, Err(PakeError::InvalidPassword)),
            "expected InvalidPassword, got {:?}",
            client_res.as_ref().err()
        );

        // And even if a malicious client forged a finalization message, the
        // server must also reject. Feed garbage of the right shape.
        let garbage = vec![0u8; 128];
        let server_res = server.finish(&garbage);
        assert!(
            server_res.is_err(),
            "server must reject forged finalization"
        );
    }

    /// HKDF subkey derivation must be deterministic and salt-separated.
    #[test]
    fn session_key_derives_distinct_chacha_keys_for_different_salts() {
        let sk = SessionKey([0x42; 32]);
        let k1 = sk.derive_xchacha_key(b"chunk-0001");
        let k2 = sk.derive_xchacha_key(b"chunk-0002");
        let k1_again = sk.derive_xchacha_key(b"chunk-0001");

        assert_eq!(k1, k1_again, "derivation must be deterministic");
        assert_ne!(k1, k2, "different salts must yield different keys");
        assert_eq!(k1.len(), 32);
    }

    /// S3: `bind_to_tls_channel` mixes the TLS binder into the session key so
    /// that keys derived on different channels are always distinct, even when
    /// the PAKE session key is identical.
    #[test]
    fn channel_binding_produces_distinct_keys_for_different_binders() {
        let sk = SessionKey([0xAB; 32]);
        let binder_a = [0x01u8; 32];
        let binder_b = [0x02u8; 32];

        let bound_a = sk.bind_to_tls_channel(&binder_a);
        let bound_b = sk.bind_to_tls_channel(&binder_b);
        let bound_a_again = sk.bind_to_tls_channel(&binder_a);

        assert_eq!(bound_a, bound_a_again, "binding must be deterministic");
        assert_ne!(
            bound_a, bound_b,
            "different TLS binders must yield different channel-bound keys"
        );
        // The channel-bound key must also differ from the raw session key.
        assert_ne!(
            *bound_a, sk.0,
            "channel-bound key must differ from the raw PAKE key"
        );
    }

    /// S3: The channel-bound key is the same on both ends of a real PAKE
    /// handshake, provided both sides supply the same TLS binder.
    #[test]
    fn channel_binding_is_symmetric_after_pake() {
        let password = "channel-binding-test-password";
        let pf = PasswordFile::register(password).expect("register");

        let (client, msg1) = PakeInitiator::new(password).expect("client new");
        let (server, msg2) = PakeResponder::respond(&pf, &msg1).expect("server respond");
        let (client_key, msg3) = client.finish(&msg2).expect("client finish");
        let server_key = server.finish(&msg3).expect("server finish");

        // Simulate the same TLS binder both sides would extract from their
        // shared TLS session (in production these come from export_keying_material).
        let shared_binder = [0xFEu8; 32];

        let client_bound = client_key.bind_to_tls_channel(&shared_binder);
        let server_bound = server_key.bind_to_tls_channel(&shared_binder);

        assert_eq!(
            client_bound, server_bound,
            "both sides must derive the same channel-bound key"
        );
    }

    /// PasswordFile must roundtrip through its serialized form and still
    /// drive a successful handshake.
    #[test]
    fn password_file_serialize_roundtrip() {
        let password = "roundtrip-pw-2026";
        let pf = PasswordFile::register(password).expect("register");

        let blob = pf.serialized.clone();
        assert_eq!(blob[0], PASSWORD_FILE_VERSION, "version tag preserved");
        drop(pf);

        let pf2 = PasswordFile { serialized: blob };
        let (client, msg1) = PakeInitiator::new(password).expect("client new");
        let (server, msg2) =
            PakeResponder::respond(&pf2, &msg1).expect("server respond from reloaded pf");
        let (client_key, msg3) = client.finish(&msg2).expect("client finish");
        let server_key = server.finish(&msg3).expect("server finish");
        assert_eq!(client_key.as_bytes(), server_key.as_bytes());
    }

    /// SAS derivation must be deterministic and yield a 6-digit decimal string.
    #[test]
    fn derive_sas_is_deterministic_and_six_digits() {
        let key = [0x37u8; 32];
        let sas1 = derive_sas(&key);
        let sas2 = derive_sas(&key);
        assert_eq!(sas1, sas2, "same key must yield the same SAS");
        assert_eq!(sas1.len(), SAS_DIGITS, "SAS must be SAS_DIGITS chars");
        assert!(
            sas1.bytes().all(|b| b.is_ascii_digit()),
            "SAS must be all decimal digits, got {sas1}"
        );
    }

    /// Different bound keys (e.g. distinct relay legs) must yield different SAS
    /// strings — that divergence is exactly what the human compare detects.
    #[test]
    fn derive_sas_differs_for_different_keys() {
        let sas_a = derive_sas(&[0x01u8; 32]);
        let sas_b = derive_sas(&[0x02u8; 32]);
        assert_ne!(
            sas_a, sas_b,
            "different bound keys must yield different SAS values"
        );
    }

    /// Both ends of a real PAKE + channel-binding handshake derive the SAME SAS
    /// when they share the same TLS binder. Mirrors
    /// `channel_binding_is_symmetric_after_pake`.
    #[test]
    fn derive_sas_is_symmetric_after_pake() {
        let password = "sas-symmetry-test-password";
        let pf = PasswordFile::register(password).expect("register");

        let (client, msg1) = PakeInitiator::new(password).expect("client new");
        let (server, msg2) = PakeResponder::respond(&pf, &msg1).expect("server respond");
        let (client_key, msg3) = client.finish(&msg2).expect("client finish");
        let server_key = server.finish(&msg3).expect("server finish");

        let shared_binder = [0xFEu8; 32];
        let client_bound = client_key.bind_to_tls_channel(&shared_binder);
        let server_bound = server_key.bind_to_tls_channel(&shared_binder);

        assert_eq!(
            derive_sas(&client_bound),
            derive_sas(&server_bound),
            "both sides must derive the same SAS from the same bound key"
        );
    }

    /// Domain separation: the SAS info string differs from the confirmation-tag
    /// info strings, so the SAS is not a truncation of either tag.
    #[test]
    fn derive_sas_is_domain_separated_from_confirm_tags() {
        let key = [0x5Au8; 32];
        let sas = derive_sas(&key);
        let init_tag = channel_confirmation_tag(&key, ConfirmRole::Initiator);
        let resp_tag = channel_confirmation_tag(&key, ConfirmRole::Responder);
        // Reproduce the SAS digit math from the tags' first 4 bytes to prove the
        // SAS is NOT derived from the same HKDF stream as either tag.
        let from_init = format!(
            "{:06}",
            u32::from_be_bytes([init_tag[0], init_tag[1], init_tag[2], init_tag[3]]) % 1_000_000
        );
        let from_resp = format!(
            "{:06}",
            u32::from_be_bytes([resp_tag[0], resp_tag[1], resp_tag[2], resp_tag[3]]) % 1_000_000
        );
        assert_ne!(sas, from_init);
        assert_ne!(sas, from_resp);
    }

    /// Sanity: PakeError display strings.
    #[test]
    fn pake_error_displays() {
        let err = PakeError::InvalidPassword;
        assert_eq!(err.to_string(), "invalid password");
        let err = PakeError::Protocol("oprf failed".into());
        assert!(err.to_string().contains("oprf failed"));
    }
}
