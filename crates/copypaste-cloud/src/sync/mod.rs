//! The sync driver: local history in, cloud rows out, and back again.
//!
//! # What this module decides, and what it does not
//!
//! It decides *when* to talk to the backend, *what* is allowed to leave the
//! device, and how to recover from a 401 or a 429. It deliberately does **not**
//! decide which of two versions of an item wins.
//!
//! That last point is load-bearing. Manifest 05 INV-C2 records the worst
//! convergence bug in v1: P2P used the full ordering while the cloud and relay
//! paths each used a cut-down comparison of their own, so two devices holding
//! different content at an equal timestamp each kept their own copy and never
//! converged. The fix was one comparator for every transport. Here that is
//! structural rather than disciplinary — the comparator lives behind
//! [`CloudSource::apply_remote`], in the daemon's store, alongside the one the
//! P2P transport already calls. This module hands a remote version down and is
//! told whether it won. There is no second comparator to drift.
//!
//! # The ordering key is `created_at`, and it is a wall clock
//!
//! The manifest's order begins with `lamport_ts`. **v2 has no Lamport clock and
//! is not getting one.** The order is `created_at` (Unix ms), then
//! `content_hash`, then `origin_device_id` — three keys, deterministic and
//! symmetric — which is exactly what `copypaste-p2p`'s `merge_decision` already
//! implements. One order for both transports is the whole point of INV-C2, so
//! the cloud path uses that one rather than introducing a second.
//!
//! What a Lamport stamp bought was monotonicity: it could not go backwards, so
//! a device with a wrong clock could not outrank a newer write. A wall clock
//! can. That is made safe here by refusing versions stamped implausibly far
//! ahead — see [`MAX_FUTURE_SKEW_MS`], which mirrors the constant and the
//! behaviour in `copypaste-p2p`. Without that guard one row stamped `i64::MAX`
//! wins every future comparison for its `item_id` and censors that item on every
//! device; with it, the damage a bad clock can do is bounded to winning ties it
//! should have lost, which is the trade manifest 05 R-CLK-2 already accepts. No
//! skew *correction* is attempted, only refusal, and refusal skips one version
//! rather than failing the round or deleting anything.
//!
//! For the same reason there is no tombstone special case in this module beyond
//! *carrying* the flag. A delete is an ordinary version with `deleted = true`
//! and no content (manifest 05 §3.5, T-1/T-2), so delete-wins is whatever the
//! store's comparator says — and a delete for an item this device has never
//! seen must still be persisted as a tombstone, or a later-arriving create has
//! nothing to lose against and resurrects it (T-3, `CopyPaste-bfiu`). That is
//! stated on [`CloudSource::apply_remote`] as a requirement on the implementor.
//!
//! # Idempotency
//!
//! Replaying a push or a pull changes nothing. Push upserts on `item_id`, so a
//! re-sent row overwrites itself. Pull hands every row to `apply_remote`, which
//! keeps the local copy when the two versions tie on every ordering key — so a
//! re-delivered version, including this device's own writes echoed back through
//! the account, is absorbed as a no-op rather than filtered by a "did I send
//! this?" check (manifest 05 INV-I1/INV-I2).
//!
//! # The watermark
//!
//! One cursor, in milliseconds, meaning *everything this device has reconciled
//! with the cloud*. [`CloudSync::pull`] advances it; [`CloudSync::push`] reads
//! it to decide what is new locally. Both bounds are **inclusive**: a row
//! exactly on the boundary is re-offered rather than skipped, because
//! re-offering is free (upsert and LWW both absorb it) and skipping is silent
//! data loss. Manifest 05 §4.4 spends its longest note on precisely this — a
//! strict `>` on a millisecond cursor permanently lost every row sharing the
//! boundary millisecond once a burst filled a page.
//!
//! The watermark is stored by the source independently of the item rows, so
//! that evicting old rows to honour a local storage cap cannot move it
//! backwards (INV-N5), and it only ever moves forward (INV-I4: it advances past
//! every row that was *readable*, including rows that were skipped as an
//! ordering loser or as undecryptable, so an unreadable row is not re-fetched
//! forever).
//!
//! # Errors never carry a secret
//!
//! Every [`SyncError`] payload is a `&'static str`. There is no `String` field
//! anywhere in this module's error type, so there is nothing to interpolate a
//! filesystem path, an access token or a passphrase into — `CLAUDE.md` rule 4,
//! made structural rather than remembered.
//!
//! # How the module is laid out
//!
//! One responsibility per file, and each file carries the reasoning for the
//! rule it enforces:
//!
//! | file | owns |
//! |---|---|
//! | [`source`] | the local seam: [`LocalItem`], [`CloudSource`], [`SensitiveGuard`] |
//! | [`transport`] | the network seam: [`RestApi`], [`AuthApi`] and the faults they report |
//! | [`outcome`] | what one round reports: [`SyncStats`] and [`SyncError`] |
//! | [`driver`] | the [`CloudSync`] value itself, and `push`-then-`pull` |
//! | [`push`] | the upload path and the sensitive gate |
//! | [`pull`] | the download path, the cursor, paging and the skew refusal |
//! | [`retry`] | the 401 and 429 recovery rules — the only retry layer here |
//! | [`cadence`] | the idle poll interval and how [`crate::realtime`] resets it |
//! | [`adapters`] | `SupabaseRest`/`SupabaseAuth` mapped onto the two seams |

pub mod adapters;
pub mod cadence;
pub mod driver;
pub mod outcome;
pub mod pull;
pub mod push;
pub mod retry;
pub mod source;
pub mod transport;

#[cfg(test)]
mod fakes;

pub use cadence::{MAX_POLL_INTERVAL, MIN_POLL_INTERVAL};
pub use driver::CloudSync;
pub use outcome::{SyncError, SyncStats};
pub use pull::MAX_FUTURE_SKEW_MS;
pub use source::{CloudSource, LocalItem, SensitiveGuard};
pub use transport::{AuthApi, AuthFault, RestApi, TransportFault};

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
//
// Every test in this module runs against a fake store and a fake transport
// (see `fakes`). Nothing here opens a socket.
//
// The tests that belong to one path live beside it; what is left here is the
// round trip that needs both paths at once, and the assertions on constants
// that span several files.

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use super::fakes::{
        allow_everything, config, item, key, session, FakeAuth, FakeRest, FakeSource, ACCOUNT, PASS,
    };
    use super::retry::{MAX_RETRY_AFTER, RETRY_AFTER_FALLBACK};
    use super::*;
    use crate::rest::CloudItem;

    #[tokio::test]
    async fn two_devices_converge_through_the_backend() {
        // Device A pushes; device B, holding the same passphrase and account,
        // pulls and reads the plaintext back. This is the round trip that the
        // whole crate exists for.
        let backend = Arc::new(FakeRest::default());

        let a_source = FakeSource::with_outgoing(vec![item("a", 1_000, "shared clipboard text")]);
        let device_a = CloudSync::new(
            SharedRest(Arc::clone(&backend)),
            FakeAuth::default(),
            key(),
            config(),
            session("token-1"),
            allow_everything(),
        );
        device_a.push(&a_source).await.unwrap();

        let b_source = FakeSource::default();
        let device_b = CloudSync::new(
            SharedRest(Arc::clone(&backend)),
            FakeAuth::default(),
            // Re-derived independently: no shared state but the passphrase.
            crate::crypto::derive_sync_key(PASS, ACCOUNT).unwrap(),
            config(),
            session("token-2"),
            allow_everything(),
        );
        let stats = device_b.pull(&b_source).await.unwrap();

        assert_eq!(stats.applied, 1);
        assert_eq!(b_source.get("a").unwrap().content, b"shared clipboard text");

        // Replaying the whole exchange changes nothing on either side.
        device_a.push(&a_source).await.unwrap();
        let replay = device_b.pull(&b_source).await.unwrap();
        assert_eq!(replay.applied, 0);
    }

    /// One backend shared by two drivers.
    struct SharedRest(Arc<FakeRest>);

    impl RestApi for SharedRest {
        async fn fetch_since(
            &self,
            token: &str,
            since_ms: i64,
            limit: u32,
        ) -> Result<Vec<CloudItem>, TransportFault> {
            self.0.fetch_since(token, since_ms, limit).await
        }
        async fn upsert(&self, token: &str, items: &[CloudItem]) -> Result<(), TransportFault> {
            self.0.upsert(token, items).await
        }
        async fn tombstone(&self, token: &str, ids: &[String]) -> Result<(), TransportFault> {
            self.0.tombstone(token, ids).await
        }
    }

    #[test]
    fn the_tunables_are_the_documented_ones() {
        assert_eq!(MIN_POLL_INTERVAL, Duration::from_secs(5));
        assert_eq!(MAX_POLL_INTERVAL, Duration::from_secs(300));
        assert_eq!(RETRY_AFTER_FALLBACK, Duration::from_secs(1));
        assert_eq!(MAX_RETRY_AFTER, Duration::from_secs(30));
        // Mirrors `copypaste_p2p::sync::MAX_FUTURE_SKEW_MS`; the two transports
        // share an ordering, so they must share what counts as a valid stamp.
        assert_eq!(MAX_FUTURE_SKEW_MS, 24 * 60 * 60 * 1000);
    }
}
