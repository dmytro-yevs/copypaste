//! Auto-wipe: deleting a detected secret from history once its TTL has elapsed.
//!
//! This is the only destructive consumer of [`Severity`], and it is the only
//! place the confidence floor turns into permission to delete user data
//! (manifest I2, manifest 01 §3.17-§3.19, manifest 07 §6.2).
//!
//! # Two gates, and both must agree
//!
//! A row is deleted only if it was flagged **at capture** (`is_sensitive = 1`,
//! the inclusive predicate that also keeps it out of the search index) **and**
//! its plaintext scans as [`Severity::HighConfidence`] **now**. The second gate
//! reads the row rather than a stamped flag on purpose: `CLAUDE.md` rule 4 puts
//! data loss above everything, and a persisted "may be deleted" bit written by a
//! ruleset that has since changed is a decision nobody can review before it
//! fires. Reading the plaintext also means an item whose confidence has dropped
//! out of the wipe band survives, which is the safe direction.
//!
//! # Why no `expires_at` column
//!
//! v1 carried one and paid for it three times: `CopyPaste-3e7y` (a sensitive row
//! with no `expires_at` outlived its TTL because two predicates disagreed),
//! `CopyPaste-44rq.62` (backfill and delete in separate transactions) and
//! `CopyPaste-8ebg.2` (a dedup bump inherited a deadline seconds away). All
//! three are shapes of "the deadline is stored, and the store drifted from the
//! rule". Here the deadline is *derived* — `created_at + ttl` — so there is one
//! predicate, nothing to backfill, and a re-copy resets the deadline for free
//! because [`crate::Store::insert_or_bump`] restamps `created_at`. The
//! consequence, stated rather than hidden: changing the TTL re-dates every
//! existing sensitive item instead of only new ones. See
//! `docs/rewrite/port-manifest/07-secret-detection.md` §6.2, amended.

use std::time::Duration;

use crate::crypto::{decrypt, ItemKey};
use crate::sensitive::Detector;
use crate::storage::{Store, StoreError};

/// How long a detected secret stays in history. Manifest 07 §6.2: long enough
/// to paste a password once, short enough that it does not linger.
pub const DEFAULT_SENSITIVE_TTL: Duration = Duration::from_secs(30);

/// Auto-wipe off.
///
/// A real value, never clamped up to a minimum (manifest 01 §3.17, bug P2).
/// v1's own bug here was treating zero as a *duration*, which put the threshold
/// at `now` and deleted every sensitive item on every tick — the exact opposite
/// of the user's "turn it off".
pub const SENSITIVE_TTL_DISABLED: Duration = Duration::ZERO;

/// Delete every sensitive item that is past its TTL and above the auto-wipe
/// floor. Returns how many were removed.
///
/// Run it from a ticker *and* once at startup before the daemon binds its socket
/// (manifest 01 §3.19, bug P2/ugv7) — otherwise a client that connects to a
/// freshly started daemon can read secrets that expired while it was stopped.
///
/// Never returns an error for a row it could not read: a candidate that fails to
/// decrypt is left alone and counted nowhere. Fail closed means refusing to act,
/// and the action here is deletion.
///
/// # Errors
///
/// [`StoreError`] if the candidate query or the delete itself fails. Nothing has
/// been deleted in that case; the next tick retries.
pub fn sweep_sensitive(
    store: &Store,
    detector: &Detector,
    key: &ItemKey,
    ttl: Duration,
    now_ms: i64,
) -> Result<u64, StoreError> {
    if ttl == SENSITIVE_TTL_DISABLED {
        return Ok(0);
    }
    if !store.has_wipeable_sensitive() {
        return Ok(0);
    }

    let ttl_ms = i64::try_from(ttl.as_millis()).unwrap_or(i64::MAX);
    let cutoff_ms = now_ms.saturating_sub(ttl_ms);

    let mut victims = Vec::new();
    for row in store.expired_sensitive(cutoff_ms)? {
        let Ok(plaintext) = decrypt(&row.content_ciphertext, &row.nonce, key, &row.id) else {
            tracing::warn!(id = %row.id, "not wiping a sensitive item that could not be read");
            continue;
        };
        let Ok(text) = String::from_utf8(plaintext) else {
            continue;
        };
        if detector.may_auto_wipe(&text) {
            victims.push(row.id);
        }
    }

    let removed = store.hard_delete(&victims)?;
    if removed > 0 {
        tracing::info!(removed, "wiped expired sensitive items");
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::encrypt;
    use crate::storage::test_support::{fts_row_count, raw_row_count, T0};
    use crate::storage::NewItem;
    use crate::{compute_content_hash, Detector, Keyring};

    struct Fixture {
        store: Store,
        detector: Detector,
        key: ItemKey,
    }

    fn fixture() -> Fixture {
        let keyring = Keyring::from_secret(&[5u8; 32]);
        Fixture {
            store: Store::open_in_memory(&[7u8; 32]).expect("store"),
            detector: Detector::new().expect("detector"),
            key: keyring.item_key(),
        }
    }

    /// Stores `text` the way the ingest path does, so the row is real: sealed
    /// under the item key with the id as AAD, and flagged by the detector.
    fn capture(f: &Fixture, text: &str, created_at: i64) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        let (nonce, ciphertext) = encrypt(text.as_bytes(), &f.key, &id).expect("seal");
        let is_sensitive = f.detector.is_sensitive(text);
        // The *surviving* row's id, which is not the candidate's when the
        // capture was a re-copy.
        f.store
            .insert(NewItem {
                id,
                content_ciphertext: ciphertext,
                nonce,
                content_type: "text".to_string(),
                content_hash: compute_content_hash(text.as_bytes()),
                is_sensitive,
                search_text: if is_sensitive {
                    None
                } else {
                    Some(text.to_string())
                },
                created_at,
                app_bundle_id: None,
                payload_metadata: None,
            })
            .expect("insert")
            .id
    }

    /// Above the floor: a real AWS access key id.
    const SECRET: &str = "AKIAIOSFODNN7EXAMPLE";
    /// Detected, so kept out of the index — but nowhere near the floor, so it
    /// must survive the sweep. This is the false-positive case rule 4 is about.
    const FLAGGED_ONLY: &str = "please email alice.smith@example.com about it";

    #[test]
    fn a_high_confidence_secret_is_wiped_after_its_ttl() {
        let f = fixture();
        let id = capture(&f, SECRET, T0);
        assert!(f.store.get(&id).unwrap().unwrap().is_sensitive);

        // Still inside the TTL.
        let removed = sweep_sensitive(
            &f.store,
            &f.detector,
            &f.key,
            Duration::from_secs(30),
            T0 + 29_000,
        )
        .unwrap();
        assert_eq!(removed, 0);
        assert!(f.store.get(&id).unwrap().is_some());

        let removed = sweep_sensitive(
            &f.store,
            &f.detector,
            &f.key,
            Duration::from_secs(30),
            T0 + 31_000,
        )
        .unwrap();
        assert_eq!(removed, 1);
        assert!(f.store.get(&id).unwrap().is_none());
        // Hard delete: no tombstone is left to travel to another device.
        assert_eq!(raw_row_count(&f.store, &id), 0);
    }

    /// `CLAUDE.md` rule 4: detection may flag, but only the high-confidence band
    /// may delete. An email address is flagged and must still be there.
    #[test]
    fn a_flagged_item_below_the_floor_is_never_wiped() {
        let f = fixture();
        let id = capture(&f, FLAGGED_ONLY, T0);
        assert!(
            f.store.get(&id).unwrap().unwrap().is_sensitive,
            "the fixture must actually be flagged, or this proves nothing"
        );

        let removed = sweep_sensitive(
            &f.store,
            &f.detector,
            &f.key,
            Duration::from_secs(30),
            T0 + 10_000_000,
        )
        .unwrap();
        assert_eq!(removed, 0);
        assert!(f.store.get(&id).unwrap().is_some());
    }

    /// Manifest 01 §3.17 / bug P2: zero is "off", not "expire everything now".
    #[test]
    fn a_zero_ttl_is_disabled_not_immediate() {
        let f = fixture();
        let id = capture(&f, SECRET, T0);
        let removed = sweep_sensitive(
            &f.store,
            &f.detector,
            &f.key,
            SENSITIVE_TTL_DISABLED,
            T0 + 10_000_000,
        )
        .unwrap();
        assert_eq!(removed, 0);
        assert!(f.store.get(&id).unwrap().is_some());
    }

    /// Manifest 03 I9: a pin is the most explicit "keep this" a user can give,
    /// and it outranks the TTL.
    #[test]
    fn a_pinned_secret_is_not_wiped() {
        let f = fixture();
        let id = capture(&f, SECRET, T0);
        assert!(f.store.set_pinned(&id, true).unwrap());

        let removed = sweep_sensitive(
            &f.store,
            &f.detector,
            &f.key,
            Duration::from_secs(30),
            T0 + 10_000_000,
        )
        .unwrap();
        assert_eq!(removed, 0);
        assert!(f.store.get(&id).unwrap().is_some());
    }

    #[test]
    fn ordinary_items_are_untouched_however_old() {
        let f = fixture();
        let id = capture(&f, "just some notes", T0);
        assert!(!f.store.get(&id).unwrap().unwrap().is_sensitive);

        let removed = sweep_sensitive(
            &f.store,
            &f.detector,
            &f.key,
            Duration::from_secs(30),
            T0 + 10_000_000,
        )
        .unwrap();
        assert_eq!(removed, 0);
        assert!(f.store.get(&id).unwrap().is_some());
    }

    /// A row sealed under a different key cannot be proved to be a secret, so it
    /// must not be deleted on the strength of its flag alone.
    #[test]
    fn an_unreadable_candidate_is_left_alone() {
        let f = fixture();
        let id = capture(&f, SECRET, T0);
        let other = Keyring::from_secret(&[9u8; 32]).item_key();

        let removed = sweep_sensitive(
            &f.store,
            &f.detector,
            &other,
            Duration::from_secs(30),
            T0 + 10_000_000,
        )
        .unwrap();
        assert_eq!(removed, 0);
        assert!(f.store.get(&id).unwrap().is_some());
    }

    /// `CopyPaste-8ebg.2`: re-copying a secret must reset its deadline, not
    /// inherit one that is seconds from firing. The bump does that by moving
    /// `created_at`, which is what the derived deadline reads.
    #[test]
    fn re_copying_a_secret_resets_its_deadline() {
        let f = fixture();
        let id = capture(&f, SECRET, T0);
        let again = capture(&f, SECRET, T0 + 25_000);
        assert_eq!(again, id, "the re-copy must have bumped, not duplicated");

        // 30 s after the first capture, but only 5 s after the re-copy.
        let removed = sweep_sensitive(
            &f.store,
            &f.detector,
            &f.key,
            Duration::from_secs(30),
            T0 + 30_000,
        )
        .unwrap();
        assert_eq!(removed, 0);
        assert!(f.store.get(&id).unwrap().is_some());

        let removed = sweep_sensitive(
            &f.store,
            &f.detector,
            &f.key,
            Duration::from_secs(30),
            T0 + 56_000,
        )
        .unwrap();
        assert_eq!(removed, 1);
    }

    #[test]
    fn the_sweep_is_a_no_op_on_a_history_with_no_secrets() {
        let f = fixture();
        capture(&f, "nothing to see", T0);
        assert!(!f.store.has_wipeable_sensitive());
        assert_eq!(
            sweep_sensitive(
                &f.store,
                &f.detector,
                &f.key,
                Duration::from_secs(30),
                T0 + 10_000_000
            )
            .unwrap(),
            0
        );
    }

    #[test]
    fn a_wiped_secret_leaves_no_index_entry() {
        let f = fixture();
        // Sensitive items are never indexed in the first place; this asserts the
        // sweep does not leave an orphan behind for anything that was.
        capture(&f, "ordinary searchable text", T0);
        let id = capture(&f, SECRET, T0);
        sweep_sensitive(
            &f.store,
            &f.detector,
            &f.key,
            Duration::from_secs(30),
            T0 + 60_000,
        )
        .unwrap();

        assert_eq!(fts_row_count(&f.store, &id), 0);
        assert_eq!(f.store.search("ordinary", 10).unwrap().len(), 1);
    }
}
