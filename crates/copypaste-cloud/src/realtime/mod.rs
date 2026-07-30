//! Supabase Realtime: a second device learns about a new row without polling.
//!
//! # Realtime is an accelerator, never the source of truth
//!
//! Read this before using anything in this module. Supabase Realtime, like the
//! v1 relay's SSE channel before it, is **at-most-once**: events that occur
//! while the socket is down are not replayed when it comes back. Manifest 05
//! §5.1 row 9a calls this "the single most important item in the table", and
//! §4.8 states the rule directly — the cursor poll in [`crate::sync`] is the
//! correctness mechanism, and this module only shortens the latency between a
//! write on one device and its appearance on another.
//!
//! Deleting the poll loop "because we have Realtime now" reintroduces silent
//! data loss on every reconnect. What Realtime is allowed to change is the poll
//! *interval*, and only once the channel has confirmed its join — not merely
//! once the socket has opened. A socket that is open but whose channel never
//! joined delivers nothing at all, and backing off on it would halve the sync
//! rate for no reason.
//!
//! # Why this speaks Phoenix by hand
//!
//! `CLAUDE.md` rule 1 says reach for a crate first. There is no maintained Rust
//! client for Supabase Realtime, and the Phoenix subset actually used here is
//! four message shapes: `phx_join`, `phx_reply`, `heartbeat`, `phx_leave`, plus
//! the `postgres_changes` payload. The v1 audit reached the same conclusion and
//! it still holds under exemption 1 — but it holds only for *this* subset. This
//! module is not a Phoenix client library: there is no channel registry, no
//! push/ref correlation table, no presence, no generic event router. Adding one
//! would be the wheel this rule exists to prevent.
//!
//! What is *not* hand-rolled: the websocket itself (`tokio-tungstenite`), the
//! JSON (`serde_json`), and the reconnect schedule (`backoff`).
//!
//! # The wire format
//!
//! Every frame is a five-element JSON array:
//!
//! ```text
//! [join_ref, ref, topic, event, payload]
//! ```
//!
//! Frames that are not exactly five elements are rejected. A `ref` that is not
//! a JSON string maps to *absent* rather than to an empty string: v1's
//! `CopyPaste-crh3.97` came from mapping a numeric ref to `Some("")`, so a
//! reply's ref never matched the heartbeat's and every heartbeat reply was
//! silently dropped.
//!
//! # Log hygiene
//!
//! A raw frame embeds clipboard ciphertext and metadata. Nothing in this module
//! logs a frame body, a payload, an access token or the socket URL. A parse
//! failure logs the frame length and a sixteen-byte hex prefix, which is enough
//! to tell "HTML error page" from "truncated JSON" and not enough to be worth
//! exfiltrating.
//!
//! # How the module is laid out
//!
//! | file | owns |
//! |---|---|
//! | [`event`] | what a subscriber sees: [`RealtimeEvent`] and [`RealtimeError`] |
//! | [`subscription`] | the handle, its background task, and shutdown |
//! | [`socket`] | one session on one socket: heartbeats, dispatch, and the reconnect schedule |
//! | [`channel`] | opening a socket and joining the channel — the URL, the JWT subject, the join frame |
//! | [`frame`] | the Phoenix envelope and the `postgres_changes` payload |
//!
//! `channel` and `frame` are apart on purpose: parsing an attacker-influenced
//! frame and deciding what to connect *as* are different jobs with different
//! failure modes, and the per-user filter in the join belongs with the token it
//! is derived from rather than with the parser.

use std::time::Duration;

pub mod channel;
pub mod event;
pub mod frame;
pub mod socket;
pub mod subscription;

pub use event::{RealtimeError, RealtimeEvent};
pub use subscription::RealtimeSubscription;

/// The table we subscribe to. One table, one channel — see manifest 05 §4.1.
pub(crate) const TABLE: &str = "clipboard_items";

/// Phoenix topic. Supabase namespaces every channel under `realtime:`.
pub(crate) const TOPIC: &str = "realtime:clipboard_items";

/// How long to wait for the socket and for the `phx_reply` that confirms the
/// join. Without a bound, a black-holed connection stalls `connect` forever.
///
/// Also bounds the farewell in [`socket`]: a shutdown that cannot get its
/// `phx_leave` out must not hold the caller either.
pub(crate) const JOIN_TIMEOUT: Duration = Duration::from_secs(10);
