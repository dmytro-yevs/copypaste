//! What a subscriber sees: one change, or one failure.
//!
//! Both types are the module's public vocabulary, and both are closed sets —
//! which is what makes the log-hygiene rule in the module docs structural
//! rather than remembered.

use crate::rest::CloudItem;

/// One Postgres change on `clipboard_items`.
///
/// `Delete` carries only an `item_id` because a Postgres delete event carries
/// only the replica-identity columns. In practice it should be rare: manifest
/// 05 §3.5 makes a delete a *tombstone*, an ordinary row version with
/// `deleted = true`, which arrives here as [`RealtimeEvent::Update`]. A real
/// `Delete` means a row was removed out of band — a retention job, or a manual
/// operation — and the sync driver treats it as a hint to re-poll, never as an
/// instruction to remove a local row.
///
/// Only `Debug` is derived. `Clone` and `PartialEq` would impose those bounds
/// on `CloudItem`, which belongs to `rest.rs`; this enum should not constrain a
/// sibling's DTO for the convenience of a test.
#[derive(Debug)]
pub enum RealtimeEvent {
    Insert(CloudItem),
    Update(CloudItem),
    Delete {
        item_id: String,
    },
    /// The socket dropped and the channel has re-joined.
    ///
    /// Not a row: it is the one moment at which the at-most-once property
    /// above is *known* to have bitten. Nothing that happened while the socket
    /// was down is replayed, so a subscriber must run a poll round on this
    /// rather than wait out its idle interval (manifest 05 §5.1 row 9a). It is
    /// delivered on re-join, never on the first connect — that one is
    /// [`RealtimeSubscription::connect`](super::RealtimeSubscription::connect)
    /// returning.
    Resubscribed,
}

/// Failures this module can produce.
///
/// Payloads are `&'static str`. No variant can carry the socket URL, the
/// access token, the anon key or a frame body: `CLAUDE.md` rule 4 plus the
/// sync-path rule that a token is never rendered. A closed set of literals
/// leaves nothing to interpolate into.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RealtimeError {
    /// The websocket could not be opened, or the handshake was rejected.
    #[error("realtime connection failed: {0}")]
    Connect(&'static str),

    /// The channel opened but the server did not confirm the join.
    #[error("realtime channel join was refused")]
    JoinRefused,

    /// A frame did not match the Phoenix envelope or a change payload was not
    /// the shape we can use. The offending bytes are not carried here.
    #[error("realtime protocol error: {0}")]
    Protocol(&'static str),

    /// The access token did not carry a subject claim, so the per-user filter
    /// cannot be built.
    ///
    /// This is a **hard error, never a silently-omitted filter**
    /// (`CopyPaste-nr2y`, manifest 05 §4.7). Without `user_id=eq.<uuid>` the
    /// Realtime server can place another account's rows into the stream before
    /// server-side RLS applies. The filter does not replace RLS; it backs it up,
    /// and defence in depth that silently disables itself is not defence.
    #[error("the session token carries no user id, so the per-user filter cannot be applied")]
    MissingUserId,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_carry_no_url_no_token_and_no_frame() {
        let errors = [
            RealtimeError::Connect("the websocket could not be opened"),
            RealtimeError::JoinRefused,
            RealtimeError::Protocol("frame is not json"),
            RealtimeError::MissingUserId,
        ];
        for e in errors {
            let msg = e.to_string();
            assert!(!msg.contains("://"), "url in {msg:?}");
            assert!(!msg.contains("supabase"), "host in {msg:?}");
            assert!(!msg.contains('/'), "path-like separator in {msg:?}");
            assert!(!msg.contains("eyJ"), "jwt-like text in {msg:?}");
        }
    }
}
