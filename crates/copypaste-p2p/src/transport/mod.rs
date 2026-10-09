//! The secure channel between two paired devices: mutually authenticated,
//! forward secret and message oriented, over TCP, built from a 256-bit pairing
//! token. Short invitation codes authenticate a separate SPAKE2 bootstrap
//! before the full token is transferred inside a Noise channel.
//!
//! Rules this module holds itself to:
//!
//! * **No secret in `Debug`, `Display` or any log line.** [`PairingToken`]'s
//!   `Debug` prints its `pairing_id` and nothing else; [`Session`]'s prints the
//!   peer address. Tests pin both.
//! * **No filesystem path in any error** (`AGENTS.md` rule 4). Structurally
//!   enforced: no variant of [`TransportError`] has a payload that can hold
//!   one, and this module never opens a file.
//! * **Constant-time comparison of secrets** via `subtle` (port manifest 02,
//!   I-13). `PairingToken`'s `PartialEq` routes through it, so no
//!   short-circuiting comparison of token bytes is reachable from outside.
//! * **Zeroize on drop** for the token and for every buffer that holds
//!   plaintext or key material (I-12).
//! * **A handshake cannot pin a task.** Dial, both handshake messages and the
//!   transport-mode transition all happen inside one [`HANDSHAKE_TIMEOUT`].

mod error;
mod handshake;
mod pairing_bootstrap;
mod pairing_code;
mod session;
mod token;

#[cfg(test)]
mod testutil;

pub use error::TransportError;
pub use handshake::{HANDSHAKE_TIMEOUT, NOISE_PARAMS};
pub(crate) use pairing_bootstrap::PairingCandidate;
pub use pairing_code::PairingCode;
pub use session::{Session, MAX_MESSAGE_BYTES, MAX_NOISE_MESSAGE};
pub use token::{PairingToken, PskCandidate, TOKEN_LEN};
