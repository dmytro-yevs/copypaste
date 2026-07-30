//! The history, as a sync session sees it — one implementation for both
//! transports and for both platforms.
//!
//! [`StoreSource`] is a [`copypaste_p2p::sync::SyncSource`] over a plain
//! [`Store`](crate::Store). It lives here rather than in the daemon because
//! Android has no daemon: the app links this crate in its own process and
//! constructs the same source over the same store, and a second implementation
//! is how the two would come apart.
//!
//! It is also what the cloud transport merges through. The comparator is
//! [`copypaste_p2p::sync::merge_decision`] and it is called from exactly one
//! place ([`merge::apply_remote_version`]), because v1's worst convergence bug
//! was two transports comparing with two different comparators (manifest 05
//! INV-C2).

mod merge;
mod source;

#[cfg(test)]
mod testkit;

pub use merge::{apply_remote_version, open_version, MergeError, RemoteVersion};
pub use source::StoreSource;

/// Fixed, pathless sentences: these reach a peer's log and a user's screen
/// (CLAUDE.md rule 4).
pub const MSG_STORE: &str = "the history database could not be read";
pub const MSG_ENCRYPT: &str = "the incoming item could not be encrypted";

/// Run a short blocking database call without stalling the reactor.
///
/// Every operation behind a [`StoreSource`] is SQLite plus an AEAD, and every
/// caller is an async task, so the work happens on whatever thread is driving
/// that task. On a multi-threaded runtime `block_in_place` hands the other
/// tasks to a different worker for the duration; on a current-thread runtime —
/// which is what `#[tokio::test]` builds — it would panic, so there the call
/// simply runs inline.
pub fn blocking<T>(f: impl FnOnce() -> T) -> T {
    use tokio::runtime::{Handle, RuntimeFlavor};
    match Handle::try_current() {
        Ok(handle) if matches!(handle.runtime_flavor(), RuntimeFlavor::MultiThread) => {
            tokio::task::block_in_place(f)
        }
        _ => f(),
    }
}
