//! The secure channel between two paired devices.
//!
//! # What this module is
//!
//! One thing: a mutually-authenticated, forward-secret, message-oriented
//! channel over TCP, built from a 256-bit pairing token.
//!
//! ```text
//! PairingToken (32 B, OS CSPRNG)                              -- token.rs
//!   ├─ to_code()      -> "0G7M-XQ4V-…"   human-transferable, Crockford base32
//!   ├─ psk()          -> Noise pre-shared key
//!   └─ pairing_id()   -> BLAKE2s(domain || token)[..16], hex — safe to log
//!
//! Session::connect / Session::accept / Session::accept_any    -- handshake.rs
//!   └─ Noise_NNpsk0_25519_ChaChaPoly_BLAKE2s
//!        └─ LengthDelimitedCodec frames -> send::<T> / recv::<T>  -- session.rs
//! ```
//!
//! Four files, along the lines the pieces actually move on:
//!
//! * [`token`] — the pairing token and its printed code. Pure; no sockets.
//! * [`handshake`] — `NNpsk0`, the three entry points, and the timeout that
//!   stops a silent peer pinning a task. This is where the "why Noise, why no
//!   PAKE" reasoning lives.
//! * [`session`] — the established channel: record framing, chunking across
//!   Noise's 65535-byte ceiling, and poisoning after an authentication failure.
//! * [`error`] — the closed error set, whose shape is itself a security
//!   property.
//!
//! The split is by responsibility, not by layer: `handshake` writes to the
//! socket and `session` reads from it, but a change to the pairing-code
//! alphabet touches only `token`, a change to the Noise pattern touches only
//! `handshake`, and a change to chunking touches only `session`.
//!
//! # Why no TLS and no PAKE
//!
//! Summarised here, argued in [`handshake`]: `NN` means no static keys and so
//! no certificate lifecycle to implement; `psk0` makes possession of the token
//! the identity and makes the handshake fail closed by construction; a 256-bit
//! CSPRNG token has no dictionary to protect it from, so a PAKE would be
//! answering a question we do not ask. v1 spent rustls, rcgen, a hand-written
//! pinning verifier, two hand-rolled DER parsers and OPAQUE on this.
//!
//! # Rules this module holds itself to
//!
//! * **No secret in `Debug`, `Display` or any log line.** [`PairingToken`]'s
//!   `Debug` prints its `pairing_id` and nothing else; [`Session`]'s prints the
//!   peer address. Tests pin both.
//! * **No filesystem path in any error** (`CLAUDE.md` rule 4). Structurally
//!   enforced: no variant of [`TransportError`] has a payload that can hold
//!   one, and this module never opens a file.
//! * **Constant-time comparison of secrets** via `subtle` (port manifest 02,
//!   I-13). `PairingToken`'s `PartialEq` routes through it, so there is no
//!   short-circuiting comparison of token bytes reachable from outside.
//! * **Zeroize on drop** for the token and for every buffer that holds
//!   plaintext or key material (I-12).
//! * **A handshake cannot pin a task.** Dial, both handshake messages and the
//!   transport-mode transition all happen inside one
//!   [`HANDSHAKE_TIMEOUT`].

mod error;
mod handshake;
mod session;
mod token;

#[cfg(test)]
mod testutil;

pub use error::TransportError;
pub use handshake::{HANDSHAKE_TIMEOUT, NOISE_PARAMS};
pub use session::{Session, MAX_MESSAGE_BYTES, MAX_NOISE_MESSAGE};
pub use token::{PairingToken, TOKEN_LEN};
