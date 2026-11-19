//! The download path: the cursor, the listing, and the ways a row can be
//! refused without failing the round.
//!
//! # The cursor
//!
//! The cursor is a position the backend assigned, never a stamp a device
//! wrote. A row uploaded late, or under a wrong clock, still lands after every
//! row accepted before it, so a forward cursor meets it. Every row listed is
//! passed once it has been dealt with, whatever was done with it: refusing one
//! cannot stall the rows behind it, and no row chooses its own position.
//!
//! # Listing first
//!
//! A pull lists what the backend holds after the cursor — which version each
//! row claims to be and where it was placed, never the payload — and fetches
//! whole only the rows this process has not dealt with. A position is taken
//! when a write begins and the row is visible only when it commits, so each
//! listing starts [`OVERLAP`] below the cursor: a slow write that surfaces
//! behind it is listed and fetched, and everything else there costs a line.

use std::collections::{HashSet, VecDeque};
use std::sync::MutexGuard;
use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};

use super::driver::CloudSync;
use super::outcome::{SyncError, SyncStats};
use super::source::{Applied, CloudSource, LocalItem, PullCursor};
use super::space::RowKey;
use super::transport::{AuthApi, RestApi};
use crate::crypto::decrypt_row;
use crate::rest::body::MAX_PAYLOAD_B64_BYTES;
use crate::rest::{CloudItem, RowListing, RowPage, MAX_PAGE_LIMIT, REST_TIMEOUT};
use zeroize::Zeroizing;

/// Rows listed per page. A listed row is a line of text, so the widest page
/// the backend serves is also the cheapest way through a backlog.
const LIST_PAGE_LIMIT: u32 = MAX_PAGE_LIMIT;

/// Rows asked for by id in one request, and the room their ids may take in its
/// address: an id is at most 128 characters, and a proxy in front of the
/// backend need not accept a longer line than this leaves.
const FETCH_ROWS: usize = 100;
const FETCH_ID_BYTES: usize = 3 * 1024;

/// Requests one [`CloudSync::pull`] makes before returning.
///
/// A device that has been offline for a month has thousands of rows waiting.
/// Draining them in one call would make a single `pull` unbounded in time and
/// traffic; stopping here returns control to the caller, which simply polls
/// again — the cursor has advanced, so the next call resumes where this one
/// stopped, and a pull that applied anything resets the poll interval to its
/// floor. Each fetch is cut at [`PAGE_BYTES`](crate::rest::body::PAGE_BYTES),
/// so one pull reads at most this many times two maximal rows.
const MAX_REQUESTS_PER_PULL: usize = 50;

/// How long a pull keeps asking for further listing pages.
const DRAIN_SPAN: Duration = REST_TIMEOUT;

/// How far below the cursor every listing starts.
///
/// A row is placed when its write begins and listed only once it commits.
/// Supabase by default cancels a statement of the `authenticated` role after
/// 8 s and this client abandons a request after [`REST_TIMEOUT`], which is the
/// allowance for one write. A pull may also move the cursor for up to
/// [`DRAIN_SPAN`] after its first listing, past rows that began before it.
/// The two together are what a later listing has to reach back over.
const OVERLAP: Duration = Duration::from_secs(60);

/// Versions and stored rows remembered as dealt with. Enough for twice what
/// the backend keeps for an account; forgetting one costs a download again.
const HANDLED_KEYS: usize = 2048;

/// Decrypted content held before it is merged. One maximal item fills a batch,
/// so a batch holds less than two.
const APPLY_BATCH_BYTES: usize = copypaste_ipc::MAX_CONTENT_BYTES;

const MSG_BATCH_ARITY: &str = "the history database did not answer for every row of a page";
const MSG_UNPLACED_ROW: &str = "the sync backend returned a row without its position";

/// How far ahead of the moment the backend took a row its `created_at` may be
/// before this device refuses to take that version.
///
/// The lower bound is clamped on arrival. Without the upper one, a row stamped
/// `i64::MAX` wins every future comparison for its `item_id` and effectively
/// censors that item on every device — and manifest 05 §5.3 notes that under
/// Supabase an attacker who holds the account password, but not the sync
/// passphrase, *can write rows*. A day is far more skew than any real device
/// shows against the backend.
///
/// No correction is attempted, only refusal, and refusal skips **one version**
/// — it never fails the round and never deletes anything (manifest 05 R-CLK-2).
///
/// Must equal `copypaste_p2p::sync::MAX_FUTURE_SKEW_MS`: the two transports
/// share an ordering, so they must share what counts as a valid stamp. There is
/// no dependency edge to import it across, so `tests/constant_parity.rs` fails
/// the build instead if they diverge.
pub const MAX_FUTURE_SKEW_MS: i64 = 24 * 60 * 60 * 1000;

type Key = [u8; 16];

/// What this process has already dealt with, so that a listing which names it
/// again costs no download. Held in memory and bounded, oldest out first.
///
/// Two kinds of key. A *version* — what the merge would see, the same for every
/// upload of it — is added only once this device uploaded it, or verified its
/// signature and had the store take or decline it; a refused row never adds
/// one, or a garbage row claiming a real version would hide the genuine one.
/// A *stored row* — one item at the position one write gave it — is added for
/// anything dealt with, so a refused row is not fetched again.
#[derive(Default)]
pub(super) struct Handled {
    keys: HashSet<Key>,
    order: VecDeque<Key>,
}

impl Handled {
    fn insert(&mut self, key: Key) {
        if self.keys.insert(key) {
            self.order.push_back(key);
            if self.order.len() > HANDLED_KEYS {
                if let Some(oldest) = self.order.pop_front() {
                    self.keys.remove(&oldest);
                }
            }
        }
    }

    fn clear(&mut self) {
        self.keys.clear();
        self.order.clear();
    }
}

fn key(kind: &str, item_id: &str, rest: std::fmt::Arguments<'_>) -> Key {
    // `item_id` holds no NUL, so the fields cannot run into each other.
    let digest = Sha256::digest(format!("{kind}\0{item_id}\0{rest}"));
    let mut key = Key::default();
    key.copy_from_slice(&digest[..16]);
    key
}

fn version_key(item_id: &str, created_at: i64, deleted: bool, origin: &str) -> Key {
    key(
        "version",
        item_id,
        format_args!("{created_at}\0{deleted}\0{origin}"),
    )
}

fn stored_key(item_id: &str, position: i64) -> Key {
    key("stored", item_id, format_args!("{position}"))
}

/// Why a row is not merged.
enum Refusal {
    /// Sealed under a key this device does not hold yet.
    NewerEpoch,
    /// Written under a key a removed device may hold.
    OlderEpoch,
    Future,
    Forged,
    TooLarge,
    Undecryptable,
}

/// One listed row and whether it has been dealt with: skipped as handled,
/// refused, fetched and processed, or found gone.
struct Entry {
    listing: RowListing,
    done: bool,
}

impl<R: RestApi, A: AuthApi> CloudSync<R, A> {
    /// List what the backend holds after the cursor, then fetch, open and
    /// apply what this process has not dealt with.
    ///
    /// Idempotent. Applying the same row twice is absorbed by the store's
    /// ordering (INV-I1). The cursor never moves back, and within one position
    /// it follows the server's order.
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
        let cursor = self.load_cursor(source)?;
        self.pull_from(source, cursor).await.map(|(stats, _)| stats)
    }

    /// The cursor this account and key may continue from. Starts over, and
    /// forgets what was dealt with, when the stored one cannot be trusted to
    /// have counted these rows: it belongs to another account, or rows of a
    /// later epoch were passed unread while this device lacked its key.
    ///
    /// Before a round uploads anything, so that what the round then uploads
    /// is still remembered when it lists.
    pub(super) fn load_cursor(&self, source: &dyn CloudSource) -> Result<PullCursor, SyncError> {
        let row_key = self
            .row_key()
            .ok_or(SyncError::Source("the space key is not available yet"))?;
        let account = self.inspect_session(|session| session.user_id.clone());
        let mut stored = source.pull_cursor()?;
        if stored.account != account || row_key.epoch > stored.epoch {
            stored = PullCursor {
                account,
                epoch: row_key.epoch,
                ..PullCursor::default()
            };
            self.lock_handled().clear();
            source.set_pull_cursor(&stored)?;
        }
        Ok(stored)
    }

    pub(super) async fn pull_from(
        &self,
        source: &dyn CloudSource,
        mut stored: PullCursor,
    ) -> Result<(SyncStats, bool), SyncError> {
        let row_key = self
            .row_key()
            .ok_or(SyncError::Source("the space key is not available yet"))?;
        stored.epoch = row_key.epoch;
        let head = Cursor {
            position: stored.position,
            item_id: stored.item_id.clone(),
        };
        let overlap = i64::try_from(OVERLAP.as_micros()).unwrap_or(i64::MAX);
        let mut cursor = Cursor {
            position: stored.position.saturating_sub(overlap).max(0),
            item_id: None,
        };
        // The overlap lists rows at or below `head` first; only what follows
        // them in the server's order moves the cursor.
        let mut beyond = false;
        let started = Instant::now();
        let mut stats = SyncStats::default();
        let mut republish = false;
        let mut requests = 0usize;

        while requests < MAX_REQUESTS_PER_PULL {
            let listed = self
                .execute(|token| {
                    let after = cursor.item_id.clone();
                    async move {
                        self.rest
                            .list_since(&token, cursor.position, after.as_deref(), LIST_PAGE_LIMIT)
                            .await
                    }
                })
                .await?;
            requests += 1;
            // A later page moves the cursor past rows whose writes began
            // before the first; `OVERLAP` reaches back over this long.
            if requests > 1 && started.elapsed() > DRAIN_SPAN {
                break;
            }
            let full = listed.len() >= LIST_PAGE_LIMIT as usize;
            let mut entries: Vec<Entry> = listed
                .into_iter()
                .map(|mut listing| {
                    listing.created_at = clamp_stamp(listing.created_at);
                    let done = !self.wants(&listing, &row_key, &mut stats);
                    Entry { listing, done }
                })
                .collect();

            let mut advanced = cursor.clone();
            let mut passed = 0usize;
            let mut exhausted = false;
            loop {
                // Pass what has been dealt with, in the order it was listed,
                // and persist it so an interrupted drain resumes from here.
                let mut reached = false;
                while let Some(entry) = entries.get(passed).filter(|entry| entry.done) {
                    if advanced.pass(entry.listing.position, &entry.listing.item_id) {
                        if beyond || advanced.position > head.position {
                            beyond = true;
                            reached = true;
                        } else if advanced == head {
                            beyond = true;
                        }
                    }
                    passed += 1;
                }
                if reached {
                    stored.position = advanced.position;
                    stored.item_id = advanced.item_id.clone();
                    source.set_pull_cursor(&stored)?;
                }

                let wanted = next_ids(&entries[passed..]);
                if wanted.is_empty() {
                    break;
                }
                if requests >= MAX_REQUESTS_PER_PULL {
                    exhausted = true;
                    break;
                }
                let RowPage {
                    rows,
                    oversized,
                    cut,
                } = self
                    .execute(|token| {
                        let wanted = &wanted;
                        async move { self.rest.fetch_rows(&token, wanted).await }
                    })
                    .await?;
                requests += 1;
                stats.downloaded += rows.len();
                // A cut page holds at least one of the rows asked for; one
                // that does not would be asked for again forever.
                if cut && !rows.iter().any(|row| wanted.contains(&row.item_id)) {
                    exhausted = true;
                    break;
                }

                let mut batch: Vec<LocalItem> = Vec::new();
                let mut batch_keys: Vec<Key> = Vec::new();
                let mut batch_bytes = 0usize;
                for mut row in rows {
                    // As loud as a page that does not parse: a row that cannot
                    // be placed is never skipped.
                    let position = row.position.ok_or(SyncError::Transport(MSG_UNPLACED_ROW))?;
                    row.created_at = clamp_stamp(row.created_at);
                    if let Some(entry) = entries[passed..]
                        .iter_mut()
                        .find(|entry| entry.listing.item_id == row.item_id)
                    {
                        entry.done = true;
                    }
                    let stored_row = stored_key(&row.item_id, position);
                    let version = version_key(
                        &row.item_id,
                        row.created_at,
                        row.deleted,
                        &row.origin_device_id,
                    );
                    let item_id = row.item_id.clone();
                    let too_long = oversized.contains(&row.item_id);
                    match open_row(row, position, too_long, &row_key) {
                        Ok(item) => {
                            batch_bytes += item.content.len();
                            batch.push(item);
                            batch_keys.extend([stored_row, version]);
                        }
                        Err(refusal) => {
                            self.note_refusal(&refusal, &item_id, &mut stats);
                            self.lock_handled().insert(stored_row);
                        }
                    }
                    if batch_bytes >= APPLY_BATCH_BYTES {
                        republish |= apply_batch(source, std::mem::take(&mut batch), &mut stats)?;
                        self.note_handled(std::mem::take(&mut batch_keys));
                        batch_bytes = 0;
                    }
                }
                republish |= apply_batch(source, batch, &mut stats)?;
                self.note_handled(batch_keys);

                if !cut {
                    // Asked for, and absent from a page that was not cut: the
                    // row no longer exists.
                    for entry in &mut entries[passed..] {
                        entry.done |= wanted.contains(&entry.listing.item_id);
                    }
                }
            }

            if exhausted || !full {
                break;
            }
            // Full page that produced no progress: no row in it was placed
            // after the cursor. Stop rather than re-requesting the same window
            // forever (manifest 05 AT-29).
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

    /// Whether a listed row has to be fetched: this process has not dealt with
    /// it, and what the listing says of it does not already refuse it.
    fn wants(&self, listing: &RowListing, row_key: &RowKey, stats: &mut SyncStats) -> bool {
        let stored_row = stored_key(&listing.item_id, listing.position);
        let version = version_key(
            &listing.item_id,
            listing.created_at,
            listing.deleted,
            &listing.origin_device_id,
        );
        let mut handled = self.lock_handled();
        if handled.keys.contains(&stored_row) || handled.keys.contains(&version) {
            return false;
        }
        let Some(refusal) = refusal(
            listing.key_epoch(),
            listing.created_at,
            listing.position,
            row_key.epoch,
        ) else {
            return true;
        };
        handled.insert(stored_row);
        drop(handled);
        self.note_refusal(&refusal, &listing.item_id, stats);
        false
    }

    fn note_refusal(&self, refusal: &Refusal, item_id: &str, stats: &mut SyncStats) {
        let reason = match refusal {
            Refusal::OlderEpoch => return,
            Refusal::NewerEpoch => {
                // The tag is outside the signature, so anyone who can write
                // rows can name any epoch: never a reason to wait. If a key
                // does arrive, the cursor starts over under it.
                self.note_newer_epoch();
                stats.skipped_undecryptable += 1;
                "passing a row sealed under a key this device does not hold"
            }
            Refusal::Future => {
                stats.skipped_future += 1;
                "skipping a row stamped implausibly far ahead of when the backend took it"
            }
            Refusal::Forged => {
                stats.skipped_forged += 1;
                "refusing a row whose metadata is unsigned or wrongly signed"
            }
            Refusal::TooLarge => {
                stats.skipped_too_large += 1;
                "skipping a row that exceeds the content cap"
            }
            // Never a delete, never a partial row, never a log line containing
            // the payload or the key.
            Refusal::Undecryptable => {
                stats.skipped_undecryptable += 1;
                "skipping a row this device cannot decrypt"
            }
        };
        tracing::warn!(item_id = %item_id, "{reason}");
    }

    /// This device uploaded these versions; reading them back would only be
    /// declined.
    pub(super) fn note_uploaded(&self, rows: &[CloudItem]) {
        self.note_handled(rows.iter().map(|row| {
            version_key(
                &row.item_id,
                clamp_stamp(row.created_at),
                row.deleted,
                &row.origin_device_id,
            )
        }));
    }

    fn note_handled(&self, keys: impl IntoIterator<Item = Key>) {
        let mut handled = self.lock_handled();
        for key in keys {
            handled.insert(key);
        }
    }

    fn lock_handled(&self) -> MutexGuard<'_, Handled> {
        self.handled.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// The next rows to fetch: the first listed ones not yet dealt with, as many
/// as one request may name.
fn next_ids(entries: &[Entry]) -> Vec<String> {
    let mut bytes = 0usize;
    entries
        .iter()
        .filter(|entry| !entry.done)
        .map(|entry| &entry.listing.item_id)
        .take(FETCH_ROWS)
        .take_while(|item_id| {
            bytes += item_id.len() + 1;
            bytes <= FETCH_ID_BYTES
        })
        .cloned()
        .collect()
}

/// What can be told of a row without opening it.
///
/// The stamp is the writer's and the position is the backend's, so a version
/// is judged against the moment the backend took it, never against this
/// device's clock.
fn refusal(epoch: Option<u64>, created_at: i64, position: i64, held: u64) -> Option<Refusal> {
    match epoch {
        None => return Some(Refusal::Forged),
        Some(epoch) if epoch > held => return Some(Refusal::NewerEpoch),
        Some(epoch) if epoch < held => return Some(Refusal::OlderEpoch),
        Some(_) => {}
    }
    (created_at > (position / 1_000).saturating_add(MAX_FUTURE_SKEW_MS)).then_some(Refusal::Future)
}

/// Verify and open a fetched row, or say why not.
fn open_row(
    row: CloudItem,
    position: i64,
    too_long: bool,
    row_key: &RowKey,
) -> Result<LocalItem, Refusal> {
    if let Some(refusal) = refusal(row.key_epoch(), row.created_at, position, row_key.epoch) {
        return Err(refusal);
    }
    // The transport dropped a string longer than any accepted payload unread,
    // so neither signature nor size can be checked on it. Only a client
    // ignoring the cap writes such a row.
    if too_long {
        return Err(Refusal::TooLarge);
    }
    // Before the payload is touched, and before anything reaches the
    // comparator: a row whose metadata was not signed by a holder of the sync
    // key is not a version of anything. The backend cannot produce this
    // signature, so this is what stops an account-password holder from
    // stamping a competing version that outranks the real one — or a tombstone
    // that deletes it everywhere (manifest 05 §5.3).
    if row.verify_epoch(&row_key.key).is_err() {
        return Err(Refusal::Forged);
    }
    // The signature authenticates the base64 spelling, so this length check
    // cannot let an unsigned row choose a refusal class. It avoids decoding or
    // opening ciphertext that cannot possibly contain a product-sized
    // plaintext.
    if !row.deleted && row.ciphertext.len() > MAX_PAYLOAD_B64_BYTES {
        return Err(Refusal::TooLarge);
    }
    let content = if row.deleted {
        Zeroizing::new(Vec::new())
    } else {
        decrypt_row(&row.ciphertext, &row.nonce, &row_key.key, &row.item_id)
            .map_err(|_| Refusal::Undecryptable)?
    };
    if !row.deleted && super::too_large_to_sync(&row.content_type, content.len()) {
        return Err(Refusal::TooLarge);
    }
    Ok(LocalItem {
        item_id: row.item_id,
        content,
        content_type: row.content_type,
        payload_metadata: row.payload_metadata,
        source_app_bundle_id: row.source_app_bundle_id,
        source_app_name: row.source_app_name,
        created_at: row.created_at,
        deleted: row.deleted,
        origin_device_id: row.origin_device_id,
    })
}

/// Merge decrypted rows. Returns whether a declined row's local winner was
/// queued for upload.
fn apply_batch(
    source: &dyn CloudSource,
    batch: Vec<LocalItem>,
    stats: &mut SyncStats,
) -> Result<bool, SyncError> {
    if batch.is_empty() {
        return Ok(false);
    }
    let offered = batch.len();
    let outcomes = source.apply_remote_batch(batch)?;
    if outcomes.len() != offered {
        return Err(SyncError::Source(MSG_BATCH_ARITY));
    }
    let mut republish = false;
    for outcome in outcomes {
        match outcome {
            Applied::Merged => stats.applied += 1,
            Applied::Declined(declined) => {
                republish |= source.requeue_local_winner(&declined)?;
            }
        }
    }
    Ok(republish)
}

/// Where a read has reached, as a keyset over `(position, item_id)`.
///
/// The pair, not the position: every row of one upsert shares a position, so a
/// cursor that carries only that cannot be advanced through a batch larger
/// than a page (INV-N1, manifest 05 §5.1 row 6). `item_id` is `None` only
/// where a read starts inclusively, which is the `gte` case in
/// [`RestApi::list_since`](super::RestApi::list_since).
#[derive(Clone, PartialEq, Eq)]
struct Cursor {
    position: i64,
    item_id: Option<String>,
}

impl Cursor {
    /// Move onto a row, in the order the server listed it. Returns whether it
    /// moved.
    ///
    /// Rows are taken in the server's order because its keyset filter compares
    /// `item_id` by the database collation, which byte order need not match.
    /// Guarded on the position only, which both sides compare the same way:
    /// the cursor never moves back (INV-N5).
    fn pass(&mut self, position: i64, item_id: &str) -> bool {
        let forward = position >= self.position;
        if forward {
            self.position = position;
            self.item_id = Some(item_id.to_owned());
        }
        forward
    }
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
    use crate::auth::now_ms;
    use crate::crypto::encrypt_row;

    /// The row the download cursor rests on.
    fn reached(source: &FakeSource) -> Option<String> {
        source.pull_cursor().unwrap().item_id
    }

    fn signed_boundary_row(
        id: &str,
        created_at: i64,
        content_type: &str,
        content: &[u8],
        payload_metadata: Option<&str>,
    ) -> CloudItem {
        let (nonce, ciphertext) = encrypt_row(content, &super::super::fakes::key(), id).unwrap();
        signed(CloudItem {
            item_id: id.into(),
            ciphertext,
            nonce,
            content_type: content_type.into(),
            payload_metadata: payload_metadata.map(str::to_owned),
            source_app_bundle_id: None,
            source_app_name: None,
            created_at,
            deleted: false,
            origin_device_id: "device-b".into(),
            signature: String::new(),
            position: None,
        })
    }

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
        assert_eq!(reached(&source).as_deref(), Some("b"));
    }

    #[tokio::test]
    async fn a_page_is_merged_through_one_batch_call_that_answers_for_every_row() {
        let rows: Vec<CloudItem> = (0..FETCH_ROWS + 30)
            .map(|i| cloud_row(&format!("item-{i:04}"), 1_000 + i as i64, "x"))
            .collect();
        let source = FakeSource::default();
        let sync = driver(FakeRest::seeded(rows), FakeAuth::default());

        let stats = sync.pull(&source).await.unwrap();

        assert_eq!(stats.applied, FETCH_ROWS + 30);
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
            reached(&source),
            None,
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
        // The second round lists the row again and knows it: no payload
        // travels twice.
        assert_eq!((second.downloaded, second.applied), (0, 0));
        assert_eq!(source.get("a").unwrap().content.as_slice(), b"first");
        assert_eq!(reached(&source).as_deref(), Some("a"));
    }

    #[tokio::test]
    async fn a_self_echoed_row_changes_nothing() {
        // INV-I2: a device both writes to and reads from the same account, so
        // it is offered its own writes back. It remembers what it uploaded, so
        // the payload does not travel; where it has forgotten, the ordering
        // absorbs the echo.
        let source = FakeSource::with_local(item("a", 1_000, "mine"));
        let sync = driver(FakeRest::default(), FakeAuth::default());

        let round = sync.sync(&source).await.unwrap();
        assert_eq!(
            (round.uploaded, round.downloaded),
            (1, 0),
            "this device downloaded its own upload"
        );
        let after_first = source.get("a");

        source.rewind();
        let echoed = sync.pull(&source).await.unwrap();
        assert_eq!((echoed.downloaded, echoed.applied), (1, 0));
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
        sync.rest.write(vec![cloud_tombstone("a", 2_000)]);
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
            .write(vec![cloud_row("a", 1_000, "back from the dead")]);
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
                source_app_bundle_id: None,
                source_app_name: None,
                created_at: 2_000,
                deleted: false,
                origin_device_id: "device-b".into(),
                signature: String::new(),
                position: None,
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
        assert_eq!(reached(&source).as_deref(), Some("b"));
    }

    #[tokio::test]
    async fn signed_size_boundaries_apply_or_skip_without_blocking_progress() {
        // Test inputs, not a second classifier: IPC owns these six shapes.
        for (name, content_type, binary, metadata) in [
            ("text", "text", false, None),
            ("rtf", "text/rtf", false, None),
            ("html", "text/html", false, None),
            ("image", "image/png", true, None),
            (
                "file",
                "file",
                true,
                Some(r#"{"filename":"report.pdf","mime_type":"application/pdf"}"#),
            ),
            ("future", "application/x-future", true, None),
        ] {
            let mut exact = if binary {
                vec![0; copypaste_ipc::MAX_CONTENT_BYTES]
            } else {
                "\0".repeat(copypaste_ipc::MAX_CONTENT_BYTES).into_bytes()
            };
            if binary {
                exact[..3].copy_from_slice(&[0, 0xff, 0x80]);
            }
            let exact_row = signed_boundary_row("exact", 1_000, content_type, &exact, metadata);
            exact_row
                .verify(&super::super::fakes::key())
                .expect("boundary fixture is signed");
            let source = FakeSource::default();
            let sync = driver(FakeRest::seeded(vec![exact_row]), FakeAuth::default());
            let stats = sync.pull(&source).await.unwrap();
            assert_eq!(
                (
                    stats.applied,
                    stats.skipped_too_large,
                    stats.skipped_undecryptable
                ),
                (1, 0, 0),
                "{name}"
            );
            assert_eq!(
                source.get("exact").unwrap().content.as_slice(),
                exact.as_slice(),
                "{name} bytes"
            );

            let old_local = item("oversized", 500, "local history");
            let source = FakeSource::with_local(old_local.clone());
            let mut oversized = exact;
            oversized.push(0);
            let sync = driver(
                FakeRest::seeded(vec![
                    signed_boundary_row("oversized", 1_000, content_type, &oversized, metadata),
                    signed_boundary_row("valid", 2_000, "text", b"still reachable", None),
                ]),
                FakeAuth::default(),
            );
            let stats = sync.pull(&source).await.unwrap();
            assert_eq!(
                (
                    stats.skipped_too_large,
                    stats.applied,
                    stats.skipped_undecryptable
                ),
                (1, 1, 0),
                "{name}"
            );
            assert_eq!(
                source.get("oversized"),
                Some(old_local),
                "{name} refusal changed local row"
            );
            assert_eq!(
                source.get("valid").unwrap().content.as_slice(),
                b"still reachable"
            );
            assert_eq!(
                reached(&source).as_deref(),
                Some("valid"),
                "{name} did not advance past refusal"
            );
        }
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
        source.rewind();

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
        // (the privacy contract), so this one is asserted on its own.
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
        sync.rest.write(vec![forged]);

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

        let rest = FakeRest::seeded(vec![cloud_row("real", 2_000, "genuine"), unsigned]);
        let source = FakeSource::default();
        let sync = driver(rest, FakeAuth::default());

        let stats = sync.pull(&source).await.unwrap();

        assert_eq!(stats.skipped_forged, 1);
        assert_eq!(stats.applied, 1);
        assert!(source.get("forged").is_none());
        assert_eq!(source.get("real").unwrap().content.as_slice(), b"genuine");
        assert_eq!(
            reached(&source).as_deref(),
            Some("forged"),
            "the cursor did not move past a refused row, so it can be stalled"
        );
        // And the refused row is not re-offered forever.
        assert_eq!(sync.pull(&source).await.unwrap().downloaded, 0);
    }

    #[tokio::test]
    async fn rows_passed_for_want_of_a_key_are_read_again_after_a_restart() {
        // A member that holds no key of epoch 1 yet passes its rows, the
        // process ends, and only then does the key arrive. Nothing in memory
        // remembers the rows; the cursor's own epoch has to.
        let next = crate::crypto::SyncKey::from_bytes([7; 32]);
        let backend = || {
            let (nonce, ciphertext) = encrypt_row(b"for members", &next, "rotated").unwrap();
            let mut row =
                CloudItem::from_sealed_b64("rotated", ciphertext, nonce, "text", 1_000, "device-b");
            row.sign_epoch(&next, 1);
            let rest = FakeRest::seeded(vec![row]);
            // Far enough on that no overlap reaches back to the row.
            rest.clock
                .store(10 * OVERLAP.as_micros() as i64, Ordering::SeqCst);
            rest.write(vec![cloud_row("later", 2_000, "readable")]);
            rest
        };
        let source = FakeSource::default();

        let stats = driver(backend(), FakeAuth::default())
            .pull(&source)
            .await
            .unwrap();
        assert_eq!((stats.skipped_undecryptable, stats.applied), (1, 1));
        assert_eq!(reached(&source).as_deref(), Some("later"));

        source.adopt_space_key(1, &next);
        let sync = driver(backend(), FakeAuth::default());
        // The fake has no envelope table, so the first load only finds that
        // out; the second reads the application's key.
        sync.load_space(&source).await.unwrap();
        sync.load_space(&source).await.unwrap();

        assert_eq!(sync.pull(&source).await.unwrap().applied, 1);
        assert_eq!(
            source.get("rotated").unwrap().content.as_slice(),
            b"for members"
        );
    }

    #[tokio::test]
    async fn a_cursor_of_another_account_is_no_cursor() {
        let source = FakeSource::default();
        source.plant_cursor(PullCursor {
            position: i64::MAX / 2,
            item_id: Some("theirs".into()),
            account: "another-account".into(),
            epoch: 0,
        });
        let sync = driver(
            FakeRest::seeded(vec![cloud_row("mine", 1_000, "below their position")]),
            FakeAuth::default(),
        );

        assert_eq!(sync.pull(&source).await.unwrap().applied, 1);
        let cursor = source.pull_cursor().unwrap();
        assert_eq!(
            (cursor.account.as_str(), cursor.item_id.as_deref()),
            ("user-1", Some("mine"))
        );
    }

    #[tokio::test]
    async fn a_device_whose_clock_is_far_behind_still_reads_and_passes() {
        // To a device ten days slow, every honest row is placed and stamped
        // ten days ahead. The backend's clock is the one that counts.
        let ahead = now_ms() + MAX_FUTURE_SKEW_MS * 10;
        let rest = FakeRest::default();
        rest.clock.store(ahead * 1_000, Ordering::SeqCst);
        rest.write(vec![cloud_row("honest", ahead, "written now")]);
        let source = FakeSource::default();
        let sync = driver(rest, FakeAuth::default());

        let stats = sync.pull(&source).await.unwrap();
        assert_eq!((stats.applied, stats.skipped_future), (1, 0));
        assert_eq!(reached(&source).as_deref(), Some("honest"));
        assert_eq!(sync.pull(&source).await.unwrap().downloaded, 0);
    }

    #[tokio::test]
    async fn a_row_stamped_far_in_the_future_is_refused_without_stalling_the_cursor() {
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
        // The stamp is the writer's; the position is not, so passing the row
        // skips nothing. And the listing was enough to refuse it.
        assert_eq!(reached(&source).as_deref(), Some("hostile"));
        assert_eq!(stats.downloaded, 1);
    }

    #[tokio::test]
    async fn the_cursor_follows_the_server_collation_across_a_cut() {
        let rows = ["a", "B", "C", "D"].map(|id| cloud_row(id, 1_000, "large"));
        let source = FakeSource::default();
        let mut rest = FakeRest::default();
        rest.write(rows.to_vec());
        rest.cut_after = Some(2);
        rest.collated = true;
        let sync = driver(rest, FakeAuth::default());

        let stats = sync.pull(&source).await.unwrap();

        assert_eq!(stats.applied, 4);
        assert_eq!(
            reached(&source).as_deref(),
            Some("D"),
            "a byte-order cursor stays at `a` and stalls after `B`"
        );
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
    async fn pull_drains_more_rows_than_one_page() {
        // AT-23 in miniature: more rows than a page, all fetched.
        let rows: Vec<CloudItem> = (0..LIST_PAGE_LIMIT as usize + 30)
            .map(|i| cloud_row(&format!("item-{i:04}"), 1_000 + i as i64, "x"))
            .collect();
        let source = FakeSource::default();
        let sync = driver(FakeRest::seeded(rows), FakeAuth::default());

        let stats = sync.pull(&source).await.unwrap();

        assert_eq!(stats.applied, LIST_PAGE_LIMIT as usize + 30);
        assert_eq!(
            sync.rest.lists.load(Ordering::SeqCst),
            2,
            "a full page did not trigger a burst drain"
        );
    }

    /// AT-24 / INV-N1: every row of one write shares a position, so a write
    /// larger than a page can only be paged through with the tie-break.
    #[tokio::test]
    async fn a_write_holding_more_than_one_page_still_drains() {
        let burst = LIST_PAGE_LIMIT as usize * 2 + 7;
        let rest = FakeRest::default();
        rest.write(
            (0..burst)
                .map(|i| cloud_row(&format!("item-{i:04}"), 1_000, "bulk import"))
                .collect(),
        );
        let source = FakeSource::default();
        let sync = driver(rest, FakeAuth::default());

        let stats = sync.pull(&source).await.unwrap();

        assert_eq!(stats.applied, burst, "rows behind the boundary position");
        assert_eq!(reached(&source), Some(format!("item-{:04}", burst - 1)));
    }

    #[tokio::test]
    async fn large_rows_are_merged_in_bounded_batches_from_cut_pages() {
        let half = "x".repeat(APPLY_BATCH_BYTES / 2);
        let rows: Vec<CloudItem> = (0..6)
            .map(|i| cloud_row(&format!("item-{i}"), 1_000 + i, &half))
            .collect();

        let source = FakeSource::default();
        let sync = driver(FakeRest::seeded(rows.clone()), FakeAuth::default());
        let stats = sync.pull(&source).await.unwrap();
        assert_eq!(stats.applied, 6);
        assert_eq!(
            source.batches.load(Ordering::SeqCst),
            3,
            "two rows fill a batch"
        );
        assert_eq!(reached(&source).as_deref(), Some("item-5"));

        let source = FakeSource::default();
        let mut rest = FakeRest::seeded(rows);
        rest.cut_after = Some(2);
        let sync = driver(rest, FakeAuth::default());
        let stats = sync.pull(&source).await.unwrap();
        assert_eq!(stats.applied, 6, "a cut page is not the end of the backlog");
        assert_eq!(sync.rest.fetches.load(Ordering::SeqCst), 3);
        assert_eq!(reached(&source).as_deref(), Some("item-5"));
    }

    #[tokio::test]
    async fn a_pull_stops_after_its_request_budget_and_the_next_one_resumes() {
        let rows: Vec<CloudItem> = (0..MAX_REQUESTS_PER_PULL as i64 + 10)
            .map(|i| cloud_row(&format!("item-{i:03}"), 1_000 + i, "x"))
            .collect();
        let source = FakeSource::default();
        let mut rest = FakeRest::seeded(rows);
        rest.cut_after = Some(1);
        let sync = driver(rest, FakeAuth::default());

        // One listing, then one row per fetch.
        assert_eq!(
            sync.pull(&source).await.unwrap().applied,
            MAX_REQUESTS_PER_PULL - 1
        );
        assert_eq!(
            sync.rest.lists.load(Ordering::SeqCst) + sync.rest.fetches.load(Ordering::SeqCst),
            MAX_REQUESTS_PER_PULL
        );
        assert_eq!(sync.pull(&source).await.unwrap().applied, 11);
        assert_eq!(
            reached(&source),
            Some(format!("item-{:03}", MAX_REQUESTS_PER_PULL + 9))
        );
    }

    #[tokio::test]
    async fn a_cut_fetch_resumes_and_a_row_that_vanished_is_passed() {
        let rows: Vec<CloudItem> = (1..=4)
            .map(|i| cloud_row(&format!("row-{i}"), 1_000 + i, "x"))
            .collect();
        let source = FakeSource::default();
        let mut rest = FakeRest::seeded(rows);
        rest.cut_after = Some(1);
        // Listed, and deleted before it is fetched.
        rest.vanished = vec!["row-2".into()];
        let sync = driver(rest, FakeAuth::default());

        let stats = sync.pull(&source).await.unwrap();

        assert_eq!(stats.applied, 3, "a cut page lost a row");
        assert!(source.get("row-2").is_none());
        assert_eq!(reached(&source).as_deref(), Some("row-4"));
        assert_eq!(
            sync.rest.fetches.load(Ordering::SeqCst),
            3,
            "the missing row was asked for again and again"
        );
    }

    #[tokio::test]
    async fn an_oversized_row_is_refused_without_stalling_the_cursor() {
        let source = FakeSource::default();
        let mut rest = FakeRest::seeded(vec![
            cloud_row("huge", 1_000, "dropped by the transport"),
            cloud_row("real", 2_000, "genuine"),
        ]);
        rest.oversized = vec!["huge".into()];
        let sync = driver(rest, FakeAuth::default());

        let stats = sync.pull(&source).await.unwrap();

        assert_eq!((stats.skipped_too_large, stats.applied), (1, 1));
        assert!(source.get("huge").is_none());
        assert_eq!(reached(&source).as_deref(), Some("real"));
    }

    #[tokio::test]
    async fn a_row_uploaded_late_with_an_older_stamp_is_still_delivered() {
        // A device that was offline uploads what it copied hours ago. Its
        // stamp is older than rows every other device has already passed; the
        // position the backend gives it is not.
        let source = FakeSource::default();
        let sync = driver(
            FakeRest::seeded(vec![cloud_row("recent", 9_000, "copied just now")]),
            FakeAuth::default(),
        );
        assert_eq!(sync.pull(&source).await.unwrap().applied, 1);

        sync.rest
            .write(vec![cloud_row("offline", 1_000, "copied hours ago")]);
        let stats = sync.pull(&source).await.unwrap();

        assert_eq!(stats.applied, 1, "a row stamped behind the cursor was lost");
        assert_eq!(
            source.get("offline").unwrap().content.as_slice(),
            b"copied hours ago"
        );
    }

    #[tokio::test]
    async fn a_row_committed_late_inside_the_overlap_is_delivered() {
        let source = FakeSource::default();
        let sync = driver(
            FakeRest::seeded(vec![
                cloud_row("first", 1_000, "committed at once"),
                cloud_row("second", 2_000, "began later, committed sooner"),
            ]),
            FakeAuth::default(),
        );
        assert_eq!(sync.pull(&source).await.unwrap().applied, 2);

        // A write that began between the two and commits only now: its
        // position is behind the cursor.
        let slow = CloudItem {
            position: Some(15),
            ..cloud_row("slow", 1_500, "took its time")
        };
        sync.rest.rows.lock().unwrap().insert("slow".into(), slow);

        let stats = sync.pull(&source).await.unwrap();
        assert_eq!(
            (stats.downloaded, stats.applied),
            (1, 1),
            "only the row behind the cursor is fetched"
        );
    }

    #[tokio::test]
    async fn the_persisted_cursor_never_moves_back() {
        // Rows surface behind a cursor that is already ahead of them. They are
        // read; the cursor does not return to them.
        let source = FakeSource::default();
        source.plant_cursor(PullCursor {
            position: 25,
            item_id: Some("zz".into()),
            account: "user-1".into(),
            epoch: 0,
        });
        let sync = driver(
            FakeRest::seeded(vec![
                cloud_row("behind-1", 1_000, "at 10"),
                cloud_row("behind-2", 2_000, "at 20"),
            ]),
            FakeAuth::default(),
        );

        assert_eq!(sync.pull(&source).await.unwrap().applied, 2);
        let cursor = source.pull_cursor().unwrap();
        assert_eq!(
            (cursor.position, cursor.item_id.as_deref()),
            (25, Some("zz"))
        );

        sync.rest.write(vec![cloud_row("ahead", 3_000, "at 30")]);
        sync.pull(&source).await.unwrap();
        assert_eq!(reached(&source).as_deref(), Some("ahead"));
    }

    #[tokio::test]
    async fn a_version_uploaded_again_is_not_downloaded_again() {
        // Several devices forward the same version, each sealing it afresh:
        // another nonce, another signature, another position, one version.
        let source = FakeSource::default();
        let sync = driver(
            FakeRest::seeded(vec![cloud_row("a", 1_000, "one megabyte, say")]),
            FakeAuth::default(),
        );
        assert_eq!(sync.pull(&source).await.unwrap().downloaded, 1);

        sync.rest
            .write(vec![cloud_row("a", 1_000, "one megabyte, say")]);
        let again = sync.pull(&source).await.unwrap();

        assert_eq!(again.downloaded, 0);
        assert_eq!(reached(&source).as_deref(), Some("a"));
        assert_eq!(source.pull_cursor().unwrap().position, 20);
    }
}
