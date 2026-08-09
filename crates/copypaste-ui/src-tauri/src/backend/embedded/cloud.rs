mod account;
mod credentials;
mod cursor;
mod schedule;
mod source;

use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

use copypaste_cloud::auth::{AuthError, Session, SupabaseAuth};
use copypaste_cloud::crypto::derive_sync_key;
use copypaste_cloud::rest::SupabaseRest;
use copypaste_cloud::sync::{CloudSync, SensitiveGuard, SyncError};
use copypaste_cloud::{CloudConfig, SyncKey};
use copypaste_ipc::{CloudStatusData, CloudSyncData, ErrorCode};
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;
use zeroize::Zeroizing;

use super::open::Inner;
use super::{BackendError, Result};
use account::{Account, AccountSlot};
use credentials::{
    read_key, read_session, write_session, CREDENTIAL_KEYS, KEY_EMAIL, KEY_LAST_SYNC, KEY_SYNC_KEY,
    KEY_USER_ID,
};
use cursor::{UploadCursor, UploadFloor};
use schedule::until_cancelled;
use source::StoreSource;

type Driver = CloudSync<SupabaseRest, SupabaseAuth>;

pub(super) const KEY_WATERMARK: &str = "cloud_watermark_ms";
pub(super) const KEY_WATERMARK_ITEM: &str = "cloud_watermark_item_id";
pub(super) const KEY_UPLOAD_FLOOR: &str = "cloud_upload_floor_ms";
pub(super) const KEY_UPLOAD_FLOOR_ITEM: &str = "cloud_upload_floor_item_id";

const MSG_NOT_CONFIGURED: &str = "Cloud sync is not configured in this build.";
const MSG_SIGNED_OUT: &str = "Sign in before syncing.";
const MSG_REJECTED: &str = "The email address or password was not accepted.";
const MSG_CONFIRM: &str = "Confirm the email address before signing in.";
const MSG_UNAVAILABLE: &str = "The account service could not be reached.";
const MSG_PASSPHRASE: &str = "The sync passphrase is too short.";
const MSG_STORE: &str = "The sync account could not be stored.";

pub(super) struct EmbeddedCloud {
    config: Option<CloudConfig>,
    account: AccountSlot,
    last_sync_ms: AtomicI64,
    last_error: Mutex<Option<&'static str>>,
    upload_cursor: UploadCursor,
    wake: Notify,
    poller_started: AtomicBool,
    shutdown: CancellationToken,
}

impl EmbeddedCloud {
    pub(super) fn open(state: &super::state::BackendState) -> Self {
        let cloud = Self {
            config: cloud_config(),
            account: AccountSlot::default(),
            last_sync_ms: AtomicI64::new(0),
            last_error: Mutex::new(None),
            upload_cursor: UploadCursor::new(),
            wake: Notify::new(),
            poller_started: AtomicBool::new(false),
            shutdown: CancellationToken::new(),
        };
        cloud.restore(state);
        cloud
    }

    pub(super) fn status(&self) -> CloudStatusData {
        let account = self.account();
        let last_sync = self.last_sync_ms.load(Ordering::Acquire);
        CloudStatusData {
            configured: self.config.is_some(),
            signed_in: account.is_some(),
            key_ready: account.is_some(),
            email: account.as_ref().map(|value| value.email.clone()),
            last_sync_ms: (last_sync > 0).then_some(last_sync),
            last_error: self.error().map(str::to_string),
            poll_interval_secs: account
                .as_ref()
                .map_or(60, |value| value.driver.poll_interval().as_secs()),
        }
    }

    pub(super) fn ensure_poller(&self, inner: &Arc<Inner>) {
        if self.poller_started.swap(true, Ordering::AcqRel) {
            return;
        }
        let inner = Arc::downgrade(inner);
        let shutdown = self.shutdown.clone();
        tokio::spawn(async move { schedule::poll(inner, shutdown).await });
    }

    pub(super) async fn sign_in(
        &self,
        inner: &Arc<Inner>,
        email: &str,
        password: &str,
        passphrase: &str,
    ) -> Result<CloudStatusData> {
        let config = self
            .config
            .clone()
            .ok_or(BackendError::Unsupported(MSG_NOT_CONFIGURED))?;
        let email = email.trim().to_string();
        if email.is_empty() {
            return Err(BackendError::Invalid("An email address is required."));
        }
        let session = SupabaseAuth::new(config.clone())
            .sign_in(&email, password)
            .await
            .map_err(auth_error)?;
        let user_id = session.user_id.clone();
        let phrase = Zeroizing::new(passphrase.to_string());
        let key_account = user_id.clone();
        let key = tokio::task::spawn_blocking(move || derive_sync_key(&phrase, &key_account))
            .await
            .map_err(|_| BackendError::internal(MSG_PASSPHRASE))?
            .map_err(|_| BackendError::Invalid(MSG_PASSPHRASE))?;

        self.account.cancel();
        let previous = inner.state.store.state(KEY_USER_ID).ok().flatten();
        let switched = previous.as_deref() != Some(&user_id);
        if switched
            && inner
                .state
                .store
                .set_state_all(&[(KEY_WATERMARK, "0"), (KEY_WATERMARK_ITEM, "")])
                .is_err()
        {
            self.clear_local(&inner.state.store);
            return Err(BackendError::internal(MSG_STORE));
        }
        if self.upload_cursor.reset(&inner.state.store).is_err() {
            self.clear_local(&inner.state.store);
            return Err(BackendError::internal(MSG_STORE));
        }
        let key_hex = Zeroizing::new(hex::encode(key.to_bytes()));
        let driver = Arc::new(make_driver(inner, config, key, session));
        self.account.install(Account {
            email,
            user_id,
            driver: Arc::clone(&driver),
            cancel: CancellationToken::new(),
        });
        if switched {
            self.last_sync_ms.store(0, Ordering::Release);
            let _ = inner.state.store.clear_state(&[KEY_LAST_SYNC]);
        }
        *self.error() = None;
        if self.persist(&inner.state.store, &key_hex).is_err() {
            self.clear_local(&inner.state.store);
            return Err(BackendError::internal(MSG_STORE));
        }
        self.ensure_poller(inner);
        self.wake.notify_one();
        Ok(self.status())
    }

    pub(super) async fn sign_out(&self, inner: &Arc<Inner>) {
        let account = self.account.take();
        self.clear_local(&inner.state.store);
        if let (Some(account), Some(config)) = (account, self.config.clone()) {
            let token = account
                .driver
                .inspect_session(|session| session.access_token.clone());
            if let Err(error) = SupabaseAuth::new(config).sign_out(&token).await {
                tracing::warn!(?error, "cloud session revocation failed");
            }
        }
        self.wake.notify_one();
    }

    pub(super) async fn sync_now(&self, inner: &Arc<Inner>) -> Result<CloudSyncData> {
        if !inner.settings().sync_enabled {
            return Err(BackendError::NotReady);
        }
        if self.config.is_none() {
            return Err(BackendError::Unsupported(MSG_NOT_CONFIGURED));
        }
        if self.shutdown.is_cancelled() {
            return Err(BackendError::NotReady);
        }
        let (driver, cancel) = self.account.round().ok_or_else(|| {
            BackendError::from_code(Some(ErrorCode::AuthFailed), None, Some(MSG_SIGNED_OUT))
        })?;
        let source = StoreSource::for_round(inner, &driver, &cancel);
        let started = copypaste_core::now_ms();
        let Some(outcome) = until_cancelled(&cancel, driver.sync(&source)).await else {
            return Err(BackendError::NotReady);
        };
        self.persist_session(&inner.state.store, &driver);
        match outcome {
            Ok(stats) => {
                let completed = copypaste_core::now_ms();
                self.note_success(&inner.state.store, &driver, completed);
                let _ = source.commit_upload_floor(started);
                if stats.applied > 0 {
                    inner.publish_items(false, 0);
                }
                Ok(to_wire(stats))
            }
            Err(error) => {
                let message = describe(&error);
                self.record_failure(
                    &inner.state.store,
                    &driver,
                    message,
                    terminal_auth_error(&error),
                );
                Err(BackendError::from_code(
                    Some(if terminal_auth_error(&error) {
                        ErrorCode::AuthFailed
                    } else {
                        ErrorCode::Internal
                    }),
                    None,
                    Some(message),
                ))
            }
        }
    }

    fn restore(&self, state: &super::state::BackendState) {
        let Some(config) = self.config.clone() else {
            return;
        };
        let store = &state.store;
        let Some(session) = read_session(store) else {
            return;
        };
        let Some(key) = read_key(store) else {
            return;
        };
        let (Ok(Some(email)), Ok(Some(user_id))) =
            (store.state(KEY_EMAIL), store.state(KEY_USER_ID))
        else {
            return;
        };
        let guard = sensitive_guard(&state.detector);
        let driver = CloudSync::new(
            SupabaseRest::new(config.clone()),
            SupabaseAuth::new(config.clone()),
            key,
            config,
            session,
            guard,
        );
        self.account.install(Account {
            email,
            user_id,
            driver: Arc::new(driver),
            cancel: CancellationToken::new(),
        });
        if let Ok(ms) = store.state_ms(KEY_LAST_SYNC) {
            self.last_sync_ms.store(ms, Ordering::Release);
        }
    }

    fn persist(
        &self,
        store: &copypaste_core::Store,
        key_hex: &str,
    ) -> std::result::Result<(), copypaste_core::StoreError> {
        let account = self.account();
        let Some(account) = account.as_ref() else {
            return Ok(());
        };
        store.set_state_all(&[
            (KEY_EMAIL, &account.email),
            (KEY_USER_ID, &account.user_id),
            (KEY_SYNC_KEY, key_hex),
        ])?;
        write_session(store, &account.driver)
    }

    fn persist_session(&self, store: &copypaste_core::Store, expected: &Arc<Driver>) {
        let account = self.account();
        if let Some(account) = account
            .as_ref()
            .filter(|value| Arc::ptr_eq(&value.driver, expected))
        {
            if let Err(error) = write_session(store, &account.driver) {
                tracing::warn!(?error, "rotated cloud session was not persisted");
            }
        }
    }

    fn note_success(&self, store: &copypaste_core::Store, expected: &Arc<Driver>, completed: i64) {
        let account = self.account();
        if account
            .as_ref()
            .is_some_and(|value| Arc::ptr_eq(&value.driver, expected))
        {
            self.last_sync_ms.store(completed, Ordering::Release);
            *self.error() = None;
            let _ = store.set_state_ms(KEY_LAST_SYNC, completed);
        }
    }

    fn record_failure(
        &self,
        store: &copypaste_core::Store,
        expected: &Arc<Driver>,
        message: &'static str,
        terminal: bool,
    ) {
        let mut account = self.account();
        if !account
            .as_ref()
            .is_some_and(|value| Arc::ptr_eq(&value.driver, expected))
        {
            return;
        }
        if terminal {
            if let Some(expired) = account.take() {
                expired.cancel.cancel();
            }
            let _ = store.clear_state(CREDENTIAL_KEYS);
            self.last_sync_ms.store(0, Ordering::Release);
        }
        *self.error() = Some(message);
    }

    fn clear_local(&self, store: &copypaste_core::Store) {
        self.account.take();
        let _ = store.clear_state(CREDENTIAL_KEYS);
        self.last_sync_ms.store(0, Ordering::Release);
        *self.error() = None;
    }

    fn driver(&self) -> Option<Arc<Driver>> {
        self.account.driver()
    }

    fn with_driver<T>(
        &self,
        expected: &Arc<Driver>,
        cancel: &CancellationToken,
        f: impl FnOnce() -> T,
    ) -> Option<T> {
        self.account.with_driver(expected, cancel, f)
    }
    fn account(&self) -> MutexGuard<'_, Option<Account>> {
        self.account.lock()
    }
    fn error(&self) -> MutexGuard<'_, Option<&'static str>> {
        self.last_error
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }

    pub(super) fn note_version_written(&self, inner: &Inner, created_at: i64) {
        self.upload_cursor
            .note_version_written(&inner.state.store, created_at);
    }

    pub(super) fn sync_enabled_changed(&self, enabled: bool) {
        if enabled {
            self.wake.notify_one();
        } else {
            self.account.interrupt();
        }
    }

    pub(super) fn shutdown(&self) {
        self.account.cancel();
        self.shutdown.cancel();
        self.wake.notify_one();
    }

    fn upload_floor_epoch(&self) -> u64 {
        self.upload_cursor.epoch()
    }

    fn commit_upload_floor(
        &self,
        inner: &Inner,
        started: &UploadFloor,
        started_epoch: u64,
        candidate: &UploadFloor,
    ) -> std::result::Result<(), copypaste_core::StoreError> {
        self.upload_cursor
            .commit(&inner.state.store, started, started_epoch, candidate)
    }
}

fn cloud_config() -> Option<CloudConfig> {
    fn value(build: Option<&str>, runtime: &str) -> Option<String> {
        build
            .map(str::to_owned)
            .or_else(|| std::env::var(runtime).ok())
            .map(|v| v.trim().to_string())
            .filter(|v| !v.is_empty())
    }
    Some(CloudConfig {
        url: value(option_env!("COPYPASTE_CLOUD_URL"), "COPYPASTE_CLOUD_URL")?,
        anon_key: value(
            option_env!("COPYPASTE_CLOUD_ANON_KEY"),
            "COPYPASTE_CLOUD_ANON_KEY",
        )?,
    })
}

fn make_driver(inner: &Arc<Inner>, config: CloudConfig, key: SyncKey, session: Session) -> Driver {
    CloudSync::new(
        SupabaseRest::new(config.clone()),
        SupabaseAuth::new(config.clone()),
        key,
        config,
        session,
        sensitive_guard(&inner.state.detector),
    )
}

fn sensitive_guard(detector: &Arc<copypaste_core::Detector>) -> SensitiveGuard {
    let detector = Arc::clone(detector);
    SensitiveGuard::new(move |item| {
        std::str::from_utf8(&item.content).is_ok_and(|text| detector.is_sensitive(text))
    })
}

fn auth_error(error: AuthError) -> BackendError {
    match error {
        AuthError::InvalidCredentials | AuthError::SessionExpired => {
            BackendError::from_code(Some(ErrorCode::AuthFailed), None, Some(MSG_REJECTED))
        }
        AuthError::EmailConfirmationRequired => {
            BackendError::from_code(Some(ErrorCode::AuthFailed), None, Some(MSG_CONFIRM))
        }
        _ => BackendError::internal(MSG_UNAVAILABLE),
    }
}
fn terminal_auth_error(error: &SyncError) -> bool {
    matches!(
        error,
        SyncError::InvalidCredentials | SyncError::SessionExpired
    )
}
fn describe(error: &SyncError) -> &'static str {
    match error {
        SyncError::Source(_) => "The local history could not be read.",
        SyncError::Encrypt => "An item could not be encrypted for upload.",
        SyncError::Unauthorized | SyncError::InvalidCredentials => {
            "The stored account credentials were rejected; sign in again."
        }
        SyncError::SessionExpired => "The session expired; sign in again.",
        SyncError::RateLimited => "The backend is rate limiting this account.",
        SyncError::Transport(_) => "The sync backend could not be reached.",
    }
}
fn to_wire(stats: copypaste_cloud::SyncStats) -> CloudSyncData {
    let n = |v| u32::try_from(v).unwrap_or(u32::MAX);
    CloudSyncData {
        uploaded: n(stats.uploaded),
        tombstoned: n(stats.tombstoned),
        downloaded: n(stats.downloaded),
        applied: n(stats.applied),
        skipped_sensitive: n(stats.skipped_sensitive),
        skipped_undecryptable: n(stats.skipped_undecryptable),
        skipped_forged: n(stats.skipped_forged),
        skipped_future: n(stats.skipped_future),
        skipped_too_large: n(stats.skipped_too_large),
    }
}

#[cfg(test)]
mod tests {
    use super::credentials::KEY_ACCESS;
    use super::*;

    fn configured() -> EmbeddedCloud {
        EmbeddedCloud {
            config: Some(CloudConfig {
                url: "https://example.invalid".into(),
                anon_key: "public-anon".into(),
            }),
            account: AccountSlot::default(),
            last_sync_ms: AtomicI64::new(0),
            last_error: Mutex::new(None),
            upload_cursor: UploadCursor::new(),
            wake: Notify::new(),
            poller_started: AtomicBool::new(false),
            shutdown: CancellationToken::new(),
        }
    }

    fn driver(
        cloud: &EmbeddedCloud,
        state: &super::super::state::BackendState,
        token: &str,
    ) -> Arc<Driver> {
        let config = cloud.config.clone().unwrap();
        Arc::new(CloudSync::new(
            SupabaseRest::new(config.clone()),
            SupabaseAuth::new(config.clone()),
            SyncKey::from_bytes([9; 32]),
            config,
            Session {
                access_token: token.into(),
                refresh_token: format!("refresh-{token}"),
                user_id: "user-1".into(),
                expires_at_ms: 123_000,
            },
            sensitive_guard(&state.detector),
        ))
    }

    #[test]
    fn encrypted_store_restores_a_complete_session_and_key() {
        let dir = tempfile::TempDir::new().unwrap();
        let state = super::super::state::BackendState::open(dir.path()).unwrap();
        let cloud = configured();
        let session = Session {
            access_token: "access".into(),
            refresh_token: "refresh".into(),
            user_id: "user-1".into(),
            expires_at_ms: 123_000,
        };
        let key = SyncKey::from_bytes([9; 32]);
        let config = cloud.config.clone().unwrap();
        let driver = Arc::new(CloudSync::new(
            SupabaseRest::new(config.clone()),
            SupabaseAuth::new(config.clone()),
            key,
            config,
            session,
            sensitive_guard(&state.detector),
        ));
        *cloud.account() = Some(Account {
            email: "a@example.com".into(),
            user_id: "user-1".into(),
            driver,
            cancel: CancellationToken::new(),
        });
        cloud.persist(&state.store, &hex::encode([9; 32])).unwrap();

        let restarted = configured();
        restarted.restore(&state);
        let status = restarted.status();
        assert!(status.signed_in && status.key_ready);
        assert_eq!(status.email.as_deref(), Some("a@example.com"));
    }

    #[test]
    fn incomplete_persisted_credentials_fail_closed_to_signed_out() {
        let dir = tempfile::TempDir::new().unwrap();
        let state = super::super::state::BackendState::open(dir.path()).unwrap();
        state
            .store
            .set_state_all(&[(KEY_EMAIL, "a@example.com"), (KEY_ACCESS, "access")])
            .unwrap();
        let cloud = configured();
        cloud.restore(&state);
        assert!(!cloud.status().signed_in);
        assert!(!cloud.status().key_ready);
    }

    #[test]
    fn local_sign_out_clears_every_credential() {
        let dir = tempfile::TempDir::new().unwrap();
        let state = super::super::state::BackendState::open(dir.path()).unwrap();
        for key in CREDENTIAL_KEYS {
            state.store.set_state(key, "secret").unwrap();
        }
        let cloud = configured();
        cloud.clear_local(&state.store);
        for key in CREDENTIAL_KEYS {
            assert_eq!(state.store.state(key).unwrap(), None);
        }
    }

    #[test]
    fn terminal_failure_signs_out_but_preserves_the_status_error() {
        let dir = tempfile::TempDir::new().unwrap();
        let state = super::super::state::BackendState::open(dir.path()).unwrap();
        let cloud = configured();
        let driver = driver(&cloud, &state, "access");
        *cloud.account() = Some(Account {
            email: "a@example.com".into(),
            user_id: "user-1".into(),
            driver: Arc::clone(&driver),
            cancel: CancellationToken::new(),
        });
        state.store.set_state(KEY_ACCESS, "secret").unwrap();

        cloud.record_failure(&state.store, &driver, MSG_REJECTED, true);

        let status = cloud.status();
        assert!(!status.signed_in);
        assert_eq!(status.last_error.as_deref(), Some(MSG_REJECTED));
        assert_eq!(state.store.state(KEY_ACCESS).unwrap(), None);
    }

    #[test]
    fn a_stale_round_cannot_overwrite_the_replacement_accounts_status() {
        let dir = tempfile::TempDir::new().unwrap();
        let state = super::super::state::BackendState::open(dir.path()).unwrap();
        let cloud = configured();
        let stale = driver(&cloud, &state, "stale");
        let current = driver(&cloud, &state, "current");
        *cloud.account() = Some(Account {
            email: "new@example.com".into(),
            user_id: "user-1".into(),
            driver: current,
            cancel: CancellationToken::new(),
        });

        cloud.record_failure(&state.store, &stale, MSG_UNAVAILABLE, true);
        cloud.note_success(&state.store, &stale, 999);

        let status = cloud.status();
        assert!(status.signed_in);
        assert_eq!(status.email.as_deref(), Some("new@example.com"));
        assert_eq!(status.last_error, None);
        assert_eq!(status.last_sync_ms, None);
    }

    #[tokio::test]
    async fn signing_out_cancels_an_in_flight_round() {
        let dir = tempfile::TempDir::new().unwrap();
        let state = super::super::state::BackendState::open(dir.path()).unwrap();
        let cloud = configured();
        let driver = driver(&cloud, &state, "access");
        let cancel = CancellationToken::new();
        cloud.account.install(Account {
            email: "a@example.com".into(),
            user_id: "user-1".into(),
            driver,
            cancel: cancel.clone(),
        });

        cloud.clear_local(&state.store);

        assert!(cancel.is_cancelled());
        assert_eq!(
            until_cancelled(&cancel, std::future::pending::<()>()).await,
            None
        );
    }

    #[test]
    fn disabling_sync_interrupts_a_round_without_signing_out() {
        let dir = tempfile::TempDir::new().unwrap();
        let state = super::super::state::BackendState::open(dir.path()).unwrap();
        let cloud = configured();
        let driver = driver(&cloud, &state, "access");
        let previous = CancellationToken::new();
        cloud.account.install(Account {
            email: "a@example.com".into(),
            user_id: "user-1".into(),
            driver,
            cancel: previous.clone(),
        });

        cloud.sync_enabled_changed(false);

        assert!(previous.is_cancelled());
        assert!(cloud.status().signed_in);
        assert!(!cloud.account.round().unwrap().1.is_cancelled());
    }

    #[test]
    fn shutdown_cancels_the_engine_and_current_round() {
        let dir = tempfile::TempDir::new().unwrap();
        let state = super::super::state::BackendState::open(dir.path()).unwrap();
        let cloud = configured();
        let driver = driver(&cloud, &state, "access");
        let cancel = CancellationToken::new();
        cloud.account.install(Account {
            email: "a@example.com".into(),
            user_id: "user-1".into(),
            driver,
            cancel: cancel.clone(),
        });

        cloud.shutdown();

        assert!(cancel.is_cancelled());
        assert!(cloud.shutdown.is_cancelled());
    }
}
