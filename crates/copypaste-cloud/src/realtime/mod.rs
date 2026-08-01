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
//! # Using this from a process that stays up for days
//!
//! Four obligations, each of which is a way this goes quiet rather than a way
//! it fails loudly:
//!
//! 1. **Hand every refreshed JWT to
//!    [`RealtimeSubscription::set_access_token`].** Supabase closes a channel
//!    whose token has expired; a subscription that never re-authenticates dies
//!    about an hour in, and the only symptom is that events stop.
//! 2. **Run a round on [`RealtimeEvent::Resubscribed`].** It is the one moment
//!    at which the at-most-once property is known to have bitten.
//! 3. **Report the channel's state to
//!    [`CloudSync::note_push_channel`](crate::sync::CloudSync::note_push_channel)** —
//!    on a confirmed join, and again when it drops. The long idle poll ceiling
//!    is only defensible while this module is carrying the latency.
//! 4. **Treat an `Err` as information, not as an end.** The task keeps
//!    reconnecting on its own schedule; `None` from
//!    [`RealtimeSubscription::next_event`] is the only terminal signal. A
//!    repeated [`RealtimeError::JoinRefused`] usually means the token, so
//!    refresh the session and push it down rather than rebuilding the
//!    subscription.
//!
//! # Why this speaks Phoenix by hand
//!
//! `CLAUDE.md` rule 1 says reach for a crate first. There is no maintained Rust
//! client for Supabase Realtime, and the Phoenix subset actually used here is
//! five message shapes: `phx_join`, `phx_reply`, `heartbeat`, `access_token`,
//! `phx_leave`, plus the `postgres_changes` payload. The v1 audit reached the same conclusion and
//! it still holds under exemption 1 — but it holds only for *this* subset. This
//! module is not a Phoenix client library: there is no channel registry, no
//! push/ref correlation table, no presence, no generic event router. Adding one
//! would be the wheel this rule exists to prevent.
//!
//! What is *not* hand-rolled: the websocket itself (`tokio-tungstenite`), the
//! JSON (`serde_json`), and the reconnect schedule (`backon`).
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
//! failure logs only the frame length.
//!
//! [`channel`] and [`frame`] are separate files on purpose: parsing an
//! attacker-influenced frame and deciding what to connect *as* are different
//! jobs with different failure modes, and the per-user filter in the join
//! belongs with the token it is derived from rather than with the parser.

use std::time::Duration;

pub mod channel;
pub mod event;
pub mod frame;
pub mod socket;
pub mod subscription;

pub use event::{RealtimeError, RealtimeEvent};
pub use subscription::RealtimeSubscription;

/// The table we subscribe to. One table, one channel — see manifest 05 §4.1.
///
/// [`crate::rest::TABLE`]'s value, not a second spelling of it: a subscription
/// naming a table the REST client does not write would receive nothing, and
/// nothing about that failure is visible — the poll simply carries everything.
pub(crate) const TABLE: &str = crate::rest::TABLE;

/// Phoenix topic. Supabase namespaces every channel under `realtime:`.
pub(crate) const TOPIC: &str = "realtime:clipboard_items";

/// How long to wait for the socket and for the `phx_reply` that confirms the
/// join. Without a bound, a black-holed connection stalls `connect` forever.
///
/// Also bounds the farewell in [`socket`]: a shutdown that cannot get its
/// `phx_leave` out must not hold the caller either.
pub(crate) const JOIN_TIMEOUT: Duration = Duration::from_secs(10);

#[cfg(test)]
mod tests {
    use super::*;

    /// [`TOPIC`] cannot be built from [`TABLE`] in a `const`, so it is the one
    /// place the name is still written twice.
    #[test]
    fn the_topic_names_the_table_we_subscribe_to() {
        assert_eq!(TOPIC, format!("realtime:{TABLE}"));
    }
}
