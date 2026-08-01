//! Test doubles shared by every file in this module.
//!
//! One fake store and one fake transport, so that the push, pull, recovery and
//! cadence rules are all asserted against the same behaviour rather than
//! against four slightly different mocks. Nothing here opens a socket.
//!
//! This mirrors [`crate::auth::stub`], which plays the same role for the two
//! HTTP clients.

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use super::*;
use crate::auth::{now_ms, Session};
use crate::crypto::{encrypt_row, SyncKey};
use crate::rest::CloudItem;
use crate::CloudConfig;

pub(super) const PASS: &str = "correct horse battery staple";
pub(super) const ACCOUNT: &str = "acct-1";
const DEVICE: &str = "device-a";

/// The fixture key, derived once per binary and rebuilt from bytes after
/// that. Argon2id is deliberately expensive; a hundred-row fixture that
/// re-derived per row would take minutes.
pub(super) fn key() -> SyncKey {
    static KEY: std::sync::OnceLock<[u8; crate::crypto::KEY_LEN]> = std::sync::OnceLock::new();
    SyncKey::from_bytes(*KEY.get_or_init(|| {
        *crate::crypto::derive_sync_key(PASS, ACCOUNT)
            .unwrap()
            .to_bytes()
    }))
}

pub(super) fn config() -> CloudConfig {
    CloudConfig {
        url: "https://proj.supabase.invalid".into(),
        anon_key: "anon".into(),
    }
}

pub(super) fn session(access: &str) -> Session {
    Session {
        access_token: access.into(),
        refresh_token: "refresh-1".into(),
        user_id: "user-1".into(),
        expires_at_ms: now_ms() + 3_600_000,
    }
}

pub(super) fn allow_everything() -> SensitiveGuard {
    SensitiveGuard::new(|_| false)
}

pub(super) fn item(id: &str, created_at: i64, content: &str) -> LocalItem {
    LocalItem {
        item_id: id.into(),
        content: content.as_bytes().to_vec(),
        content_type: "text".into(),
        payload_metadata: None,
        created_at,
        deleted: false,
        origin_device_id: DEVICE.into(),
    }
}

pub(super) fn tombstone(id: &str, created_at: i64) -> LocalItem {
    LocalItem {
        item_id: id.into(),
        content: Vec::new(),
        content_type: "text".into(),
        payload_metadata: None,
        created_at,
        deleted: true,
        origin_device_id: DEVICE.into(),
    }
}

/// A cloud row sealed *and signed* with the test key, as the backend would hold
/// it. Signed because an unsigned row is not one any device will accept — a
/// fixture that skipped it would be testing the refusal path by accident.
pub(super) fn cloud_row(id: &str, created_at: i64, content: &str) -> CloudItem {
    let (nonce, ciphertext) = encrypt_row(content.as_bytes(), &key(), id).unwrap();
    signed(CloudItem {
        item_id: id.into(),
        ciphertext,
        nonce,
        content_type: "text".into(),
        payload_metadata: None,
        created_at,
        deleted: false,
        origin_device_id: "device-b".into(),
        signature: String::new(),
    })
}

/// A cloud tombstone, signed.
pub(super) fn cloud_tombstone(id: &str, created_at: i64) -> CloudItem {
    signed(CloudItem::tombstone(id, "text", created_at, "device-b"))
}

/// Re-sign a row after a test has changed it, so that the test exercises the
/// rule it is about rather than the signature check.
pub(super) fn signed(mut row: CloudItem) -> CloudItem {
    row.sign(&key());
    row
}

pub(super) fn driver(rest: FakeRest, auth: FakeAuth) -> CloudSync<FakeRest, FakeAuth> {
    CloudSync::new(
        rest,
        auth,
        key(),
        config(),
        session("token-1"),
        allow_everything(),
    )
    .without_retry_delays()
}

// ---------------------------------------------------------------------------
// The fake store
// ---------------------------------------------------------------------------

/// A store with the ordering the real one is required to have: newer
/// `created_at` wins, an exact tie keeps local. That tie rule is what makes
/// replay a no-op, so the idempotency tests are only meaningful with it.
#[derive(Default)]
pub(super) struct FakeSource {
    outgoing: Mutex<Vec<LocalItem>>,
    stored: Mutex<HashMap<String, LocalItem>>,
    watermark: Mutex<i64>,
    /// The tie-break half, for the sources that keep one. `None` in the
    /// default fake, which is the weaker cursor most of these tests exercise.
    watermark_item_id: Mutex<Option<String>>,
    keyset: bool,
    /// Tracked separately from the watermark, as a real source must — see
    /// [`CloudSource::upload_floor`].
    floor: Mutex<i64>,
    #[allow(dead_code)]
    applies: AtomicUsize,
}

impl FakeSource {
    pub(super) fn with_outgoing(items: Vec<LocalItem>) -> Self {
        Self {
            outgoing: Mutex::new(items),
            ..Self::default()
        }
    }

    /// A source that persists both halves of the keyset cursor, as a real one
    /// should.
    pub(super) fn with_keyset_watermark() -> Self {
        Self {
            keyset: true,
            ..Self::default()
        }
    }

    pub(super) fn get(&self, id: &str) -> Option<LocalItem> {
        self.stored.lock().unwrap().get(id).cloned()
    }

    /// Move the cursor without the monotonicity assertion, to set up a case
    /// the driver itself would never produce.
    pub(super) fn rewind(&self, ms: i64) {
        *self.watermark.lock().unwrap() = ms;
    }

    /// What the owner of a source does after a round completes.
    pub(super) fn set_upload_floor(&self, ms: i64) {
        *self.floor.lock().unwrap() = ms;
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

    fn watermark_item_id(&self) -> Result<Option<String>, SyncError> {
        Ok(self.watermark_item_id.lock().unwrap().clone())
    }

    fn upload_floor(&self) -> Result<i64, SyncError> {
        Ok(*self.floor.lock().unwrap())
    }

    fn set_watermark(&self, ms: i64) -> Result<(), SyncError> {
        let mut w = self.watermark.lock().unwrap();
        // INV-N5, asserted for every test that pulls: the driver must never
        // hand back a lower cursor than it was given.
        assert!(ms >= *w, "the watermark moved backwards: {} -> {ms}", *w);
        *w = ms;
        *self.watermark_item_id.lock().unwrap() = None;
        Ok(())
    }

    fn set_watermark_keyset(&self, ms: i64, item_id: &str) -> Result<(), SyncError> {
        if !self.keyset {
            return self.set_watermark(ms);
        }
        let mut w = self.watermark.lock().unwrap();
        assert!(ms >= *w, "the watermark moved backwards: {} -> {ms}", *w);
        *w = ms;
        *self.watermark_item_id.lock().unwrap() = Some(item_id.to_owned());
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// The fake transport
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub(super) enum Reply {
    Ok,
    Unauthorized,
    RateLimited(Option<Duration>),
    Transient,
    Permanent,
}

#[derive(Default)]
pub(super) struct FakeRest {
    /// Rows the backend holds, keyed on `item_id` — an upsert, like the
    /// real conflict target.
    pub(super) rows: Mutex<HashMap<String, CloudItem>>,
    /// Scripted failures, consumed in order before any request succeeds.
    script: Mutex<Vec<Reply>>,
    pub(super) upserts: AtomicUsize,
    pub(super) fetches: AtomicUsize,
    /// Tokens seen, so a test can assert the refreshed one was used.
    pub(super) tokens: Mutex<Vec<String>>,
}

impl FakeRest {
    pub(super) fn scripted(script: Vec<Reply>) -> Self {
        Self {
            script: Mutex::new(script),
            ..Self::default()
        }
    }

    pub(super) fn seeded(rows: Vec<CloudItem>) -> Self {
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
            Reply::RateLimited(after) => Some(TransportFault::RateLimited { retry_after: after }),
            Reply::Transient => Some(TransportFault::Transient("backend unavailable")),
            Reply::Permanent => Some(TransportFault::Permanent("rejected")),
        }
    }

    pub(super) fn sorted_rows(&self) -> Vec<CloudItem> {
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
    /// Pages exactly as the deployment does: inclusive on the millisecond
    /// alone, strictly after the pair once the tie-break is known.
    async fn fetch_since(
        &self,
        token: &str,
        since_ms: i64,
        after_item_id: Option<&str>,
        limit: u32,
    ) -> Result<Vec<CloudItem>, TransportFault> {
        if let Some(fault) = self.next(token) {
            return Err(fault);
        }
        self.fetches.fetch_add(1, Ordering::SeqCst);
        Ok(self
            .sorted_rows()
            .into_iter()
            .filter(|r| match after_item_id {
                Some(id) => (r.created_at, r.item_id.as_str()) > (since_ms, id),
                None => r.created_at >= since_ms,
            })
            .take(limit as usize)
            .collect())
    }

    /// Validates like the real transport does, so a test cannot upload a row
    /// the deployment would refuse — an unsigned one above all.
    async fn upsert(&self, token: &str, items: &[CloudItem]) -> Result<(), TransportFault> {
        if let Some(fault) = self.next(token) {
            return Err(fault);
        }
        for item in items {
            if item.signature.is_empty() {
                return Err(TransportFault::Permanent(
                    "the row's metadata was never signed",
                ));
            }
        }
        self.upserts.fetch_add(items.len(), Ordering::SeqCst);
        let mut rows = self.rows.lock().unwrap();
        for item in items {
            rows.insert(item.item_id.clone(), item.clone());
        }
        Ok(())
    }
}

#[derive(Default)]
pub(super) struct FakeAuth {
    pub(super) refreshes: AtomicUsize,
    fault: Option<AuthFault>,
}

impl FakeAuth {
    pub(super) fn failing(fault: AuthFault) -> Self {
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
