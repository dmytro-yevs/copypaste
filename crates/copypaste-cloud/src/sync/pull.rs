//! The download path: the cursor, the paging, and the ways a row can be refused
//! without failing the round.
//!
//! # Refusal and the cursor
//!
//! A refused row still advances the cursor past itself, but **never past the
//! local clock**. Both halves of that are load-bearing, and each fixes what the
//! other would break:
//!
//! * Advancing is what stops a refusal from stalling sync. Anyone who can write
//!   to the account can inject a page's worth of rows stamped one millisecond
//!   after the cursor; if refusing them left the cursor where it was, that page
//!   would come back on every pull and nothing behind it would ever download.
//! * The clock ceiling is what stops a refusal from *becoming* the censorship it
//!   is meant to prevent. A forged row stamped a day ahead would otherwise drag
//!   the cursor a day forward and skip every honest row written in between. A
//!   row stamped at or behind `now` cannot skip anything: pages arrive in
//!   ascending keyset order with no gaps, so everything before it has already
//!   been offered. A row stamped ahead of `now` sorts after every honest row, so
//!   leaving the cursor behind it costs one re-fetched row per pull and blocks
//!   nothing.
//!
//! The cursor rules live here because they are one rule seen from three angles
//! — the inclusive bound, the ascending page order, and the watermark that only
//! moves forward — and separating them is how v1 ended up with a `>` on one
//! side and a `>=` on the other.

use super::driver::CloudSync;
use super::outcome::{SyncError, SyncStats};
use super::source::{Applied, CloudSource, LocalItem};
use super::transport::{AuthApi, RestApi};
// One wall clock for the crate: `auth` already owns the saturating
// `SystemTime` read that every expiry and every stamp comparison uses, and a
// second copy of it here would be a second thing to get wrong.
use crate::auth::now_ms;
use crate::crypto::decrypt_row;
use crate::rest::CloudItem;
use zeroize::Zeroizing;

/// Rows requested per page.
///
/// Performance only: correctness comes from the keyset cursor, not from the
/// page size. Larger pages mean fewer round trips on a catch-up; smaller ones
/// mean a shorter tail if the connection drops mid-drain. `SupabaseRest`
/// clamps this to its own `MAX_PAGE_LIMIT`, so asking for more is harmless.
const PULL_PAGE_LIMIT: u32 = 100;

/// Pages one [`CloudSync::pull`] will drain before returning.
///
/// A device that has been offline for a month has thousands of rows waiting.
/// Draining them in one call would make a single `pull` unbounded in time and
/// memory; stopping after this many pages returns control to the caller, which
/// simply polls again — the watermark has advanced, so the next call resumes
/// exactly where this one stopped.
const MAX_PAGES_PER_PULL: usize = 50;

const MSG_BATCH_ARITY: &str = "the history database did not answer for every row of a page";

/// How far ahead of the local clock a row's `created_at` may be before this
/// device refuses to take that version.
///
/// The lower bound is clamped on arrival; the upper bound needs the local clock
/// and so lives here. Without it, one row stamped `i64::MAX` wins every future
/// comparison for its `item_id` and effectively censors that item on every
/// device — and manifest 05 §5.3 notes that under Supabase an attacker who
/// holds the account password, but not the sync passphrase, *can write rows*.
/// A day is far more skew than any real pair of devices shows.
///
/// No correction is attempted, only refusal, and refusal skips **one version**
/// — it never fails the round and never deletes anything (manifest 05 R-CLK-2).
///
/// Must equal `copypaste_p2p::sync::MAX_FUTURE_SKEW_MS`: the two transports
/// share an ordering, so they must share what counts as a valid stamp. There is
/// no dependency edge to import it across, so `tests/constant_parity.rs` fails
/// the build instead if they diverge.
pub const MAX_FUTURE_SKEW_MS: i64 = 24 * 60 * 60 * 1000;

impl<R: RestApi, A: AuthApi> CloudSync<R, A> {
    /// Fetch, open and apply everything since the watermark.
    ///
    /// Drains up to [`MAX_PAGES_PER_PULL`] pages: when a page comes back full
    /// there is more waiting, and waiting a whole poll interval per page makes
    /// a multi-device burst drain at one page per tick.
    ///
    /// Idempotent. Applying the same page twice is absorbed by the store's
    /// ordering (INV-I1), and the watermark only moves forward.
    ///
    /// # Errors
    ///
    /// [`SyncError::Source`] if the store fails. [`SyncError::Unauthorized`],
    /// [`SyncError::InvalidCredentials`] or [`SyncError::SessionExpired`] if
    /// the session cannot be recovered. [`SyncError::RateLimited`] if the
    /// backend throttles twice in a row. [`SyncError::Transport`] if the
    /// transient budget runs out.
    ///
    /// A row that will not decrypt is **not** an error: it is skipped, counted,
    /// and the cursor still advances past it (INV-N3, INV-I4).
    pub async fn pull(&self, source: &dyn CloudSource) -> Result<SyncStats, SyncError> {
        self.pull_with_republish(source)
            .await
            .map(|(stats, _)| stats)
    }

    pub(super) async fn pull_with_republish(
        &self,
        source: &dyn CloudSource,
    ) -> Result<(SyncStats, bool), SyncError> {
        let mut cursor = Cursor {
            created_at: source.watermark()?,
            item_id: source.watermark_item_id()?,
        };
        let mut stats = SyncStats::default();
        let mut republish = false;

        for _ in 0..MAX_PAGES_PER_PULL {
            let mut rows = self
                .execute(|token| {
                    let after = cursor.item_id.clone();
                    async move {
                        self.rest
                            .fetch_since(
                                &token,
                                cursor.created_at,
                                after.as_deref(),
                                PULL_PAGE_LIMIT,
                            )
                            .await
                    }
                })
                .await?;
            let page_len = rows.len();
            stats.downloaded += page_len;
            // Normalise at the page boundary, before the ordering and cursor
            // see a stamp. Sorting raw `-1` before `(0, "a")` and then
            // clamping it to `(0, "z")` would advance the keyset past the
            // honest `(0, "a")` row.
            for row in &mut rows {
                row.created_at = clamp_stamp(row.created_at);
            }
            sort_page(&mut rows);

            let now = now_ms();
            let mut advanced = cursor.clone();
            let mut batch: Vec<LocalItem> = Vec::new();

            for row in rows {
                let created_at = row.created_at;

                if created_at > now.saturating_add(MAX_FUTURE_SKEW_MS) {
                    // Refuse the version, and do *not* let the cursor follow it
                    // into the future — that would skip every legitimate row
                    // behind it. The cost is that this row is re-offered on
                    // each pull; the stall guard below stops a page full of
                    // them from spinning.
                    stats.skipped_future += 1;
                    tracing::warn!(
                        item_id = %row.item_id,
                        "skipping a row stamped implausibly far in the future"
                    );
                    continue;
                }

                // Before the payload is touched, and before anything reaches
                // the comparator: a row whose metadata was not signed by a
                // holder of the sync key is not a version of anything. The
                // backend cannot produce this signature, so this is what stops
                // an account-password holder from stamping a competing version
                // that outranks the real one — or a tombstone that deletes it
                // everywhere (manifest 05 §5.3).
                if row.verify(&self.key).is_err() {
                    stats.skipped_forged += 1;
                    tracing::warn!(
                        item_id = %row.item_id,
                        "refusing a row whose metadata is unsigned or wrongly signed"
                    );
                    advanced.advance_after_refusal(created_at, &row.item_id, now);
                    continue;
                }

                let content = if row.deleted {
                    Zeroizing::new(Vec::new())
                } else {
                    match decrypt_row(&row.ciphertext, &row.nonce, &self.key, &row.item_id) {
                        Ok(plaintext) => plaintext,
                        Err(_) => {
                            // Never a delete, never a partial row, never a log
                            // line containing the payload or the key.
                            stats.skipped_undecryptable += 1;
                            tracing::warn!(
                                item_id = %row.item_id,
                                "skipping a row this device cannot decrypt"
                            );
                            advanced.advance_after_refusal(created_at, &row.item_id, now);
                            continue;
                        }
                    }
                };

                batch.push(LocalItem {
                    item_id: row.item_id,
                    content,
                    content_type: row.content_type,
                    payload_metadata: row.payload_metadata,
                    created_at,
                    deleted: row.deleted,
                    origin_device_id: row.origin_device_id,
                });
            }

            let reached = batch
                .last()
                .map(|item| (item.created_at, item.item_id.clone()));
            let offered = batch.len();
            if offered > 0 {
                let outcomes = source.apply_remote_batch(batch)?;
                if outcomes.len() != offered {
                    return Err(SyncError::Source(MSG_BATCH_ARITY));
                }
                for outcome in outcomes {
                    match outcome {
                        Applied::Merged => stats.applied += 1,
                        Applied::Declined(declined) => {
                            republish |= source.requeue_local_winner(&declined)?;
                        }
                    }
                }
                if let Some((created_at, item_id)) = reached {
                    advanced.advance_past(created_at, &item_id);
                }
            }

            // Persist per page, so an interrupted drain resumes from the last
            // completed page rather than from the start. Monotonic by
            // construction: `advanced` starts at the current cursor and
            // `advance_past` only ever moves it forward.
            if advanced != cursor {
                if let Some(item_id) = advanced.item_id.as_deref() {
                    source.set_watermark_keyset(advanced.created_at, item_id)?;
                }
            }

            // Short page: caught up.
            if page_len < PULL_PAGE_LIMIT as usize {
                break;
            }
            // Full page that produced no progress: every row in it was
            // unusable in a way that does not advance the cursor. Stop rather
            // than re-requesting the same window forever (manifest 05 AT-29).
            if advanced == cursor {
                tracing::warn!(
                    "a full page of rows produced no cursor progress; pausing the drain"
                );
                break;
            }
            cursor = advanced;
        }

        Ok((stats, republish))
    }
}

/// Where the download has reached, as a keyset over `(created_at, item_id)`.
///
/// The pair, not the millisecond: a millisecond is not unique, so a cursor that
/// carries only one cannot be advanced past a millisecond that holds more than
/// a page of rows — every pull re-fetches the same first page of them and the
/// rest never download (INV-N1, manifest 05 §5.1 row 6). `item_id` is `None`
/// only before the first row of a round has been seen, which is the inclusive
/// `gte` case in [`RestApi::fetch_since`](super::RestApi::fetch_since).
#[derive(Clone, PartialEq, Eq)]
struct Cursor {
    created_at: i64,
    item_id: Option<String>,
}

impl Cursor {
    /// Move to this row's position, if it is ahead of where we are.
    ///
    /// Guarded rather than assigned: pages arrive sorted, but a cursor that
    /// could move backwards would re-download history on the next round, and
    /// the "only ever forward" property is what INV-N5 leans on.
    fn advance_past(&mut self, created_at: i64, item_id: &str) {
        let ahead = (created_at, Some(item_id)) > (self.created_at, self.item_id.as_deref());
        if ahead {
            self.created_at = created_at;
            self.item_id = Some(item_id.to_owned());
        }
    }

    /// Move past a row this device refused, if it is safe to.
    ///
    /// Refused rows are the only ones whose stamp is entirely attacker-chosen,
    /// so they are the only ones the cursor must not follow into the future.
    /// See the module docs for why both the advance and the ceiling are needed.
    fn advance_after_refusal(&mut self, created_at: i64, item_id: &str, now: i64) {
        if created_at <= now {
            self.advance_past(created_at, item_id);
        }
    }
}

/// Put a page in the order the cursor needs, and say so if it was not already.
///
/// The sort is cheap insurance against a page that is merely *unordered*.
/// It is **not** a repair for a page that is ordered newest-first: a
/// newest-first source returns the newest `limit` rows and there is no way,
/// from this side, to reach the older ones behind them. The warning exists so
/// that failure is visible in a log rather than showing up months later as
/// "my old history never downloaded" — see
/// [`RestApi::fetch_since`](super::RestApi::fetch_since).
fn sort_page(rows: &mut [CloudItem]) {
    let descending = rows
        .windows(2)
        .any(|w| (w[1].created_at, &w[1].item_id) < (w[0].created_at, &w[0].item_id));
    if descending {
        tracing::warn!(
            rows = rows.len(),
            "a page arrived out of cursor order; a newest-first page cannot be drained \
             and older rows behind it will not be fetched"
        );
    }
    rows.sort_by(|a, b| {
        a.created_at
            .cmp(&b.created_at)
            .then_with(|| a.item_id.cmp(&b.item_id))
    });
}

/// Clamp a wire timestamp's lower bound.
///
/// At the decode boundary, not at the use site: a validation that lives in one
/// consumer is one a second consumer will silently skip (manifest 05 R-CLK-1).
/// A negative stamp cast to unsigned becomes the largest possible value and
/// wins every comparison forever (`CopyPaste-psx7`).
fn clamp_stamp(raw: i64) -> i64 {
    raw.max(0)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;

    use super::super::fakes::{
        cloud_row, cloud_tombstone, driver, item, signed, FakeAuth, FakeRest, FakeSource, PASS,
    };
    use super::*;
    use crate::crypto::encrypt_row;

    #[tokio::test]
    async fn pull_opens_rows_and_applies_them() {
        let rest = FakeRest::seeded(vec![
            cloud_row("a", 1_000, "first"),
            cloud_row("b", 2_000, "second"),
        ]);
        let source = FakeSource::default();
        let sync = driver(rest, FakeAuth::default());

        let stats = sync.pull(&source).await.unwrap();

        assert_eq!(stats.downloaded, 2);
        assert_eq!(stats.applied, 2);
        assert_eq!(source.get("a").unwrap().content.as_slice(), b"first");
        assert_eq!(source.get("b").unwrap().content.as_slice(), b"second");
        assert_eq!(source.watermark().unwrap(), 2_000);
    }

    #[tokio::test]
    async fn a_page_is_merged_through_one_batch_call_that_answers_for_every_row() {
        let rows: Vec<CloudItem> = (0..PULL_PAGE_LIMIT as usize + 30)
            .map(|i| cloud_row(&format!("item-{i:04}"), 1_000 + i as i64, "x"))
            .collect();
        let source = FakeSource::default();
        let sync = driver(FakeRest::seeded(rows), FakeAuth::default());

        let stats = sync.pull(&source).await.unwrap();

        assert_eq!(stats.applied, PULL_PAGE_LIMIT as usize + 30);
        assert_eq!(
            source.batches.load(Ordering::SeqCst),
            2,
            "the merge was driven per row rather than per page"
        );
    }

    #[tokio::test]
    async fn a_declined_row_is_still_reported_on_its_own_after_batching() {
        let local = item("a", 5_000, "the local winner");
        let source = FakeSource::with_local(local);
        source.set_upload_floor(9_000);
        let sync = driver(
            FakeRest::seeded(vec![
                cloud_row("a", 1_000, "an older remote version"),
                cloud_row("b", 2_000, "a new one"),
            ]),
            FakeAuth::default(),
        );

        let stats = sync.pull(&source).await.unwrap();

        assert_eq!(stats.applied, 1, "only the unseen row should have merged");
        assert_eq!(
            source.get("a").unwrap().content.as_slice(),
            b"the local winner"
        );
        assert_eq!(
            source.upload_floor().unwrap(),
            5_000,
            "the declined row's local winner was not re-offered"
        );
    }

    #[tokio::test]
    async fn a_batch_that_answers_for_fewer_rows_than_it_was_given_fails_the_page() {
        let source = FakeSource::dropping_one_batch_outcome();
        let sync = driver(
            FakeRest::seeded(vec![
                cloud_row("a", 1_000, "first"),
                cloud_row("b", 2_000, "second"),
            ]),
            FakeAuth::default(),
        );

        assert!(matches!(
            sync.pull(&source).await,
            Err(SyncError::Source(MSG_BATCH_ARITY))
        ));
        assert_eq!(
            source.watermark().unwrap(),
            0,
            "the cursor advanced over a page the source did not answer for"
        );
    }

    #[tokio::test]
    async fn pull_is_idempotent() {
        let rest = FakeRest::seeded(vec![cloud_row("a", 1_000, "first")]);
        let source = FakeSource::default();
        let sync = driver(rest, FakeAuth::default());

        let first = sync.pull(&source).await.unwrap();
        let second = sync.pull(&source).await.unwrap();

        assert_eq!(first.applied, 1);
        // The row is re-offered (the cursor bound is inclusive) and loses the
        // ordering, so nothing changes.
        assert_eq!(second.applied, 0);
        assert_eq!(source.get("a").unwrap().content.as_slice(), b"first");
        assert_eq!(source.watermark().unwrap(), 1_000);
    }

    #[tokio::test]
    async fn a_self_echoed_row_changes_nothing() {
        // INV-I2: a device both writes to and reads from the same account, so
        // it gets its own writes back. Absorbed by the ordering, not by a
        // "did I send this?" filter.
        let source = FakeSource::with_outgoing(vec![item("a", 1_000, "mine")]);
        let sync = driver(FakeRest::default(), FakeAuth::default());

        sync.push(&source).await.unwrap();
        sync.pull(&source).await.unwrap();
        let after_first = source.get("a");

        sync.pull(&source).await.unwrap();
        assert_eq!(source.get("a"), after_first);
    }

    #[tokio::test]
    async fn a_tombstone_reaches_the_store_even_for_an_unknown_item() {
        // T-3 / CopyPaste-bfiu. Dropping it here lets a later-arriving create
        // resurrect the item, so the tombstone must be handed down.
        let source = FakeSource::default();
        let sync = driver(
            FakeRest::seeded(vec![cloud_tombstone("gone", 4_000)]),
            FakeAuth::default(),
        );

        let stats = sync.pull(&source).await.unwrap();
        assert_eq!(stats.applied, 1);

        let stored = source.get("gone").expect("no tombstone was persisted");
        assert!(stored.deleted);
        assert!(stored.content.is_empty());
    }

    #[tokio::test]
    async fn a_newer_tombstone_beats_an_older_live_version() {
        let source = FakeSource::default();
        let sync = driver(
            FakeRest::seeded(vec![cloud_row("a", 1_000, "live")]),
            FakeAuth::default(),
        );
        sync.pull(&source).await.unwrap();
        assert!(!source.get("a").unwrap().deleted);

        // Now the delete arrives, stamped later.
        sync.rest
            .rows
            .lock()
            .unwrap()
            .insert("a".into(), cloud_tombstone("a", 2_000));
        sync.pull(&source).await.unwrap();

        let stored = source.get("a").unwrap();
        assert!(stored.deleted, "delete did not win");
        assert!(stored.content.is_empty(), "content survived the tombstone");
    }

    #[tokio::test]
    async fn an_older_live_version_cannot_resurrect_a_newer_tombstone() {
        let source = FakeSource::default();
        // Tombstone first, at a later stamp than the create that follows it.
        let sync = driver(
            FakeRest::seeded(vec![cloud_tombstone("a", 5_000)]),
            FakeAuth::default(),
        );
        sync.pull(&source).await.unwrap();
        assert!(source.get("a").unwrap().deleted);

        // The out-of-order create arrives.
        sync.rest
            .rows
            .lock()
            .unwrap()
            .insert("a".into(), cloud_row("a", 1_000, "back from the dead"));
        // Re-offer it from the start: the ordering, not the cursor, is what has
        // to keep the item dead.
        source.rewind(0);
        sync.pull(&source).await.unwrap();

        assert!(source.get("a").unwrap().deleted, "the item was resurrected");
    }

    #[tokio::test]
    async fn an_undecryptable_row_is_skipped_and_never_deletes_the_local_copy() {
        // INV-N3. Two rows: one this device can open, one sealed under another
        // account's key. The bad one must not stop the good one, must not be
        // persisted, and must not stall the cursor.
        let other_key = crate::crypto::derive_sync_key(PASS, "another-account").unwrap();
        let (nonce, ciphertext) = encrypt_row(b"not for us", &other_key, "b").unwrap();

        let rest = FakeRest::seeded(vec![
            cloud_row("a", 1_000, "readable"),
            // Signed with *our* key so the row gets as far as the decrypt: the
            // point of this test is the payload, not the metadata.
            signed(CloudItem {
                item_id: "b".into(),
                ciphertext,
                nonce,
                content_type: "text".into(),
                payload_metadata: None,
                created_at: 2_000,
                deleted: false,
                origin_device_id: "device-b".into(),
                signature: String::new(),
            }),
        ]);
        let source = FakeSource::default();
        let sync = driver(rest, FakeAuth::default());

        let stats = sync.pull(&source).await.unwrap();

        assert_eq!(stats.downloaded, 2);
        assert_eq!(stats.applied, 1);
        assert_eq!(stats.skipped_undecryptable, 1);
        assert!(source.get("a").is_some());
        assert!(source.get("b").is_none(), "a poison row was persisted");
        // INV-I4: the cursor advanced past the unreadable row, so it is not
        // re-fetched forever.
        assert_eq!(source.watermark().unwrap(), 2_000);
    }

    // --- signed metadata (manifest 05 §5.3) --------------------------------

    #[tokio::test]
    async fn a_row_whose_metadata_was_restamped_is_refused() {
        // The §5.3 attack in full: something holding the account password, but
        // not the sync passphrase, rewrites a real row's `created_at` so its
        // version outranks every honest one for that item. The ciphertext is
        // untouched and still opens — encryption cannot see this — so the
        // signature is the only thing that can refuse it.
        let source = FakeSource::default();
        let sync = driver(
            FakeRest::seeded(vec![cloud_row("a", 1_000, "the real version")]),
            FakeAuth::default(),
        );
        sync.pull(&source).await.unwrap();
        assert_eq!(
            source.get("a").unwrap().content.as_slice(),
            b"the real version"
        );

        // The backend rewrites the stamp, leaving the signature as it was.
        sync.rest
            .rows
            .lock()
            .unwrap()
            .get_mut("a")
            .unwrap()
            .created_at = 9_000;
        source.rewind(0);

        let stats = sync.pull(&source).await.unwrap();

        assert_eq!(stats.skipped_forged, 1);
        assert_eq!(stats.applied, 0);
        assert_eq!(
            source.get("a").unwrap().created_at,
            1_000,
            "a forged stamp reached the merge"
        );
    }

    #[tokio::test]
    async fn a_forged_tombstone_cannot_delete_an_item() {
        // The most destructive shape: one write into the account, and the item
        // is gone from every device. Data loss is the worst outcome
        // (`AGENTS.md` rule 4), so this one is asserted on its own.
        let source = FakeSource::default();
        let sync = driver(
            FakeRest::seeded(vec![cloud_row("a", 1_000, "keep me")]),
            FakeAuth::default(),
        );
        sync.pull(&source).await.unwrap();

        // Signed by something that is not us: a real signature, wrong key.
        let attacker = crate::crypto::derive_sync_key(PASS, "attacker-account").unwrap();
        let mut forged = CloudItem::tombstone("a", "text", 5_000, "device-b");
        forged.sign(&attacker);
        sync.rest.rows.lock().unwrap().insert("a".into(), forged);

        let stats = sync.pull(&source).await.unwrap();

        assert_eq!(stats.skipped_forged, 1);
        let stored = source.get("a").expect("the item was deleted by a forgery");
        assert!(!stored.deleted);
        assert_eq!(stored.content.as_slice(), b"keep me");
    }

    #[tokio::test]
    async fn an_unsigned_row_is_refused_and_does_not_stall_the_cursor() {
        // Fail closed, but not fail stuck: a page full of unsigned rows must
        // not park the cursor in front of them forever, or anyone who can write
        // to the account can stop sync with a hundred cheap rows.
        let mut unsigned = cloud_row("forged", 1_000, "not from a key holder");
        unsigned.signature = String::new();

        let rest = FakeRest::seeded(vec![unsigned, cloud_row("real", 2_000, "genuine")]);
        let source = FakeSource::default();
        let sync = driver(rest, FakeAuth::default());

        let stats = sync.pull(&source).await.unwrap();

        assert_eq!(stats.skipped_forged, 1);
        assert_eq!(stats.applied, 1);
        assert!(source.get("forged").is_none());
        assert_eq!(source.get("real").unwrap().content.as_slice(), b"genuine");
        assert_eq!(source.watermark().unwrap(), 2_000);

        // And the refused row is not re-offered forever: the cursor is past it.
        source.rewind(0);
        let mut only_forged = FakeRest::seeded(vec![]);
        only_forged.rows = std::sync::Mutex::new(
            sync.rest
                .rows
                .lock()
                .unwrap()
                .iter()
                .filter(|(id, _)| id.as_str() == "forged")
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        );
        let sync = driver(only_forged, FakeAuth::default());
        sync.pull(&source).await.unwrap();
        assert_eq!(
            source.watermark().unwrap(),
            1_000,
            "the cursor did not move past a refused row, so it can be stalled"
        );
    }

    #[tokio::test]
    async fn a_refused_row_cannot_drag_the_cursor_into_the_future() {
        // The other half of the rule. If refusal advanced unconditionally, one
        // forged row stamped just inside the skew window would push the cursor
        // most of a day forward and skip every honest row written in between —
        // the censorship the signature exists to prevent, delivered by the
        // check that is meant to prevent it.
        let ahead = now_ms() + MAX_FUTURE_SKEW_MS / 2;
        let mut forged = cloud_row("forged", ahead, "tomorrow");
        forged.signature = String::new();

        let source = FakeSource::default();
        let sync = driver(
            FakeRest::seeded(vec![cloud_row("real", 1_000, "genuine"), forged]),
            FakeAuth::default(),
        );

        let stats = sync.pull(&source).await.unwrap();

        assert_eq!(stats.skipped_forged, 1);
        assert_eq!(
            source.watermark().unwrap(),
            1_000,
            "the cursor followed a refused row into the future"
        );
    }

    #[tokio::test]
    async fn a_row_stamped_far_in_the_future_is_refused_and_does_not_move_the_cursor() {
        let far = now_ms() + MAX_FUTURE_SKEW_MS * 10;
        let rest = FakeRest::seeded(vec![
            cloud_row("a", 1_000, "real"),
            cloud_row("hostile", far, "censoring version"),
        ]);
        let source = FakeSource::default();
        let sync = driver(rest, FakeAuth::default());

        let stats = sync.pull(&source).await.unwrap();

        assert_eq!(stats.skipped_future, 1);
        assert!(source.get("hostile").is_none());
        assert_eq!(source.get("a").unwrap().content.as_slice(), b"real");
        assert_eq!(
            source.watermark().unwrap(),
            1_000,
            "the cursor followed a hostile stamp into the future"
        );
    }

    #[test]
    fn a_page_is_put_into_cursor_order_before_it_is_applied() {
        // Defensive: the transport is required to page oldest-first, and the
        // cursor cannot drain a newest-first page. Sorting costs nothing and
        // means an unordered page is merely slow rather than wrong.
        let mut page = vec![
            cloud_row("c", 3_000, "third"),
            cloud_row("a", 1_000, "first"),
            cloud_row("b2", 2_000, "second, later id"),
            cloud_row("b1", 2_000, "second, earlier id"),
        ];
        sort_page(&mut page);

        let order: Vec<&str> = page.iter().map(|r| r.item_id.as_str()).collect();
        // Compound key: `created_at` then `item_id`, so the ordering has no
        // ties even for a burst inside one millisecond (INV-N1).
        assert_eq!(order, ["a", "b1", "b2", "c"]);
    }

    #[test]
    fn a_negative_stamp_is_clamped_at_the_boundary() {
        // CopyPaste-psx7: a negative value cast to unsigned becomes the largest
        // possible one and wins every comparison forever. Clamped on arrival,
        // not at the use site, so every ingress path is covered.
        assert_eq!(clamp_stamp(-42), 0);
        assert_eq!(clamp_stamp(i64::MIN), 0);
        assert_eq!(clamp_stamp(0), 0);
        assert_eq!(clamp_stamp(1_700_000_000_000), 1_700_000_000_000);
    }

    #[tokio::test]
    async fn a_negative_row_cannot_sort_a_zero_boundary_past_the_cursor() {
        // The negative row's id sorts after the honest one. If sorting happened
        // before normalisation it would become `(0, "z-negative")` first and
        // cause `(0, "a-zero")` to be skipped by keyset advancement.
        let source = FakeSource::with_keyset_watermark();
        // A malformed persisted cursor or transport fixture can still expose a
        // negative row even though production cursors are clamped at zero.
        source.rewind(-1);
        let sync = driver(
            FakeRest::seeded(vec![
                cloud_row("z-negative", -1, "bad clock"),
                cloud_row("a-zero", 0, "honest boundary row"),
            ]),
            FakeAuth::default(),
        );

        let stats = sync.pull(&source).await.unwrap();

        assert_eq!(stats.applied, 1);
        assert_eq!(
            source.get("a-zero").unwrap().content.as_slice(),
            b"honest boundary row"
        );
        assert_eq!(source.watermark().unwrap(), 0);
        assert_eq!(
            source.watermark_item_id().unwrap().as_deref(),
            Some("z-negative")
        );
    }

    #[tokio::test]
    async fn pull_drains_more_rows_than_one_page() {
        // AT-23/AT-24 in miniature: more rows than a page, all fetched, and the
        // cursor is inclusive so the boundary row is never dropped.
        let rows: Vec<CloudItem> = (0..PULL_PAGE_LIMIT as usize + 30)
            .map(|i| cloud_row(&format!("item-{i:04}"), 1_000 + i as i64, "x"))
            .collect();
        let source = FakeSource::default();
        let sync = driver(FakeRest::seeded(rows), FakeAuth::default());

        let stats = sync.pull(&source).await.unwrap();

        assert_eq!(stats.applied, PULL_PAGE_LIMIT as usize + 30);
        assert!(
            sync.rest.fetches.load(Ordering::SeqCst) >= 2,
            "a full page did not trigger a burst drain"
        );
    }

    /// AT-24 / INV-N1, in the shape that survived into v2: more than one page
    /// of rows stamped inside a single millisecond. A cursor carrying only the
    /// millisecond cannot advance past them — every pull re-fetches the same
    /// first page by `item_id` and the rest never arrive.
    #[tokio::test]
    async fn a_millisecond_holding_more_than_one_page_still_drains() {
        let burst = PULL_PAGE_LIMIT as usize * 2 + 7;
        let rows: Vec<CloudItem> = (0..burst)
            .map(|i| cloud_row(&format!("item-{i:04}"), 1_000, "bulk import"))
            .collect();
        let source = FakeSource::default();
        let sync = driver(FakeRest::seeded(rows), FakeAuth::default());

        let stats = sync.pull(&source).await.unwrap();

        assert_eq!(stats.applied, burst, "rows behind the boundary millisecond");
        assert_eq!(source.watermark().unwrap(), 1_000);
    }

    #[tokio::test]
    async fn a_source_that_persists_the_tie_break_never_refetches_the_boundary() {
        let source = FakeSource::with_keyset_watermark();
        let sync = driver(
            FakeRest::seeded(vec![
                cloud_row("a", 1_000, "first"),
                cloud_row("b", 1_000, "same millisecond"),
            ]),
            FakeAuth::default(),
        );

        assert_eq!(sync.pull(&source).await.unwrap().applied, 2);
        // The second round starts strictly after `(1_000, "b")`, so it fetches
        // nothing at all rather than re-offering the pair.
        let second = sync.pull(&source).await.unwrap();
        assert_eq!(second.downloaded, 0);
        assert_eq!(source.watermark_item_id().unwrap().as_deref(), Some("b"));
    }

    #[tokio::test]
    async fn the_watermark_only_moves_forward() {
        // FakeSource asserts this internally; do it explicitly too, because
        // INV-N5 is about local pruning not dragging the cursor back.
        let source = FakeSource::default();
        source.set_watermark(9_000).unwrap();

        let sync = driver(
            FakeRest::seeded(vec![cloud_row("a", 1_000, "older than the cursor")]),
            FakeAuth::default(),
        );
        let stats = sync.pull(&source).await.unwrap();

        assert_eq!(stats.downloaded, 0, "a row behind the cursor was refetched");
        assert_eq!(source.watermark().unwrap(), 9_000);
    }
}
