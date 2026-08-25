//! Cloud sync, wired into the daemon.
//!
//! # Where the credentials live
//!
//! Three secrets are stored: the access token, the rotated refresh token, and
//! the derived sync key — never the account password, never the passphrase. So
//! a stolen database yields a session that expires and a key for one account,
//! not the means to re-derive one elsewhere.
//!
//! They live in `sync_device_state` inside the SQLCipher database, under the
//! device key from the OS keystore. A token in a plain file beside an encrypted
//! database would be the weakest link in a design whose claim is that the
//! backend never sees plaintext.
//!
//! Unconfigured and signed-out are both states the daemon runs in normally;
//! local history and peer sync do not depend on any of this.

mod account;
pub mod handlers;
pub mod poll;
pub mod realtime;
pub mod refresh;
pub mod source;

use std::sync::atomic::{AtomicI64, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use copypaste_cloud::auth::SupabaseAuth;
use copypaste_cloud::credentials::CloudStateKey;
use copypaste_cloud::rest::SupabaseRest;
use copypaste_cloud::sync::{CloudSync, SensitiveGuard};
use copypaste_cloud::CloudConfig;
use copypaste_core::sync::{RoundGate, RoundGuard};
use copypaste_core::StoreError;
use copypaste_ipc::CloudStatusData;
use tokio::sync::{watch, Notify};
use tracing::warn;

use crate::meta::Meta;
use crate::AppState;

use account::Account;
pub(crate) use account::{ActivateError, ActivateRequest, MSG_ACCOUNT_CHANGED};
pub use poll::run;

pub(crate) const KEY_LAST_SYNC: &str = CloudStateKey::LastSyncMs.as_str();
#[cfg(test)]
pub(crate) const KEY_CURSOR_USER_ID: &str = CloudStateKey::CursorUserId.as_str();
#[cfg(test)]
pub(crate) const KEY_REFRESH: &str = CloudStateKey::RefreshToken.as_str();
#[cfg(test)]
pub(crate) const KEY_SYNC_KEY: &str = CloudStateKey::SyncKey.as_str();

/// The production instantiation of the driver.
pub type Driver = CloudSync<SupabaseRest, SupabaseAuth>;

pub(crate) enum SetEndpointError {
    Incomplete,
    Invalid,
    Store(StoreError),
}

impl From<StoreError> for SetEndpointError {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}

/// The download cursor: everything this device has reconciled with the account.
pub(crate) const KEY_WATERMARK: &str = CloudStateKey::WatermarkMs.as_str();
/// The tie-break half of that cursor: the last `item_id` the millisecond above
/// covers. Written in the same transaction, never on its own.
pub(crate) const KEY_WATERMARK_ITEM: &str = CloudStateKey::WatermarkItemId.as_str();
/// The upload floor: everything created before this has been offered for upload.
pub(crate) const KEY_UPLOAD_FLOOR: &str = CloudStateKey::UploadFloorMs.as_str();
/// The tie-break half of the upload floor. Together the two keys let a capped
/// upload scan resume after `(created_at, item_id)` instead of re-reading a
/// full boundary page forever.
pub(crate) const KEY_UPLOAD_FLOOR_ITEM: &str = CloudStateKey::UploadFloorItemId.as_str();
/// Local rows this device could not open for upload. Ids only — see
/// [`copypaste_cloud::sync::UnreadableUploads`].
pub(crate) const KEY_UNREADABLE_UPLOADS: &str = CloudStateKey::UnreadableUploads.as_str();

#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) struct UploadFloor {
    pub(crate) created_at: i64,
    pub(crate) item_id: Option<String>,
}

/// Everything the cloud half of the daemon shares.
pub struct Cloud {
    /// First-party bake or process env. Custom Advanced values overlay this.
    hosted: Option<CloudConfig>,
    /// `None` when no deployment is configured. Not a credential — the anon key
    /// is publishable and row-level security is what restricts access.
    config: Mutex<Option<CloudConfig>>,
    account: Mutex<Option<Account>>,
    account_revision: AtomicU64,
    /// Zero means "never". An `AtomicI64` rather than a field of `Account` so
    /// that `status` can read it without taking the account lock a round may be
    /// holding.
    last_sync_ms: AtomicI64,
    /// Why the last round failed, as a fixed sentence. `SyncError`'s payloads
    /// are `&'static str` by construction, so there is nothing here that could
    /// have interpolated a path or a token.
    last_error: Mutex<Option<&'static str>>,
    /// Serializes a peer lowering the upload floor with a completed cloud round
    /// advancing it. Without this, the round can overwrite an old-stamp peer
    /// write that arrived while its network requests were in flight.
    upload_floor_lock: Mutex<()>,
    /// Changes whenever a writer asks the floor to be reconsidered. The floor
    /// pair alone cannot record a peer write that lands in the same boundary
    /// millisecond while a round is scanning it.
    upload_floor_epoch: AtomicU64,
    /// Changes whenever the in-memory session may have rotated. Realtime
    /// listens to this so its live channel receives the new bearer before the
    /// old one expires.
    session_revision: watch::Sender<u64>,
    /// Woken by sign-in and by `cloud sync`, so neither has to wait out the
    /// idle interval.
    wake: Notify,
    /// One round at a time. Without it `cloud sync` ran a second round beside
    /// the poll loop's, against the same history and the same floor.
    rounds: RoundGate,
    /// How many local rows the last upload scan could not open. Read by
    /// `status`, which must not take a lock a round may be holding.
    unreadable_uploads: AtomicU32,
}

impl std::fmt::Debug for Cloud {
    /// No URL, no email, no tokens: this type exists to hold credentials.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cloud")
            .field("configured", &self.is_configured())
            .field("signed_in", &self.signed_in())
            .finish_non_exhaustive()
    }
}

impl Cloud {
    pub fn new(config: Option<CloudConfig>) -> Self {
        let (session_revision, _) = watch::channel(0_u64);
        Self {
            hosted: config.clone(),
            config: Mutex::new(config),
            account: Mutex::new(None),
            account_revision: AtomicU64::new(0),
            last_sync_ms: AtomicI64::new(0),
            last_error: Mutex::new(None),
            upload_floor_lock: Mutex::new(()),
            upload_floor_epoch: AtomicU64::new(0),
            wake: Notify::new(),
            rounds: RoundGate::new(),
            unreadable_uploads: AtomicU32::new(0),
            session_revision,
        }
    }

    pub fn is_configured(&self) -> bool {
        self.config().is_some()
    }

    pub fn signed_in(&self) -> bool {
        self.lock_account().is_some()
    }

    /// The driver for the signed-in account, if there is one.
    ///
    /// Hands back an `Arc` rather than a guard: a round takes minutes of
    /// network time and must not hold a lock that `status` needs.
    pub fn driver(&self) -> Option<Arc<Driver>> {
        self.lock_account()
            .as_ref()
            .map(|account| Arc::clone(&account.driver))
    }

    /// The permit for one round, or `None` when a round is already running.
    ///
    /// The poll loop takes this one: a tick that collides with a round in
    /// flight has nothing to add, and it will tick again.
    pub(crate) fn try_begin_round(&self) -> Option<RoundGuard> {
        self.rounds.try_enter()
    }

    /// The permit for one round, waiting for any running round first.
    ///
    /// `cloud sync` takes this one: the user asked after the running round
    /// began its scan, so answering with that round's result would answer for
    /// a history older than the request.
    pub(crate) async fn begin_round(&self) -> RoundGuard {
        self.rounds.enter().await
    }

    #[cfg(test)]
    pub(crate) fn round_in_flight(&self) -> bool {
        self.rounds.is_running()
    }

    /// Ask the poll loop to run a round now.
    pub fn wake(&self) {
        self.wake.notify_one();
    }

    pub(crate) fn session_updates(&self) -> watch::Receiver<u64> {
        self.session_revision.subscribe()
    }

    fn notify_session_changed(&self) {
        self.session_revision.send_modify(|revision| {
            *revision = revision.wrapping_add(1);
        });
    }

    /// Resolves when someone calls [`Cloud::wake`]. `Notify` stores one permit,
    /// so a wake that arrives while a round is running is not lost.
    pub async fn wake_signal(&self) {
        self.wake.notified().await;
    }

    pub fn status(&self) -> CloudStatusData {
        let account = self.lock_account();
        let last_sync_ms = self.last_sync_ms.load(Ordering::Acquire);
        CloudStatusData {
            configured: self.is_configured(),
            signed_in: account.is_some(),
            // One and the same in v2: the key is derived during sign-in, so a
            // session without a key cannot be constructed. v1 could be signed
            // in with no passphrase and silently synced nothing; the field
            // stays on the wire because a UI should be able to say which half
            // is missing if that ever changes.
            key_ready: account.is_some(),
            email: account.as_ref().map(|a| a.email.clone()),
            last_sync_ms: (last_sync_ms > 0).then_some(last_sync_ms),
            last_error: self.lock_error().map(str::to_string),
            unreadable_uploads: self.unreadable_uploads.load(Ordering::Acquire),
            poll_interval_secs: account
                .as_ref()
                .map_or(poll::SIGNED_OUT_INTERVAL, |a| a.driver.poll_interval())
                .as_secs(),
        }
    }

    // Lock recovery rather than propagation, as elsewhere in the daemon: a
    // poisoned lock means some other task panicked, and refusing to sync from
    // then on is a worse outcome than continuing.
    fn lock_account(&self) -> MutexGuard<'_, Option<Account>> {
        self.account.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn lock_error(&self) -> MutexGuard<'_, Option<&'static str>> {
        self.last_error.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn lock_upload_floor(&self) -> MutexGuard<'_, ()> {
        self.upload_floor_lock
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }

    fn upload_floor(&self, meta: &Meta) -> Result<UploadFloor, crate::meta::MetaError> {
        Ok(UploadFloor {
            created_at: meta.state_ms(KEY_UPLOAD_FLOOR)?,
            item_id: meta.state(KEY_UPLOAD_FLOOR_ITEM)?,
        })
    }

    fn store_upload_floor(
        &self,
        meta: &Meta,
        floor: &UploadFloor,
    ) -> Result<(), crate::meta::MetaError> {
        meta.set_state_all(&[
            (KEY_UPLOAD_FLOOR, &floor.created_at.max(0).to_string()),
            (
                KEY_UPLOAD_FLOOR_ITEM,
                floor.item_id.as_deref().unwrap_or(""),
            ),
        ])
    }

    /// Record how many local rows an upload scan could not open.
    ///
    /// A count, never an id: what reaches `status` reaches a client, and
    /// `AGENTS.md` rule 4 keeps paths and row identity out of both.
    pub(crate) fn note_unreadable_uploads(&self, total: u32) {
        self.unreadable_uploads.store(total, Ordering::Release);
    }

    pub(crate) fn upload_floor_epoch(&self) -> u64 {
        self.upload_floor_epoch.load(Ordering::Acquire)
    }

    pub(crate) fn note_version_written(&self, meta: &Meta, created_at_ms: i64) {
        self.upload_floor_epoch.fetch_add(1, Ordering::AcqRel);
        let _guard = self.lock_upload_floor();
        let incoming = UploadFloor {
            created_at: created_at_ms.max(0),
            // An old-stamp write has no reliable position within this device's
            // keyset, so re-offer its whole boundary millisecond.
            item_id: None,
        };
        match self.upload_floor(meta) {
            Ok(floor) if floor <= incoming => {}
            Ok(_) => {
                if let Err(e) = self.store_upload_floor(meta, &incoming) {
                    warn!(error = ?e, "could not lower the cloud upload floor");
                }
            }
            Err(e) => warn!(error = ?e, "could not read the cloud upload floor"),
        }
    }

    pub(crate) fn commit_upload_floor(
        &self,
        meta: &Meta,
        started: &UploadFloor,
        started_epoch: u64,
        candidate: &UploadFloor,
    ) -> Result<(), crate::meta::MetaError> {
        let _guard = self.lock_upload_floor();
        if self.upload_floor_epoch() != started_epoch {
            return Ok(());
        }
        let current = self.upload_floor(meta)?;
        // A value below the one this round began from can only be a writer
        // (not this round) pulling the floor back. Keep it so that writer is
        // offered next time; a re-offer is safe, stranding it is not.
        if current < *started {
            return Ok(());
        }
        self.store_upload_floor(meta, &current.max(candidate.clone()))
    }

    fn lock_config(&self) -> MutexGuard<'_, Option<CloudConfig>> {
        self.config.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn config(&self) -> Option<CloudConfig> {
        self.lock_config().clone()
    }

    pub(crate) fn overlay_persisted_endpoint(&self, store: &copypaste_core::Store) {
        let url = store
            .state(CloudStateKey::EndpointUrl.as_str())
            .ok()
            .flatten();
        let key = store
            .state(CloudStateKey::EndpointAnonKey.as_str())
            .ok()
            .flatten();
        let next = match (url, key) {
            (Some(url), Some(key)) => match CloudConfig::new(url, key) {
                Ok(config) => Some(config),
                Err(_) => {
                    warn!("stored cloud endpoint was rejected; using the hosted default");
                    self.hosted.clone()
                }
            },
            _ => self.hosted.clone(),
        };
        *self.lock_config() = next;
    }

    pub(crate) fn set_custom_endpoint(
        &self,
        store: &copypaste_core::Store,
        url: &str,
        anon_key: &str,
    ) -> Result<(), SetEndpointError> {
        let url = url.trim();
        let anon_key = anon_key.trim();
        if url.is_empty() && anon_key.is_empty() {
            store.clear_state(&[
                CloudStateKey::EndpointUrl.as_str(),
                CloudStateKey::EndpointAnonKey.as_str(),
            ])?;
            *self.lock_config() = self.hosted.clone();
            return Ok(());
        }
        if url.is_empty() || anon_key.is_empty() {
            return Err(SetEndpointError::Incomplete);
        }
        let config = CloudConfig::new(url, anon_key).map_err(|_| SetEndpointError::Invalid)?;
        store.set_state_all(&[
            (CloudStateKey::EndpointUrl.as_str(), url),
            (CloudStateKey::EndpointAnonKey.as_str(), anon_key),
        ])?;
        *self.lock_config() = Some(config);
        Ok(())
    }
}

/// Tell the cloud transport that a version was written *below* its cursor.
///
/// The floor assumes a version's `created_at` is when it appeared here. A peer
/// session breaks that (rows carry the sender's stamp); without pulling the
/// floor back those rows never upload. Lowering the floor costs one re-offer
/// of everything above it, and a re-offer is an idempotent upsert.
pub fn note_version_written(state: &AppState, created_at_ms: i64) {
    state.cloud.note_version_written(&state.meta, created_at_ms);
}

/// The gate every item passes before it may leave the device.
///
/// The store already filters sensitive rows out of the outbound query; this is
/// the second layer, and it exists because manifest 05 AT-56
/// (`CopyPaste-20yw`) records that v1 had exactly one enforcement point and it
/// had a hole. It calls the daemon's own detector rather than re-implementing a
/// "quick check", which would be a second regex engine disagreeing with the one
/// that ran at capture time.
fn sensitive_guard(state: &AppState) -> SensitiveGuard {
    let detector = Arc::clone(&state.detector);
    SensitiveGuard::new(move |item| {
        std::str::from_utf8(&item.content)
            // Not decodable as text: nothing the ruleset can judge, and not
            // something to guess about. The store's capture-time flag is the
            // first layer and has already had its say.
            .is_ok_and(|text| detector.is_sensitive(text))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{test_state, test_state_with_cloud};
    use copypaste_cloud::auth::Session;
    use copypaste_cloud::crypto::derive_sync_key;

    fn config() -> CloudConfig {
        CloudConfig::new("https://example.supabase.co", "anon").unwrap()
    }

    fn session() -> Session {
        Session {
            access_token: "access-1".into(),
            refresh_token: "refresh-1".into(),
            user_id: "user-1".into(),
            expires_at_ms: 1_700_000_000_000,
        }
    }

    #[test]
    fn an_unconfigured_daemon_reports_signed_out_and_never_restores() {
        let (state, _dir) = test_state("alpha");
        assert!(!state.cloud.is_configured());
        assert!(!state.cloud.restore(&state));

        let status = state.cloud.status();
        assert!(!status.configured);
        assert!(!status.signed_in);
        assert_eq!(status.email, None);
        assert_eq!(status.last_sync_ms, None);
    }

    #[test]
    fn an_account_survives_a_restart_and_sign_out_clears_it() {
        let (state, dir) = test_state_with_cloud("alpha", Cloud::new(Some(config())));
        let key = derive_sync_key("correct horse battery staple", "user-1").unwrap();
        state.cloud.install(
            &state,
            config(),
            "a@example.com".into(),
            "user-1".into(),
            derive_sync_key("correct horse battery staple", "user-1").unwrap(),
            session(),
        );
        let key_bytes = key.to_bytes();
        state.cloud.persist(&state.store, &key_bytes).unwrap();
        assert!(state.cloud.signed_in());

        // A second daemon over the same database picks the account back up.
        let restarted = Cloud::new(Some(config()));
        let (state2, _dir2) = crate::testutil::reopen(dir, restarted, "alpha");
        assert!(state2.cloud.restore(&state2));
        assert_eq!(
            state2.cloud.status().email.as_deref(),
            Some("a@example.com")
        );

        // Signing out leaves nothing behind to restore from.
        let stale_driver = state2.cloud.driver().unwrap();
        state2.cloud.sign_out(&state2.store);
        state2.cloud.persist_session(&state2.store, &stale_driver);
        assert!(!state2.cloud.signed_in());
        assert!(!state2.cloud.restore(&state2));
        assert_eq!(state2.meta.state(KEY_REFRESH).unwrap(), None);
        assert_eq!(state2.meta.state(KEY_SYNC_KEY).unwrap(), None);
    }

    /// During an outage `last sync` is the only number that says how stale the
    /// account is, so a restart must not reset it to "never" — which reads as
    /// "sync has never worked" on an account that synced an hour ago.
    #[test]
    fn how_long_ago_the_last_round_completed_survives_a_restart() {
        let (state, dir) = test_state_with_cloud("alpha", Cloud::new(Some(config())));
        let key = derive_sync_key("correct horse battery staple", "user-1").unwrap();
        state.cloud.install(
            &state,
            config(),
            "a@example.com".into(),
            "user-1".into(),
            derive_sync_key("correct horse battery staple", "user-1").unwrap(),
            session(),
        );
        let key_bytes = key.to_bytes();
        state.cloud.persist(&state.store, &key_bytes).unwrap();

        let at = 1_700_000_000_000;
        let driver = state.cloud.driver().unwrap();
        state.cloud.note_driver_success(&state.meta, &driver, at);

        let (restarted, _dir) = crate::testutil::reopen(dir, Cloud::new(Some(config())), "alpha");
        assert!(restarted.cloud.restore(&restarted));
        assert_eq!(restarted.cloud.status().last_sync_ms, Some(at));

        // And it is the *account's* number, not the device's: signing out has
        // to take it with the rest of the account.
        restarted.cloud.sign_out(&restarted.store);
        assert_eq!(restarted.meta.state(KEY_LAST_SYNC).unwrap(), None);
        assert_eq!(restarted.cloud.status().last_sync_ms, None);
    }

    /// N1. The count is what *this account's* scan of the history found. A
    /// second account has scanned none of it, so carrying the number over shows
    /// the previous account's figure — and the rows behind it are not even
    /// necessarily this account's problem — until the first round finishes.
    #[test]
    fn switching_accounts_clears_the_unreadable_upload_count() {
        let (state, _dir) = test_state_with_cloud("alpha", Cloud::new(Some(config())));
        state.cloud.install(
            &state,
            config(),
            "old@example.com".into(),
            "user-1".into(),
            derive_sync_key("correct horse battery staple", "user-1").unwrap(),
            session(),
        );
        state.cloud.note_unreadable_uploads(7);
        assert_eq!(state.cloud.status().unreadable_uploads, 7);

        state.cloud.install(
            &state,
            config(),
            "new@example.com".into(),
            "user-2".into(),
            derive_sync_key("correct horse battery staple", "user-2").unwrap(),
            Session {
                access_token: "access-2".into(),
                refresh_token: "refresh-2".into(),
                user_id: "user-2".into(),
                expires_at_ms: 1_700_000_000_000,
            },
        );
        assert_eq!(
            state.cloud.status().unreadable_uploads,
            0,
            "the new account inherited the old one's count"
        );
    }

    /// Re-activating the *same* account is not a switch — a token refresh runs
    /// this path — and must not throw away a count the running rounds produced.
    /// The count has to be noted *before* the second activation: noting it
    /// after each one passes even when the activation clears it, which is the
    /// whole regression.
    #[test]
    fn re_activating_the_same_account_keeps_the_unreadable_upload_count() {
        let (state, _dir) = test_state_with_cloud("alpha", Cloud::new(Some(config())));
        let activate = || {
            state.cloud.install(
                &state,
                config(),
                "same@example.com".into(),
                "user-1".into(),
                derive_sync_key("correct horse battery staple", "user-1").unwrap(),
                session(),
            );
        };

        activate();
        state.cloud.note_unreadable_uploads(3);
        activate();

        assert_eq!(state.cloud.status().unreadable_uploads, 3);
    }

    #[test]
    fn a_stale_round_cannot_overwrite_the_replacement_accounts_status() {
        let (state, _dir) = test_state_with_cloud("alpha", Cloud::new(Some(config())));
        state.cloud.install(
            &state,
            config(),
            "old@example.com".into(),
            "user-1".into(),
            derive_sync_key("correct horse battery staple", "user-1").unwrap(),
            session(),
        );
        let stale = state.cloud.driver().unwrap();
        state.cloud.install(
            &state,
            config(),
            "new@example.com".into(),
            "user-2".into(),
            derive_sync_key("correct horse battery staple", "user-2").unwrap(),
            Session {
                access_token: "access-2".into(),
                refresh_token: "refresh-2".into(),
                user_id: "user-2".into(),
                expires_at_ms: 1_700_000_000_000,
            },
        );
        let current = state.cloud.driver().unwrap();
        let current_revision = current.session_revision();
        state
            .cloud
            .note_driver_failure(&current, current_revision, "the current account failed");

        state
            .cloud
            .note_driver_success(&state.meta, &stale, 1_700_000_000_000);

        let status = state.cloud.status();
        assert_eq!(status.email.as_deref(), Some("new@example.com"));
        assert_eq!(status.last_sync_ms, None);
        assert_eq!(
            status.last_error.as_deref(),
            Some("the current account failed")
        );
    }

    #[test]
    fn debug_output_holds_no_account_detail() {
        let (state, _dir) = test_state_with_cloud("alpha", Cloud::new(Some(config())));
        state.cloud.install(
            &state,
            config(),
            "a@example.com".into(),
            "user-1".into(),
            derive_sync_key("correct horse battery staple", "user-1").unwrap(),
            session(),
        );
        let rendered = format!("{:?}", state.cloud);
        for secret in ["a@example.com", "access-1", "refresh-1", "supabase.co"] {
            assert!(!rendered.contains(secret), "{secret} leaked: {rendered}");
        }
    }

    #[test]
    fn the_upload_gate_asks_this_devices_detector() {
        let (state, _dir) = test_state("alpha");
        let guard = sensitive_guard(&state);
        let item = |content: &str| copypaste_cloud::sync::LocalItem {
            item_id: "a".into(),
            content: zeroize::Zeroizing::new(content.as_bytes().to_vec()),
            content_type: "text".into(),
            payload_metadata: None,
            source_app_bundle_id: None,
            source_app_name: None,
            created_at: 1_000,
            deleted: false,
            origin_device_id: "device-a".into(),
        };
        assert!(guard.is_sensitive(&item("AKIAIOSFODNN7EXAMPLE")));
        assert!(!guard.is_sensitive(&item("an ordinary snippet")));
    }
}
