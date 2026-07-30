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
//! For the same reason there is no tombstone special case in this file beyond
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

use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use backoff::backoff::Backoff;
use backoff::ExponentialBackoff;

use crate::auth::Session;
use crate::crypto::{decrypt_row, encrypt_row, SyncKey};
use crate::rest::CloudItem;
use crate::CloudConfig;

// ---------------------------------------------------------------------------
// Tunables
// ---------------------------------------------------------------------------

/// Rows requested per page.
///
/// Performance only: correctness comes from the keyset cursor, not from the
/// page size. Larger pages mean fewer round trips on a catch-up; smaller ones
/// mean a shorter tail if the connection drops mid-drain.
const PULL_PAGE_LIMIT: usize = 100;

/// Pages one [`CloudSync::pull`] will drain before returning.
///
/// A device that has been offline for a month has thousands of rows waiting.
/// Draining them in one call would make a single `pull` unbounded in time and
/// memory; stopping after this many pages returns control to the caller, which
/// simply polls again — the watermark has advanced, so the next call resumes
/// exactly where this one stopped.
const MAX_PAGES_PER_PULL: usize = 50;

/// Rows per upsert request. Bounds request size without needing to measure it.
const UPLOAD_BATCH: usize = 50;

/// Attempts for a transient (5xx / network) failure, including the first.
/// Manifest 05 §4.6.3.
const MAX_TRANSIENT_ATTEMPTS: u32 = 4;

/// Transient-retry schedule floor and ceiling. Manifest 05 §4.6.3.
const TRANSIENT_INITIAL: Duration = Duration::from_secs(1);
const TRANSIENT_MAX: Duration = Duration::from_secs(30);

/// Ceiling applied to a server-supplied `Retry-After`.
///
/// The header is honoured, but a server that asks for an hour does not get to
/// pin the loop for an hour; manifest 05 §4.6.3 clamps it to the maximum
/// backoff. The retry is single-shot regardless, so a server stuck on 429
/// surfaces an error rather than blocking forever.
const MAX_RETRY_AFTER: Duration = TRANSIENT_MAX;

/// Idle poll interval floor: the cadence right after something changed.
pub const MIN_POLL_INTERVAL: Duration = Duration::from_secs(5);

/// Idle poll interval ceiling.
///
/// A device whose clipboard nobody has touched should not wake the radio every
/// few seconds — on a phone that is measurable battery, and it is the reason
/// v1's cadence grew from five seconds toward five minutes rather than staying
/// fixed. Five minutes is the ceiling because [`crate::realtime`] is the thing
/// that makes latency short when anything is actually happening; the poll is
/// the correctness backstop, and a backstop can be slow.
pub const MAX_POLL_INTERVAL: Duration = Duration::from_secs(300);

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
/// No correction is attempted, only refusal (manifest 05 R-CLK-2). The same
/// rule and the same value exist in `copypaste-p2p`'s merge; the two crates
/// have no dependency edge, and adding one for a single `i64` would be a worse
/// trade than restating it. If a third transport appears, this constant should
/// move to `copypaste-core` and both should import it.
const MAX_FUTURE_SKEW_MS: i64 = 24 * 60 * 60 * 1000;

// ---------------------------------------------------------------------------
// The local side
// ---------------------------------------------------------------------------

/// One version of one item, in the plain.
///
/// This is the shape either side of the encryption boundary: what the daemon
/// hands up to be sealed, and what comes back down after opening. `content` is
/// plaintext here and nowhere else — the moment it leaves this process it is
/// ciphertext.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalItem {
    /// Cross-device logical identity. Bound into the AEAD's associated data,
    /// and the conflict target of the upsert. Never a local row primary key
    /// (manifest 05 R-ID-1).
    pub item_id: String,
    /// Plaintext. Empty for a tombstone — see [`LocalItem::deleted`].
    pub content: Vec<u8>,
    pub content_type: String,
    /// Version stamp, Unix milliseconds.
    pub created_at: i64,
    /// A tombstone. Carried on the wire and persisted on the receiver
    /// (manifest 05 T-2); a tombstone never carries ciphertext (T-4).
    pub deleted: bool,
    /// The device that produced *this version*. Preserved across hops — never
    /// restamped with the forwarding device, or the ordering's final tie-break
    /// stops being stable.
    pub origin_device_id: String,
}

/// The daemon's history, as this driver needs to see it.
///
/// Implementations live over the store. The driver never touches SQLite, and
/// the store never touches the network.
pub trait CloudSource: Send + Sync {
    /// This device's stable id. Stamped onto rows this device uploads.
    fn device_id(&self) -> String;

    /// Local versions with `created_at >= since_ms`.
    ///
    /// Inclusive on purpose: re-offering the boundary row costs one idempotent
    /// upsert, and excluding it loses every row that shares the boundary
    /// millisecond.
    ///
    /// **Sensitive items must not appear here.** The driver checks again
    /// ([`SensitiveGuard`]) because this is data leaving the user's machine and
    /// one enforcement point is one bug away from none — but the filter belongs
    /// here too, where the detector already ran at capture time.
    ///
    /// # Errors
    ///
    /// [`SyncError::Source`] for a store failure.
    fn local_changes_since(&self, since_ms: i64) -> Result<Vec<LocalItem>, SyncError>;

    /// Merge one remote version into the local history, returning whether
    /// anything changed.
    ///
    /// The implementor owns the ordering decision, and it must be the *same*
    /// comparator the P2P transport uses (manifest 05 INV-C2). Three
    /// requirements come with that:
    ///
    /// * **Equal on every ordering key ⇒ keep local, report `false`.** This is
    ///   what makes replay and self-echo free (INV-I1, INV-I2).
    /// * **A tombstone for an unknown `item_id` must still be persisted** as a
    ///   tombstone row. Dropping it lets a later-arriving create resurrect the
    ///   item (T-3, `CopyPaste-bfiu`).
    /// * **Preserve the local row primary key** on a replace, or the full-text
    ///   index and pins that reference it are orphaned (R-ID-2).
    ///
    /// # Errors
    ///
    /// [`SyncError::Source`] for a store failure. A version this device
    /// declines to take is `Ok(false)`, not an error.
    fn apply_remote(&self, item: LocalItem) -> Result<bool, SyncError>;

    /// The reconciliation cursor, in Unix milliseconds. Zero on a first run.
    ///
    /// # Errors
    ///
    /// [`SyncError::Source`] for a store failure.
    fn watermark(&self) -> Result<i64, SyncError>;

    /// Persist the cursor.
    ///
    /// Must be stored independently of the item rows so that pruning history to
    /// a storage cap cannot move it backwards (INV-N5), and should be written
    /// in the same transaction as the rows it covers so that a crash cannot
    /// lose it. Losing it costs re-pagination, not data.
    ///
    /// # Errors
    ///
    /// [`SyncError::Source`] for a store failure.
    fn set_watermark(&self, ms: i64) -> Result<(), SyncError>;
}

// ---------------------------------------------------------------------------
// The sensitive-content gate
// ---------------------------------------------------------------------------

/// The last check before an item leaves the machine.
///
/// `CloudSource` filters sensitive items already — the detector ran at capture
/// time and the store knows the answer. This is the second layer, and it exists
/// because manifest 05 AT-56 (`CopyPaste-20yw`) records that v1 had exactly one
/// enforcement point and it had a hole: the backlog sweep took a different path
/// to the same table and did not consult it.
///
/// It is required by [`CloudSync::new`] rather than defaulted, so there is no
/// way to construct a driver that uploads unchecked. It is a callback rather
/// than a detector because the detector already exists — `copypaste-core`'s
/// `sensitive::Detector`, with the full ruleset, the confidence model and the
/// false-positive defences of manifest 07. Re-implementing even a "quick check"
/// here would be a second regex engine, which is one of the duplications
/// `CLAUDE.md` rule 1 was written about. The daemon wires it:
///
/// ```ignore
/// let detector = copypaste_core::sensitive::Detector::new()?;
/// let guard = SensitiveGuard::new(move |item| {
///     std::str::from_utf8(&item.content)
///         .map(|text| detector.is_sensitive(text))
///         // Not decodable as text: not something the ruleset can judge, and
///         // not something to guess about. Let it through; the store's own
///         // capture-time flag is the first layer.
///         .unwrap_or(false)
/// });
/// ```
#[derive(Clone)]
pub struct SensitiveGuard(Arc<dyn Fn(&LocalItem) -> bool + Send + Sync>);

impl SensitiveGuard {
    /// Wrap a predicate. `true` means "never upload this".
    pub fn new(f: impl Fn(&LocalItem) -> bool + Send + Sync + 'static) -> Self {
        Self(Arc::new(f))
    }

    /// Would this item be withheld?
    pub fn is_sensitive(&self, item: &LocalItem) -> bool {
        (self.0)(item)
    }
}

impl std::fmt::Debug for SensitiveGuard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SensitiveGuard").finish_non_exhaustive()
    }
}

// ---------------------------------------------------------------------------
// The transport seam
// ---------------------------------------------------------------------------

/// What the driver needs to know about a failed request.
///
/// The REST and auth layers classify their own HTTP statuses; the driver acts
/// on the classification. Keeping the two apart is what lets this module be
/// tested against a fake transport with no HTTP anywhere, and it is the reason
/// the recovery rules below are asserted rather than assumed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportFault {
    /// 401. The bearer is stale or wrong.
    Unauthorized,
    /// 429, with the server's `Retry-After` if it sent one.
    ///
    /// Delta-seconds only. The HTTP-date form is deliberately unsupported:
    /// Supabase emits integer seconds, and a date parser buys nothing but a
    /// dependency and a clock-skew bug (manifest 05 §4.6.3).
    RateLimited { retry_after: Option<Duration> },
    /// 5xx, a timeout, or a socket error. Worth retrying.
    Transient(&'static str),
    /// Any other 4xx, or a malformed response. Retrying will not help.
    Permanent(&'static str),
}

/// The REST surface this driver uses.
///
/// `SupabaseRest` in `rest.rs` is the production implementation. The adapter
/// between its error type and [`TransportFault`] belongs wherever `RestError`'s
/// variants are visible — it is a `match` with one arm per status class:
///
/// ```ignore
/// impl RestApi for SupabaseRest {
///     async fn fetch_since(&self, token: &str, since_ms: i64, limit: usize)
///         -> Result<Vec<CloudItem>, TransportFault>
///     {
///         SupabaseRest::fetch_since(self, token, since_ms, limit)
///             .await
///             .map_err(classify)
///     }
///     // …upsert, tombstone the same way.
/// }
/// ```
///
/// It is not written here because this module must not depend on the shape of a
/// sibling's error enum; a seam that costs ten lines at the wiring point buys
/// two files that can be written and changed independently.
pub trait RestApi: Send + Sync {
    /// Rows with `created_at >= since_ms`, oldest first, at most `limit` of
    /// them.
    ///
    /// Inclusive, for the reason on [`CloudSource::local_changes_since`]. The
    /// ordering must be total and tie-free — `(created_at, item_id)`, not
    /// `created_at` alone — or a burst sharing one millisecond is silently lost
    /// (manifest 05 INV-N1).
    fn fetch_since(
        &self,
        token: &str,
        since_ms: i64,
        limit: usize,
    ) -> impl Future<Output = Result<Vec<CloudItem>, TransportFault>> + Send;

    /// Upsert on `item_id`, so re-sending a row is a no-op rather than a
    /// conflict. Every column that has a server-side default must be sent
    /// explicitly, `deleted` above all: omitting it lets the column default
    /// fire and resurrects a tombstoned item (manifest 05 T-5,
    /// `CopyPaste-kgs7`).
    fn upsert(
        &self,
        token: &str,
        items: &[CloudItem],
    ) -> impl Future<Output = Result<(), TransportFault>> + Send;

    /// Mark rows deleted. A *tombstone*, not a row removal, and it must clear
    /// the ciphertext rather than leave a stale one behind (manifest 05 T-4).
    fn tombstone(
        &self,
        token: &str,
        item_ids: &[String],
    ) -> impl Future<Output = Result<(), TransportFault>> + Send;
}

/// Why a token refresh failed.
///
/// The two credential cases are kept apart because their recovery differs
/// completely, and GoTrue's error *body* cannot tell them apart — it returns
/// `invalid_grant` for both. The grant kind is what disambiguates, which is
/// `auth.rs`'s job (manifest 05 §4.6.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthFault {
    /// The stored password is wrong. A human has to fix this.
    InvalidCredentials,
    /// The refresh token expired or was revoked. A full sign-in can recover.
    SessionExpired,
    /// GoTrue is throttling.
    RateLimited { retry_after: Option<Duration> },
    /// Network or 5xx.
    Unavailable(&'static str),
}

/// The auth surface this driver uses. See [`RestApi`] for why it is a trait.
pub trait AuthApi: Send + Sync {
    /// Exchange a refresh token for a new session.
    ///
    /// The rotated refresh token in the returned [`Session`] must be kept —
    /// GoTrue rotates it on every refresh, and reusing the old one fails.
    fn refresh(
        &self,
        refresh_token: &str,
    ) -> impl Future<Output = Result<Session, AuthFault>> + Send;
}

// ---------------------------------------------------------------------------
// Results and errors
// ---------------------------------------------------------------------------

/// What one push, pull or sync did.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SyncStats {
    /// Live rows sealed and upserted.
    pub uploaded: usize,
    /// Tombstones propagated.
    pub tombstoned: usize,
    /// Rows fetched, including ones that were skipped.
    pub downloaded: usize,
    /// Rows the store actually took.
    pub applied: usize,
    /// Local items withheld by the [`SensitiveGuard`].
    pub skipped_sensitive: usize,
    /// Remote rows that would not open. Never treated as a delete (INV-N3).
    pub skipped_undecryptable: usize,
    /// Remote rows stamped implausibly far in the future.
    pub skipped_future: usize,
}

impl SyncStats {
    /// Did this round change anything? Drives the idle backoff.
    pub fn changed(&self) -> bool {
        self.uploaded > 0 || self.tombstoned > 0 || self.applied > 0
    }

    fn merge(self, other: Self) -> Self {
        Self {
            uploaded: self.uploaded + other.uploaded,
            tombstoned: self.tombstoned + other.tombstoned,
            downloaded: self.downloaded + other.downloaded,
            applied: self.applied + other.applied,
            skipped_sensitive: self.skipped_sensitive + other.skipped_sensitive,
            skipped_undecryptable: self.skipped_undecryptable + other.skipped_undecryptable,
            skipped_future: self.skipped_future + other.skipped_future,
        }
    }
}

/// Sync failures.
///
/// Every payload is a `&'static str`, so no variant can carry a filesystem
/// path, an access token, a refresh token, a passphrase or row content. That is
/// `CLAUDE.md` rule 4 enforced by the type rather than by review. Implementors
/// of [`CloudSource`] must hold to the same rule when they build
/// [`SyncError::Source`].
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SyncError {
    /// The store failed.
    #[error("local store error: {0}")]
    Source(&'static str),

    /// A row could not be sealed for upload. Never a reason to upload it in the
    /// clear.
    #[error("could not encrypt an item for upload")]
    Encrypt,

    /// The bearer was rejected, a refresh was attempted exactly once, and the
    /// retry was rejected too.
    ///
    /// One refresh, one retry, then stop. A refresh that returns a token which
    /// is itself rejected would otherwise spin forever (manifest 05 AT-36).
    #[error("the backend rejected this session even after a refresh")]
    Unauthorized,

    /// The stored credentials are wrong. Distinct from
    /// [`SyncError::SessionExpired`] because only a human can fix it: prompt,
    /// do not retry, and never fall back to a lower-privilege scope (INV-N6).
    #[error("the stored account credentials were rejected")]
    InvalidCredentials,

    /// The refresh token expired or was revoked. A full sign-in recovers.
    #[error("the session has expired and must be renewed by signing in")]
    SessionExpired,

    /// The backend asked us to slow down, we waited as asked, and it asked
    /// again. Single-shot, so a server stuck on 429 cannot pin the loop.
    #[error("the backend is rate limiting this account")]
    RateLimited,

    /// Network or backend failure that outlived the retry budget.
    #[error("cloud transport error: {0}")]
    Transport(&'static str),
}

// ---------------------------------------------------------------------------
// The driver
// ---------------------------------------------------------------------------

/// Push, pull, and the cadence in between.
///
/// Generic over the two transports so the recovery rules can be tested against
/// fakes with no HTTP in the picture; the production instantiation is
/// `CloudSync<SupabaseRest, SupabaseAuth>`.
pub struct CloudSync<R: RestApi, A: AuthApi> {
    rest: R,
    auth: A,
    key: SyncKey,
    config: CloudConfig,
    /// The live session. Behind a mutex because a 401 on any request rotates it
    /// and every subsequent request must see the new bearer. Never held across
    /// an await.
    session: Mutex<Session>,
    sensitive: SensitiveGuard,
    idle: Mutex<Duration>,
}

impl<R: RestApi, A: AuthApi> std::fmt::Debug for CloudSync<R, A> {
    /// Redacted in full: this type holds a session and a key.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CloudSync").finish_non_exhaustive()
    }
}

impl<R: RestApi, A: AuthApi> CloudSync<R, A> {
    /// Assemble a driver.
    ///
    /// `sensitive` is not optional; see [`SensitiveGuard`].
    pub fn new(
        rest: R,
        auth: A,
        key: SyncKey,
        config: CloudConfig,
        session: Session,
        sensitive: SensitiveGuard,
    ) -> Self {
        Self {
            rest,
            auth,
            key,
            config,
            session: Mutex::new(session),
            sensitive,
            idle: Mutex::new(MIN_POLL_INTERVAL),
        }
    }

    /// The deployment this driver talks to.
    pub fn config(&self) -> &CloudConfig {
        &self.config
    }

    /// A copy of the current session, with whatever token the last refresh
    /// installed. Persist it so a restart does not have to sign in again.
    pub fn session(&self) -> Session
    where
        Session: Clone,
    {
        self.lock_session().clone()
    }

    // -- push --------------------------------------------------------------

    /// Seal every local change since the watermark and upsert it.
    ///
    /// Does not move the watermark: that is [`CloudSync::pull`]'s job, and
    /// moving it here would mean an upload advanced the *download* cursor past
    /// rows another device wrote in the same window.
    ///
    /// Idempotent. Running it twice sends the same rows twice, and the second
    /// send is an upsert onto identical values.
    ///
    /// # Errors
    ///
    /// [`SyncError::Source`] if the store fails, [`SyncError::Encrypt`] if a
    /// row cannot be sealed, and the auth and transport variants per
    /// [`CloudSync::pull`].
    pub async fn push(&self, source: &dyn CloudSource) -> Result<SyncStats, SyncError> {
        let since = source.watermark()?;
        let device_id = source.device_id();
        let mut stats = SyncStats::default();

        let mut live: Vec<CloudItem> = Vec::new();
        let mut dead: Vec<String> = Vec::new();

        for item in source.local_changes_since(since)? {
            // The gate, before anything is sealed or counted. A sensitive item
            // is not merely withheld from this request — it is never given an
            // opportunity to reach the network at all.
            if self.sensitive.is_sensitive(&item) {
                stats.skipped_sensitive += 1;
                tracing::debug!(
                    item_id = %item.item_id,
                    "withholding a sensitive item from upload"
                );
                continue;
            }

            if item.deleted {
                // A tombstone carries no ciphertext, even if the caller handed
                // us a version that still has content in memory (T-4).
                dead.push(item.item_id);
                continue;
            }

            let (nonce, ciphertext) = encrypt_row(&item.content, &self.key, &item.item_id)
                .map_err(|_| SyncError::Encrypt)?;

            live.push(CloudItem {
                item_id: item.item_id,
                ciphertext,
                nonce,
                content_type: item.content_type,
                created_at: item.created_at,
                // Always explicit, never left to the column default (T-5).
                deleted: false,
                origin_device_id: if item.origin_device_id.is_empty() {
                    device_id.clone()
                } else {
                    // Preserve the original origin across hops; restamping it
                    // breaks the ordering's final tie-break.
                    item.origin_device_id
                },
            });
        }

        for batch in live.chunks(UPLOAD_BATCH) {
            self.with_session(|token| self.rest.upsert(token, batch))
                .await?;
            stats.uploaded += batch.len();
        }
        for batch in dead.chunks(UPLOAD_BATCH) {
            self.with_session(|token| self.rest.tombstone(token, batch))
                .await?;
            stats.tombstoned += batch.len();
        }

        Ok(stats)
    }

    // -- pull --------------------------------------------------------------

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
        let mut cursor = source.watermark()?;
        let mut stats = SyncStats::default();

        for _ in 0..MAX_PAGES_PER_PULL {
            let rows = self
                .with_session(|token| self.rest.fetch_since(token, cursor, PULL_PAGE_LIMIT))
                .await?;
            let page_len = rows.len();
            stats.downloaded += page_len;

            let now = now_ms();
            let mut advanced = cursor;

            for row in rows {
                // Lower bound at the decode boundary, not at the use site: a
                // negative stamp cast to unsigned becomes the largest possible
                // value and wins every comparison forever (`CopyPaste-psx7`,
                // manifest 05 R-CLK-1).
                let created_at = row.created_at.max(0);

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

                let content = if row.deleted {
                    // A tombstone has no ciphertext to open. It is still a
                    // version, and it must reach the store even for an item
                    // this device has never seen (T-3).
                    Vec::new()
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
                            advanced = advanced.max(created_at);
                            continue;
                        }
                    }
                };

                let applied = source.apply_remote(LocalItem {
                    item_id: row.item_id,
                    content,
                    content_type: row.content_type,
                    created_at,
                    deleted: row.deleted,
                    origin_device_id: row.origin_device_id,
                })?;
                if applied {
                    stats.applied += 1;
                }
                advanced = advanced.max(created_at);
            }

            // Persist per page, so an interrupted drain resumes from the last
            // completed page rather than from the start. Monotonic by
            // construction: `advanced` starts at the current cursor.
            if advanced > cursor {
                source.set_watermark(advanced)?;
            }

            // Short page: caught up.
            if page_len < PULL_PAGE_LIMIT {
                break;
            }
            // Full page that produced no progress: every row in it was
            // unusable in a way that does not advance the cursor. Stop rather
            // than re-requesting the same window forever (manifest 05 AT-29).
            if advanced == cursor {
                tracing::warn!("a full page of rows produced no cursor progress; pausing the drain");
                break;
            }
            cursor = advanced;
        }

        Ok(stats)
    }

    // -- both --------------------------------------------------------------

    /// Push, then pull, then adjust the idle cadence.
    ///
    /// Push first so that a local delete reaches the backend before this device
    /// asks for rows — otherwise a tombstone written a moment ago can be
    /// shadowed by the live version another device is still serving, and the
    /// user watches a deleted item come back for one tick.
    ///
    /// # Errors
    ///
    /// As [`CloudSync::push`] and [`CloudSync::pull`]. A push failure aborts
    /// before the pull: if the session is dead, the pull would only fail the
    /// same way.
    pub async fn sync(&self, source: &dyn CloudSource) -> Result<SyncStats, SyncError> {
        let pushed = self.push(source).await?;
        let pulled = self.pull(source).await?;
        let stats = pushed.merge(pulled);

        self.note_activity(stats.changed());
        Ok(stats)
    }

    // -- cadence -----------------------------------------------------------

    /// How long to wait before the next [`CloudSync::sync`].
    ///
    /// Starts at [`MIN_POLL_INTERVAL`] and doubles toward
    /// [`MAX_POLL_INTERVAL`] while nothing is happening; any change on either
    /// side resets it to the floor. The poll is a correctness backstop, not the
    /// latency mechanism — [`crate::realtime`] is that — so a slow idle cadence
    /// costs nothing but is worth real battery on a phone.
    pub fn poll_interval(&self) -> Duration {
        *self.lock(&self.idle)
    }

    /// Reset the cadence to the floor.
    ///
    /// Call this when [`crate::realtime`] delivers an event: something is
    /// happening, so the backstop should tighten up rather than wait out
    /// whatever interval it had drifted to.
    pub fn wake(&self) {
        *self.lock(&self.idle) = MIN_POLL_INTERVAL;
    }

    fn note_activity(&self, changed: bool) {
        let mut idle = self.lock(&self.idle);
        *idle = if changed {
            MIN_POLL_INTERVAL
        } else {
            (*idle * 2).min(MAX_POLL_INTERVAL)
        };
    }

    // -- session recovery ---------------------------------------------------

    /// Run one request with the current bearer, recovering from a 401 or a 429
    /// exactly once each.
    ///
    /// The single-shot guards are the point (manifest 05 §4.6.3):
    ///
    /// * **401** — refresh the session, retry once. A second 401 is a hard
    ///   error; without the guard, a refresh that hands back a token the server
    ///   still rejects loops forever.
    /// * **429** — sleep for `Retry-After` (clamped), retry once. A second 429
    ///   is a hard error, so a server stuck on 429 cannot pin the caller.
    /// * **5xx / network** — retry on an exponential schedule, at most
    ///   [`MAX_TRANSIENT_ATTEMPTS`] attempts in total.
    /// * **anything else** — give up. Retrying a 400 does not make it a 200.
    async fn with_session<T, F, Fut>(&self, op: F) -> Result<T, SyncError>
    where
        F: Fn(&str) -> Fut,
        Fut: Future<Output = Result<T, TransportFault>>,
    {
        let mut refreshed = false;
        let mut waited_on_429 = false;
        let mut attempts: u32 = 0;
        let mut transient = transient_policy();

        loop {
            let token = self.lock_session().access_token.clone();
            attempts += 1;

            match op(&token).await {
                Ok(value) => return Ok(value),

                Err(TransportFault::Unauthorized) if !refreshed => {
                    refreshed = true;
                    tracing::debug!("bearer rejected; refreshing the session once");
                    self.refresh_session().await?;
                }
                Err(TransportFault::Unauthorized) => return Err(SyncError::Unauthorized),

                Err(TransportFault::RateLimited { retry_after }) if !waited_on_429 => {
                    waited_on_429 = true;
                    let delay = retry_after.unwrap_or(TRANSIENT_INITIAL).min(MAX_RETRY_AFTER);
                    tracing::debug!(
                        delay_ms = delay.as_millis(),
                        "backend asked us to slow down"
                    );
                    tokio::time::sleep(delay).await;
                }
                Err(TransportFault::RateLimited { .. }) => return Err(SyncError::RateLimited),

                Err(TransportFault::Transient(reason)) => {
                    if attempts >= MAX_TRANSIENT_ATTEMPTS {
                        return Err(SyncError::Transport(reason));
                    }
                    match transient.next_backoff() {
                        Some(delay) => tokio::time::sleep(delay).await,
                        None => return Err(SyncError::Transport(reason)),
                    }
                }

                Err(TransportFault::Permanent(reason)) => {
                    return Err(SyncError::Transport(reason))
                }
            }
        }
    }

    /// Exchange the refresh token for a new session and install it.
    ///
    /// Prefers the refresh grant and never falls back to a password sign-in or
    /// to the anonymous key from inside a request: a silent downgrade to a
    /// lower-privilege scope masks credential rotation, a misconfiguration, or
    /// an attack (INV-N6). If the refresh token itself has aged out, that
    /// surfaces as [`SyncError::SessionExpired`] and the sign-in is the
    /// caller's decision to make.
    async fn refresh_session(&self) -> Result<(), SyncError> {
        let refresh_token = self.lock_session().refresh_token.clone();

        match self.auth.refresh(&refresh_token).await {
            Ok(session) => {
                // Keep the rotated refresh token: GoTrue rotates on every
                // refresh and the old one stops working.
                *self.lock_session() = session;
                Ok(())
            }
            Err(AuthFault::InvalidCredentials) => Err(SyncError::InvalidCredentials),
            Err(AuthFault::SessionExpired) => Err(SyncError::SessionExpired),
            Err(AuthFault::RateLimited { .. }) => Err(SyncError::RateLimited),
            Err(AuthFault::Unavailable(reason)) => Err(SyncError::Transport(reason)),
        }
    }

    // -- lock helpers -------------------------------------------------------
    //
    // A poisoned mutex means some other task panicked while holding it. The
    // data behind it is a session and a duration; neither can be left in a
    // torn state by a panic, and refusing to sync from then on would be a worse
    // outcome than continuing.

    fn lock_session(&self) -> std::sync::MutexGuard<'_, Session> {
        self.session.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn lock<'a, T>(&self, m: &'a Mutex<T>) -> std::sync::MutexGuard<'a, T> {
        m.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// Transient-retry schedule: 1 s doubling to 30 s, with the crate's jitter.
fn transient_policy() -> ExponentialBackoff {
    ExponentialBackoff {
        initial_interval: TRANSIENT_INITIAL,
        max_interval: TRANSIENT_MAX,
        max_elapsed_time: None,
        ..ExponentialBackoff::default()
    }
}

/// Wall clock in Unix milliseconds. A clock before the epoch yields 0 rather
/// than panicking; that only makes the future-skew guard stricter.
fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------
//
// Every test here runs against a fake store and a fake transport. Nothing in
// this file opens a socket.

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicUsize, Ordering};

    const PASS: &str = "correct horse battery staple";
    const ACCOUNT: &str = "acct-1";
    const DEVICE: &str = "device-a";

    fn key() -> SyncKey {
        crate::crypto::derive_sync_key(PASS, ACCOUNT).unwrap()
    }

    fn config() -> CloudConfig {
        CloudConfig {
            url: "https://proj.supabase.invalid".into(),
            anon_key: "anon".into(),
        }
    }

    fn session(access: &str) -> Session {
        Session {
            access_token: access.into(),
            refresh_token: "refresh-1".into(),
            user_id: "user-1".into(),
            expires_at_ms: now_ms() + 3_600_000,
        }
    }

    fn allow_everything() -> SensitiveGuard {
        SensitiveGuard::new(|_| false)
    }

    fn item(id: &str, created_at: i64, content: &str) -> LocalItem {
        LocalItem {
            item_id: id.into(),
            content: content.as_bytes().to_vec(),
            content_type: "text".into(),
            created_at,
            deleted: false,
            origin_device_id: DEVICE.into(),
        }
    }

    fn tombstone(id: &str, created_at: i64) -> LocalItem {
        LocalItem {
            item_id: id.into(),
            content: Vec::new(),
            content_type: "text".into(),
            created_at,
            deleted: true,
            origin_device_id: DEVICE.into(),
        }
    }

    // --- the fake store ---------------------------------------------------

    /// A store with the ordering the real one is required to have: newer
    /// `created_at` wins, an exact tie keeps local. That tie rule is what makes
    /// replay a no-op, so the idempotency tests are only meaningful with it.
    #[derive(Default)]
    struct FakeSource {
        outgoing: Mutex<Vec<LocalItem>>,
        stored: Mutex<HashMap<String, LocalItem>>,
        watermark: Mutex<i64>,
        applies: AtomicUsize,
    }

    impl FakeSource {
        fn with_outgoing(items: Vec<LocalItem>) -> Self {
            Self {
                outgoing: Mutex::new(items),
                ..Self::default()
            }
        }

        fn get(&self, id: &str) -> Option<LocalItem> {
            self.stored.lock().unwrap().get(id).cloned()
        }
    }

    impl CloudSource for FakeSource {
        fn device_id(&self) -> String {
            DEVICE.into()
        }

        fn local_changes_since(&self, since_ms: i64) -> Result<Vec<LocalItem>, SyncError> {
            Ok(self
                .outgoing
                .lock()
                .unwrap()
                .iter()
                .filter(|i| i.created_at >= since_ms)
                .cloned()
                .collect())
        }

        fn apply_remote(&self, incoming: LocalItem) -> Result<bool, SyncError> {
            self.applies.fetch_add(1, Ordering::SeqCst);
            let mut stored = self.stored.lock().unwrap();
            match stored.get(&incoming.item_id) {
                // Strict `>`: equal keeps local, which is what makes a replay
                // free (INV-I1).
                Some(local) if incoming.created_at <= local.created_at => Ok(false),
                _ => {
                    stored.insert(incoming.item_id.clone(), incoming);
                    Ok(true)
                }
            }
        }

        fn watermark(&self) -> Result<i64, SyncError> {
            Ok(*self.watermark.lock().unwrap())
        }

        fn set_watermark(&self, ms: i64) -> Result<(), SyncError> {
            let mut w = self.watermark.lock().unwrap();
            assert!(ms >= *w, "the watermark moved backwards: {} -> {ms}", *w);
            *w = ms;
            Ok(())
        }
    }

    // --- the fake transport -----------------------------------------------

    #[derive(Debug, Clone)]
    enum Reply {
        Ok,
        Unauthorized,
        RateLimited(Option<Duration>),
        Transient,
        Permanent,
    }

    #[derive(Default)]
    struct FakeRest {
        /// Rows the backend holds, keyed on `item_id` — an upsert, like the
        /// real conflict target.
        rows: Mutex<HashMap<String, CloudItem>>,
        /// Scripted failures, consumed in order before any request succeeds.
        script: Mutex<Vec<Reply>>,
        upserts: AtomicUsize,
        tombstones: AtomicUsize,
        fetches: AtomicUsize,
        /// Tokens seen, so a test can assert the refreshed one was used.
        tokens: Mutex<Vec<String>>,
    }

    impl FakeRest {
        fn scripted(script: Vec<Reply>) -> Self {
            Self {
                script: Mutex::new(script),
                ..Self::default()
            }
        }

        fn seeded(rows: Vec<CloudItem>) -> Self {
            Self {
                rows: Mutex::new(rows.into_iter().map(|r| (r.item_id.clone(), r)).collect()),
                ..Self::default()
            }
        }

        fn next(&self, token: &str) -> Option<TransportFault> {
            self.tokens.lock().unwrap().push(token.to_owned());
            let mut script = self.script.lock().unwrap();
            if script.is_empty() {
                return None;
            }
            match script.remove(0) {
                Reply::Ok => None,
                Reply::Unauthorized => Some(TransportFault::Unauthorized),
                Reply::RateLimited(after) => {
                    Some(TransportFault::RateLimited { retry_after: after })
                }
                Reply::Transient => Some(TransportFault::Transient("backend unavailable")),
                Reply::Permanent => Some(TransportFault::Permanent("rejected")),
            }
        }

        fn sorted_rows(&self) -> Vec<CloudItem> {
            let mut rows: Vec<CloudItem> = self.rows.lock().unwrap().values().cloned().collect();
            // A total, tie-free order — INV-N1.
            rows.sort_by(|a, b| {
                a.created_at
                    .cmp(&b.created_at)
                    .then_with(|| a.item_id.cmp(&b.item_id))
            });
            rows
        }
    }

    impl RestApi for FakeRest {
        async fn fetch_since(
            &self,
            token: &str,
            since_ms: i64,
            limit: usize,
        ) -> Result<Vec<CloudItem>, TransportFault> {
            if let Some(fault) = self.next(token) {
                return Err(fault);
            }
            self.fetches.fetch_add(1, Ordering::SeqCst);
            Ok(self
                .sorted_rows()
                .into_iter()
                .filter(|r| r.created_at >= since_ms)
                .take(limit)
                .collect())
        }

        async fn upsert(&self, token: &str, items: &[CloudItem]) -> Result<(), TransportFault> {
            if let Some(fault) = self.next(token) {
                return Err(fault);
            }
            self.upserts.fetch_add(items.len(), Ordering::SeqCst);
            let mut rows = self.rows.lock().unwrap();
            for item in items {
                rows.insert(item.item_id.clone(), item.clone());
            }
            Ok(())
        }

        async fn tombstone(&self, token: &str, ids: &[String]) -> Result<(), TransportFault> {
            if let Some(fault) = self.next(token) {
                return Err(fault);
            }
            self.tombstones.fetch_add(ids.len(), Ordering::SeqCst);
            let mut rows = self.rows.lock().unwrap();
            for id in ids {
                if let Some(row) = rows.get_mut(id) {
                    row.deleted = true;
                    // A tombstone carries no ciphertext (T-4).
                    row.ciphertext = String::new();
                    row.nonce = String::new();
                }
            }
            Ok(())
        }
    }

    #[derive(Default)]
    struct FakeAuth {
        refreshes: AtomicUsize,
        fault: Option<AuthFault>,
    }

    impl FakeAuth {
        fn failing(fault: AuthFault) -> Self {
            Self {
                fault: Some(fault),
                ..Self::default()
            }
        }
    }

    impl AuthApi for FakeAuth {
        async fn refresh(&self, _refresh_token: &str) -> Result<Session, AuthFault> {
            self.refreshes.fetch_add(1, Ordering::SeqCst);
            match &self.fault {
                Some(f) => Err(f.clone()),
                None => Ok(session("token-refreshed")),
            }
        }
    }

    fn driver(rest: FakeRest, auth: FakeAuth) -> CloudSync<FakeRest, FakeAuth> {
        CloudSync::new(
            rest,
            auth,
            key(),
            config(),
            session("token-1"),
            allow_everything(),
        )
    }

    /// A cloud row sealed with the test key, as the backend would hold it.
    fn cloud_row(id: &str, created_at: i64, content: &str) -> CloudItem {
        let (nonce, ciphertext) = encrypt_row(content.as_bytes(), &key(), id).unwrap();
        CloudItem {
            item_id: id.into(),
            ciphertext,
            nonce,
            content_type: "text".into(),
            created_at,
            deleted: false,
            origin_device_id: "device-b".into(),
        }
    }

    // --- push -------------------------------------------------------------

    #[tokio::test]
    async fn push_seals_every_row_so_the_backend_never_sees_plaintext() {
        let source = FakeSource::with_outgoing(vec![item("a", 1_000, "hunter2 in the clear")]);
        let sync = driver(FakeRest::default(), FakeAuth::default());

        let stats = sync.push(&source).await.unwrap();
        assert_eq!(stats.uploaded, 1);

        let rows = sync.rest.rows.lock().unwrap();
        let row = &rows["a"];
        assert!(!row.ciphertext.contains("hunter2"));
        assert!(!row.nonce.is_empty());
        assert!(!row.deleted, "`deleted` must always be explicit (T-5)");

        // And it is the *cloud* key that opens it, bound to this item id.
        assert_eq!(
            decrypt_row(&row.ciphertext, &row.nonce, &key(), "a").unwrap(),
            b"hunter2 in the clear"
        );
        assert!(decrypt_row(&row.ciphertext, &row.nonce, &key(), "b").is_err());
    }

    #[tokio::test]
    async fn push_is_idempotent() {
        let source = FakeSource::with_outgoing(vec![
            item("a", 1_000, "one"),
            item("b", 2_000, "two"),
        ]);
        let sync = driver(FakeRest::default(), FakeAuth::default());

        sync.push(&source).await.unwrap();
        let after_first: Vec<_> = sync.rest.sorted_rows().iter().map(|r| r.item_id.clone()).collect();

        sync.push(&source).await.unwrap();
        let after_second: Vec<_> = sync.rest.sorted_rows().iter().map(|r| r.item_id.clone()).collect();

        // Same set of rows: the upsert conflict target is `item_id`, so a
        // replay overwrites rather than duplicating.
        assert_eq!(after_first, after_second);
        assert_eq!(sync.rest.rows.lock().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn a_sensitive_item_is_never_uploaded() {
        // CopyPaste-20yw / AT-56. The store is *supposed* to filter these; this
        // asserts the second layer, because this is data leaving the machine.
        let source = FakeSource::with_outgoing(vec![
            item("safe", 1_000, "a normal snippet"),
            item("secret", 2_000, "AKIAIOSFODNN7EXAMPLE"),
        ]);
        let sync = CloudSync::new(
            FakeRest::default(),
            FakeAuth::default(),
            key(),
            config(),
            session("token-1"),
            SensitiveGuard::new(|item| item.item_id == "secret"),
        );

        let stats = sync.push(&source).await.unwrap();

        assert_eq!(stats.uploaded, 1);
        assert_eq!(stats.skipped_sensitive, 1);

        let rows = sync.rest.rows.lock().unwrap();
        assert!(rows.contains_key("safe"));
        assert!(
            !rows.contains_key("secret"),
            "a sensitive item reached the backend"
        );
    }

    #[tokio::test]
    async fn a_sensitive_item_is_withheld_even_when_it_is_the_only_one() {
        // The batching must not turn "nothing to upload" into an empty request
        // that still counts as an upload.
        let source = FakeSource::with_outgoing(vec![item("secret", 1_000, "x")]);
        let sync = CloudSync::new(
            FakeRest::default(),
            FakeAuth::default(),
            key(),
            config(),
            session("token-1"),
            SensitiveGuard::new(|_| true),
        );

        let stats = sync.push(&source).await.unwrap();
        assert_eq!(stats.uploaded, 0);
        assert_eq!(stats.skipped_sensitive, 1);
        assert_eq!(sync.rest.upserts.load(Ordering::SeqCst), 0);
        assert!(sync.rest.rows.lock().unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_delete_travels_as_a_tombstone_and_carries_no_ciphertext() {
        // T-4: even if the local version still has content in memory, the
        // tombstone must not carry it.
        let mut dead = tombstone("a", 3_000);
        dead.content = b"content that must not be uploaded".to_vec();

        let source = FakeSource::with_outgoing(vec![item("a", 1_000, "live"), dead]);
        let sync = driver(FakeRest::default(), FakeAuth::default());

        let stats = sync.push(&source).await.unwrap();
        assert_eq!(stats.uploaded, 1);
        assert_eq!(stats.tombstoned, 1);

        let rows = sync.rest.rows.lock().unwrap();
        let row = &rows["a"];
        assert!(row.deleted, "the tombstone did not propagate");
        assert!(row.ciphertext.is_empty(), "a tombstone carried ciphertext");
    }

    #[tokio::test]
    async fn push_does_not_move_the_download_watermark() {
        let source = FakeSource::with_outgoing(vec![item("a", 5_000, "x")]);
        let sync = driver(FakeRest::default(), FakeAuth::default());

        sync.push(&source).await.unwrap();
        assert_eq!(source.watermark().unwrap(), 0);
    }

    // --- pull -------------------------------------------------------------

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
        assert_eq!(source.get("a").unwrap().content, b"first");
        assert_eq!(source.get("b").unwrap().content, b"second");
        assert_eq!(source.watermark().unwrap(), 2_000);
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
        assert_eq!(source.get("a").unwrap().content, b"first");
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
        let mut row = cloud_row("gone", 4_000, "");
        row.deleted = true;
        row.ciphertext = String::new();
        row.nonce = String::new();

        let source = FakeSource::default();
        let sync = driver(FakeRest::seeded(vec![row]), FakeAuth::default());

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
        {
            let mut rows = sync.rest.rows.lock().unwrap();
            let row = rows.get_mut("a").unwrap();
            row.deleted = true;
            row.ciphertext = String::new();
            row.nonce = String::new();
            row.created_at = 2_000;
        }
        sync.pull(&source).await.unwrap();

        let stored = source.get("a").unwrap();
        assert!(stored.deleted, "delete did not win");
        assert!(stored.content.is_empty(), "content survived the tombstone");
    }

    #[tokio::test]
    async fn an_older_live_version_cannot_resurrect_a_newer_tombstone() {
        let source = FakeSource::default();
        // Tombstone first, at a later stamp than the create that follows it.
        let mut dead = cloud_row("a", 5_000, "");
        dead.deleted = true;
        dead.ciphertext = String::new();
        dead.nonce = String::new();

        let sync = driver(FakeRest::seeded(vec![dead]), FakeAuth::default());
        sync.pull(&source).await.unwrap();
        assert!(source.get("a").unwrap().deleted);

        // The out-of-order create arrives.
        sync.rest
            .rows
            .lock()
            .unwrap()
            .insert("a".into(), cloud_row("a", 1_000, "back from the dead"));
        // The watermark has already passed it, but even offered directly it
        // must lose.
        source.set_watermark(0).unwrap_or(());
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
            CloudItem {
                item_id: "b".into(),
                ciphertext,
                nonce,
                content_type: "text".into(),
                created_at: 2_000,
                deleted: false,
                origin_device_id: "device-b".into(),
            },
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
        assert_eq!(source.get("a").unwrap().content, b"real");
        assert_eq!(
            source.watermark().unwrap(),
            1_000,
            "the cursor followed a hostile stamp into the future"
        );
    }

    #[tokio::test]
    async fn a_negative_stamp_is_clamped_at_the_boundary() {
        // CopyPaste-psx7: a negative value cast to unsigned wins every
        // comparison forever. Clamped on arrival, not at the use site.
        let mut row = cloud_row("a", -42, "hostile stamp");
        row.created_at = -42;
        let source = FakeSource::default();
        let sync = driver(FakeRest::seeded(vec![row]), FakeAuth::default());

        sync.pull(&source).await.unwrap();
        assert_eq!(source.get("a").unwrap().created_at, 0);
    }

    #[tokio::test]
    async fn pull_drains_more_rows_than_one_page() {
        // AT-23/AT-24 in miniature: more rows than a page, all fetched, and the
        // cursor is inclusive so the boundary row is never dropped.
        let rows: Vec<CloudItem> = (0..PULL_PAGE_LIMIT + 30)
            .map(|i| cloud_row(&format!("item-{i:04}"), 1_000 + i as i64, "x"))
            .collect();
        let source = FakeSource::default();
        let sync = driver(FakeRest::seeded(rows), FakeAuth::default());

        let stats = sync.pull(&source).await.unwrap();

        assert_eq!(stats.applied, PULL_PAGE_LIMIT + 30);
        assert!(
            sync.rest.fetches.load(Ordering::SeqCst) >= 2,
            "a full page did not trigger a burst drain"
        );
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

    // --- session recovery -------------------------------------------------

    #[tokio::test]
    async fn a_401_refreshes_once_and_retries_once() {
        // AT-34. And the *refreshed* bearer must be the one used on the retry.
        let rest = FakeRest::scripted(vec![Reply::Unauthorized, Reply::Ok]);
        let source = FakeSource::with_outgoing(vec![item("a", 1_000, "x")]);
        let sync = driver(rest, FakeAuth::default());

        let stats = sync.push(&source).await.unwrap();

        assert_eq!(stats.uploaded, 1);
        assert_eq!(sync.auth.refreshes.load(Ordering::SeqCst), 1);
        let tokens = sync.rest.tokens.lock().unwrap();
        assert_eq!(tokens.as_slice(), ["token-1", "token-refreshed"]);
    }

    #[tokio::test]
    async fn a_second_401_is_a_hard_error_not_a_loop() {
        // AT-36. A refresh that hands back a token the server still rejects
        // must not spin.
        let rest = FakeRest::scripted(vec![Reply::Unauthorized, Reply::Unauthorized, Reply::Ok]);
        let source = FakeSource::with_outgoing(vec![item("a", 1_000, "x")]);
        let sync = driver(rest, FakeAuth::default());

        assert_eq!(sync.push(&source).await.unwrap_err(), SyncError::Unauthorized);
        assert_eq!(
            sync.auth.refreshes.load(Ordering::SeqCst),
            1,
            "refreshed more than once"
        );
    }

    #[tokio::test]
    async fn the_401_path_is_the_same_on_the_read_side() {
        // The read path was originally folded into a generic error bucket, so
        // an expired token stalled downloads while uploads kept working.
        let rest = FakeRest::scripted(vec![Reply::Unauthorized, Reply::Ok]);
        rest.rows
            .lock()
            .unwrap()
            .insert("a".into(), cloud_row("a", 1_000, "x"));
        let source = FakeSource::default();
        let sync = driver(rest, FakeAuth::default());

        let stats = sync.pull(&source).await.unwrap();
        assert_eq!(stats.applied, 1);
        assert_eq!(sync.auth.refreshes.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn bad_credentials_and_an_expired_session_are_distinguished() {
        // They need completely different recovery: one prompts a human, the
        // other re-authenticates silently. GoTrue's body cannot tell them
        // apart, which is why the classification comes from the grant kind.
        for (fault, expected) in [
            (AuthFault::InvalidCredentials, SyncError::InvalidCredentials),
            (AuthFault::SessionExpired, SyncError::SessionExpired),
        ] {
            let rest = FakeRest::scripted(vec![Reply::Unauthorized]);
            let source = FakeSource::with_outgoing(vec![item("a", 1_000, "x")]);
            let sync = driver(rest, FakeAuth::failing(fault));

            assert_eq!(sync.push(&source).await.unwrap_err(), expected);
        }
    }

    #[tokio::test]
    async fn auth_failure_never_leaks_a_token_or_falls_back() {
        // INV-N6: no silent downgrade to a lower-privilege scope, and the anon
        // key never appears in an error.
        let rest = FakeRest::scripted(vec![Reply::Unauthorized]);
        let source = FakeSource::with_outgoing(vec![item("a", 1_000, "x")]);
        let sync = driver(rest, FakeAuth::failing(AuthFault::SessionExpired));

        let err = sync.push(&source).await.unwrap_err();
        let rendered = err.to_string();
        assert!(!rendered.contains("anon"));
        assert!(!rendered.contains("token-1"));
        assert!(!rendered.contains("refresh-1"));
        // And nothing was uploaded under a degraded identity.
        assert!(sync.rest.rows.lock().unwrap().is_empty());
    }

    #[tokio::test(start_paused = true)]
    async fn a_429_is_honoured_once_and_then_gives_up() {
        // AT-39. `start_paused` makes the sleep instant while still asserting
        // that we waited the amount the server asked for.
        let rest = FakeRest::scripted(vec![
            Reply::RateLimited(Some(Duration::from_secs(7))),
            Reply::Ok,
        ]);
        let source = FakeSource::with_outgoing(vec![item("a", 1_000, "x")]);
        let sync = driver(rest, FakeAuth::default());

        let started = tokio::time::Instant::now();
        let stats = sync.push(&source).await.unwrap();
        assert_eq!(stats.uploaded, 1);
        assert!(
            started.elapsed() >= Duration::from_secs(7),
            "Retry-After was not honoured"
        );

        // Twice in a row is an error, not a second wait.
        let rest = FakeRest::scripted(vec![
            Reply::RateLimited(Some(Duration::from_secs(1))),
            Reply::RateLimited(Some(Duration::from_secs(1))),
            Reply::Ok,
        ]);
        let sync = driver(rest, FakeAuth::default());
        assert_eq!(sync.push(&source).await.unwrap_err(), SyncError::RateLimited);
    }

    #[tokio::test(start_paused = true)]
    async fn a_retry_after_is_clamped() {
        // A server asking for an hour does not get to hold the loop for an
        // hour.
        let rest = FakeRest::scripted(vec![
            Reply::RateLimited(Some(Duration::from_secs(3_600))),
            Reply::Ok,
        ]);
        let source = FakeSource::with_outgoing(vec![item("a", 1_000, "x")]);
        let sync = driver(rest, FakeAuth::default());

        let started = tokio::time::Instant::now();
        sync.push(&source).await.unwrap();
        assert!(started.elapsed() <= MAX_RETRY_AFTER + Duration::from_secs(1));
    }

    #[tokio::test(start_paused = true)]
    async fn a_429_without_a_header_still_waits() {
        let rest = FakeRest::scripted(vec![Reply::RateLimited(None), Reply::Ok]);
        let source = FakeSource::with_outgoing(vec![item("a", 1_000, "x")]);
        let sync = driver(rest, FakeAuth::default());

        assert_eq!(sync.push(&source).await.unwrap().uploaded, 1);
    }

    #[tokio::test(start_paused = true)]
    async fn transient_failures_are_retried_within_a_budget() {
        let rest = FakeRest::scripted(vec![Reply::Transient, Reply::Transient, Reply::Ok]);
        let source = FakeSource::with_outgoing(vec![item("a", 1_000, "x")]);
        let sync = driver(rest, FakeAuth::default());
        assert_eq!(sync.push(&source).await.unwrap().uploaded, 1);

        // Past the budget it gives up rather than retrying forever.
        let rest = FakeRest::scripted(vec![Reply::Transient; 10]);
        let sync = driver(rest, FakeAuth::default());
        assert!(matches!(
            sync.push(&source).await.unwrap_err(),
            SyncError::Transport(_)
        ));
    }

    #[tokio::test]
    async fn a_permanent_failure_is_not_retried() {
        let rest = FakeRest::scripted(vec![Reply::Permanent, Reply::Ok]);
        let source = FakeSource::with_outgoing(vec![item("a", 1_000, "x")]);
        let sync = driver(rest, FakeAuth::default());

        assert!(matches!(
            sync.push(&source).await.unwrap_err(),
            SyncError::Transport(_)
        ));
        assert_eq!(sync.rest.upserts.load(Ordering::SeqCst), 0);
    }

    // --- cadence ----------------------------------------------------------

    #[tokio::test]
    async fn the_idle_interval_grows_and_is_bounded() {
        let source = FakeSource::default();
        let sync = driver(FakeRest::default(), FakeAuth::default());

        assert_eq!(sync.poll_interval(), MIN_POLL_INTERVAL);

        let mut previous = sync.poll_interval();
        for _ in 0..20 {
            sync.sync(&source).await.unwrap();
            let now = sync.poll_interval();
            assert!(now >= previous, "the idle interval went backwards");
            assert!(
                now <= MAX_POLL_INTERVAL,
                "the idle interval passed its ceiling: {now:?}"
            );
            previous = now;
        }
        assert_eq!(
            previous, MAX_POLL_INTERVAL,
            "the idle interval never reached its ceiling"
        );
    }

    #[tokio::test]
    async fn any_change_resets_the_idle_interval() {
        let source = FakeSource::default();
        let sync = driver(
            FakeRest::seeded(vec![cloud_row("a", 1_000, "x")]),
            FakeAuth::default(),
        );

        // Idle for a while.
        for _ in 0..5 {
            sync.sync(&source).await.unwrap();
        }
        // The seeded row applies on the first sync, so drift only starts after
        // it; force some drift, then deliver a change.
        let drifted = sync.poll_interval();
        sync.rest
            .rows
            .lock()
            .unwrap()
            .insert("b".into(), cloud_row("b", 9_000, "new"));

        let stats = sync.sync(&source).await.unwrap();
        assert!(stats.changed());
        assert_eq!(sync.poll_interval(), MIN_POLL_INTERVAL);
        assert!(drifted > MIN_POLL_INTERVAL, "the interval never drifted");
    }

    #[tokio::test]
    async fn a_realtime_event_can_wake_the_poll_loop() {
        let source = FakeSource::default();
        let sync = driver(FakeRest::default(), FakeAuth::default());

        for _ in 0..4 {
            sync.sync(&source).await.unwrap();
        }
        assert!(sync.poll_interval() > MIN_POLL_INTERVAL);

        sync.wake();
        assert_eq!(sync.poll_interval(), MIN_POLL_INTERVAL);
    }

    // --- end to end -------------------------------------------------------

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
            limit: usize,
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

    // --- hygiene ----------------------------------------------------------

    #[test]
    fn errors_carry_no_paths_tokens_or_passphrases() {
        let errors = [
            SyncError::Source("could not read the history"),
            SyncError::Encrypt,
            SyncError::Unauthorized,
            SyncError::InvalidCredentials,
            SyncError::SessionExpired,
            SyncError::RateLimited,
            SyncError::Transport("backend unavailable"),
        ];
        for e in errors {
            let msg = e.to_string();
            assert!(!msg.contains('/'), "path-like separator in {msg:?}");
            assert!(!msg.contains('\\'), "path-like separator in {msg:?}");
            assert!(!msg.contains("home"), "path-like fragment in {msg:?}");
            assert!(!msg.contains(PASS), "passphrase in {msg:?}");
            assert!(!msg.contains("eyJ"), "jwt-like text in {msg:?}");
        }
    }

    #[test]
    fn the_driver_and_the_guard_redact_their_debug_output() {
        let sync = driver(FakeRest::default(), FakeAuth::default());
        assert_eq!(format!("{sync:?}"), "CloudSync { .. }");
        assert_eq!(
            format!("{:?}", allow_everything()),
            "SensitiveGuard { .. }"
        );
    }

    #[test]
    fn the_tunables_are_the_documented_ones() {
        assert_eq!(MIN_POLL_INTERVAL, Duration::from_secs(5));
        assert_eq!(MAX_POLL_INTERVAL, Duration::from_secs(300));
        assert_eq!(MAX_TRANSIENT_ATTEMPTS, 4);
        assert_eq!(TRANSIENT_INITIAL, Duration::from_secs(1));
        assert_eq!(TRANSIENT_MAX, Duration::from_secs(30));
        assert_eq!(MAX_FUTURE_SKEW_MS, 24 * 60 * 60 * 1000);
    }
}
