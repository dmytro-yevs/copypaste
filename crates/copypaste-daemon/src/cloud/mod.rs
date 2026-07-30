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

pub mod handlers;
pub mod poll;
pub mod source;

use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use copypaste_cloud::auth::{Session, SupabaseAuth};
use copypaste_cloud::rest::SupabaseRest;
use copypaste_cloud::sync::{CloudSync, SensitiveGuard};
use copypaste_cloud::{CloudConfig, SyncKey};
use copypaste_ipc::CloudStatusData;
use tokio::sync::Notify;
use tracing::{info, warn};

use crate::meta::Meta;
use crate::AppState;

pub use poll::run;

/// The production instantiation of the driver.
pub type Driver = CloudSync<SupabaseRest, SupabaseAuth>;

// Persisted state. The first six are cleared by sign-out; the two cursors are
// not, because they describe this device's position in an account it may sign
// back into.
const KEY_EMAIL: &str = "cloud_email";
const KEY_USER_ID: &str = "cloud_user_id";
const KEY_ACCESS: &str = "cloud_access_token";
const KEY_REFRESH: &str = "cloud_refresh_token";
const KEY_EXPIRES: &str = "cloud_expires_at_ms";
const KEY_SYNC_KEY: &str = "cloud_sync_key";
const CREDENTIAL_KEYS: &[&str] = &[
    KEY_EMAIL,
    KEY_USER_ID,
    KEY_ACCESS,
    KEY_REFRESH,
    KEY_EXPIRES,
    KEY_SYNC_KEY,
];

/// The download cursor: everything this device has reconciled with the account.
pub(crate) const KEY_WATERMARK: &str = "cloud_watermark_ms";
/// The upload floor: everything created before this has been offered for upload.
pub(crate) const KEY_UPLOAD_FLOOR: &str = "cloud_upload_floor_ms";

/// A signed-in account and the driver bound to it.
struct Account {
    email: String,
    user_id: String,
    driver: Arc<Driver>,
}

/// Everything the cloud half of the daemon shares.
pub struct Cloud {
    /// `None` when no deployment is configured. Not a credential — the anon key
    /// is publishable and row-level security is what restricts access.
    config: Option<CloudConfig>,
    account: Mutex<Option<Account>>,
    /// Zero means "never". An `AtomicI64` rather than a field of `Account` so
    /// that `status` can read it without taking the account lock a round may be
    /// holding.
    last_sync_ms: AtomicI64,
    /// Why the last round failed, as a fixed sentence. `SyncError`'s payloads
    /// are `&'static str` by construction, so there is nothing here that could
    /// have interpolated a path or a token.
    last_error: Mutex<Option<&'static str>>,
    /// Woken by sign-in and by `cloud sync`, so neither has to wait out the
    /// idle interval.
    wake: Notify,
}

impl std::fmt::Debug for Cloud {
    /// No URL, no email, no tokens: this type exists to hold credentials.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Cloud")
            .field("configured", &self.config.is_some())
            .field("signed_in", &self.signed_in())
            .finish_non_exhaustive()
    }
}

impl Cloud {
    pub fn new(config: Option<CloudConfig>) -> Self {
        Self {
            config,
            account: Mutex::new(None),
            last_sync_ms: AtomicI64::new(0),
            last_error: Mutex::new(None),
            wake: Notify::new(),
        }
    }

    pub fn is_configured(&self) -> bool {
        self.config.is_some()
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

    pub fn note_success(&self, at_ms: i64) {
        self.last_sync_ms.store(at_ms, Ordering::Release);
        *self.lock_error() = None;
    }

    pub fn note_failure(&self, message: &'static str) {
        *self.lock_error() = Some(message);
    }

    /// Ask the poll loop to run a round now.
    pub fn wake(&self) {
        self.wake.notify_one();
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
            configured: self.config.is_some(),
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
            poll_interval_secs: account
                .as_ref()
                .map_or(poll::SIGNED_OUT_INTERVAL, |a| a.driver.poll_interval())
                .as_secs(),
        }
    }

    /// Re-open the account a previous run signed into.
    ///
    /// Best-effort by design: a missing or unreadable record means signed out,
    /// which is a state the daemon runs in perfectly well. The stored access
    /// token may already have expired — that costs one 401 and one refresh on
    /// the first request, which is the driver's own recovery path, and is
    /// cheaper than refreshing eagerly at every start.
    pub fn restore(&self, state: &AppState) -> bool {
        let Some(config) = self.config.clone() else {
            return false;
        };
        let meta = &state.meta;
        let Some(session) = read_session(meta) else {
            return false;
        };
        let Some(key) = read_key(meta) else {
            warn!("a stored cloud account has no sync key; not signing in");
            return false;
        };
        let (Ok(Some(email)), Ok(Some(user_id))) = (meta.state(KEY_EMAIL), meta.state(KEY_USER_ID))
        else {
            return false;
        };

        self.install(state, config, email, user_id, key, session);
        info!("restored a cloud sync account");
        true
    }

    /// Build a driver and make it the live account.
    pub fn install(
        &self,
        state: &AppState,
        config: CloudConfig,
        email: String,
        user_id: String,
        key: SyncKey,
        session: Session,
    ) {
        let driver = CloudSync::new(
            SupabaseRest::new(config.clone()),
            SupabaseAuth::new(config.clone()),
            key,
            config,
            session,
            sensitive_guard(state),
        );
        *self.lock_account() = Some(Account {
            email,
            user_id,
            driver: Arc::new(driver),
        });
    }

    /// Persist the account, its tokens and its key.
    pub fn persist(&self, meta: &Meta, key_hex: &str) -> Result<(), crate::meta::MetaError> {
        let account = self.lock_account();
        let Some(account) = account.as_ref() else {
            return Ok(());
        };
        meta.set_state(KEY_EMAIL, &account.email)?;
        meta.set_state(KEY_USER_ID, &account.user_id)?;
        meta.set_state(KEY_SYNC_KEY, key_hex)?;
        write_session(meta, &account.driver)
    }

    /// Persist whatever the driver's session has rotated into.
    ///
    /// GoTrue rotates the refresh token on every refresh and retires the old
    /// one, so a round that refreshed and did not write the new token back
    /// leaves the next start presenting a token the server has already killed.
    pub fn persist_session(&self, meta: &Meta) {
        let account = self.lock_account();
        if let Some(account) = account.as_ref() {
            if let Err(e) = write_session(meta, &account.driver) {
                warn!(error = ?e, "could not persist the rotated cloud session");
            }
        }
    }

    /// Forget the account on this device. Keeps the deployment configuration
    /// (manifest 04, `CopyPaste-crh3.100`).
    pub fn sign_out(&self, meta: &Meta) -> Option<Arc<Driver>> {
        let previous = self.lock_account().take();
        if let Err(e) = meta.clear_state(CREDENTIAL_KEYS) {
            warn!(error = ?e, "could not clear the stored cloud account");
        }
        self.last_sync_ms.store(0, Ordering::Release);
        *self.lock_error() = None;
        previous.map(|account| account.driver)
    }

    /// The account this device last synced with, for deciding whether a stored
    /// cursor still means anything.
    pub fn stored_user_id(meta: &Meta) -> Option<String> {
        meta.state(KEY_USER_ID).ok().flatten()
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

    pub fn config(&self) -> Option<&CloudConfig> {
        self.config.as_ref()
    }
}

/// Tell the cloud transport that a version was written *below* its cursor.
///
/// The floor assumes a version's `created_at` is when it appeared here. A peer
/// session breaks that (rows carry the sender's stamp) and so does a delete
/// (`Store::delete` does not restamp). Neither would ever reach the account
/// otherwise. Lowering the floor costs one re-offer of everything above it, and
/// a re-offer is an idempotent upsert.
pub fn note_version_written(state: &AppState, created_at_ms: i64) {
    let meta = &state.meta;
    match meta.state_ms(KEY_UPLOAD_FLOOR) {
        Ok(floor) if floor <= created_at_ms => {}
        Ok(_) => {
            if let Err(e) = meta.set_state_ms(KEY_UPLOAD_FLOOR, created_at_ms) {
                warn!(error = ?e, "could not lower the cloud upload floor");
            }
        }
        Err(e) => warn!(error = ?e, "could not read the cloud upload floor"),
    }
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

fn read_session(meta: &Meta) -> Option<Session> {
    Some(Session {
        access_token: meta.state(KEY_ACCESS).ok()??,
        refresh_token: meta.state(KEY_REFRESH).ok()??,
        user_id: meta.state(KEY_USER_ID).ok()??,
        expires_at_ms: meta.state_ms(KEY_EXPIRES).ok()?,
    })
}

fn write_session(meta: &Meta, driver: &Driver) -> Result<(), crate::meta::MetaError> {
    driver.inspect_session(|session| {
        meta.set_state(KEY_ACCESS, &session.access_token)?;
        meta.set_state(KEY_REFRESH, &session.refresh_token)?;
        meta.set_state_ms(KEY_EXPIRES, session.expires_at_ms)
    })
}

fn read_key(meta: &Meta) -> Option<SyncKey> {
    let raw = meta.state(KEY_SYNC_KEY).ok()??;
    let bytes = hex::decode(raw).ok()?;
    let bytes: [u8; 32] = bytes.try_into().ok()?;
    Some(SyncKey::from_bytes(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testutil::{test_state, test_state_with_cloud};
    use copypaste_cloud::crypto::derive_sync_key;

    fn config() -> CloudConfig {
        CloudConfig {
            url: "https://example.supabase.co".into(),
            anon_key: "anon".into(),
        }
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
        state
            .cloud
            .persist(&state.meta, &hex::encode(key.to_bytes().as_slice()))
            .unwrap();
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
        state2.cloud.sign_out(&state2.meta);
        assert!(!state2.cloud.signed_in());
        assert!(!state2.cloud.restore(&state2));
        assert_eq!(state2.meta.state(KEY_REFRESH).unwrap(), None);
        assert_eq!(state2.meta.state(KEY_SYNC_KEY).unwrap(), None);
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
            content: content.as_bytes().to_vec(),
            content_type: "text".into(),
            created_at: 1_000,
            deleted: false,
            origin_device_id: "device-a".into(),
        };
        assert!(guard.is_sensitive(&item("AKIAIOSFODNN7EXAMPLE")));
        assert!(!guard.is_sensitive(&item("an ordinary snippet")));
    }
}
