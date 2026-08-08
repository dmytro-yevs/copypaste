mod cursor;
mod source;

use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, Weak};

use copypaste_cloud::auth::{AuthError, Session, SupabaseAuth};
use copypaste_cloud::crypto::derive_sync_key;
use copypaste_cloud::rest::SupabaseRest;
use copypaste_cloud::sync::{CloudSync, SensitiveGuard, SyncError};
use copypaste_cloud::{CloudConfig, SyncKey};
use copypaste_ipc::{CloudStatusData, CloudSyncData, ErrorCode};
use tokio::sync::Notify;
use zeroize::Zeroizing;

use super::open::Inner;
use super::{BackendError, Result};
use cursor::{UploadCursor, UploadFloor};
use source::StoreSource;

type Driver = CloudSync<SupabaseRest, SupabaseAuth>;

const KEY_EMAIL: &str = "cloud_email";
const KEY_USER_ID: &str = "cloud_user_id";
const KEY_ACCESS: &str = "cloud_access_token";
const KEY_REFRESH: &str = "cloud_refresh_token";
const KEY_EXPIRES: &str = "cloud_expires_at_ms";
const KEY_SYNC_KEY: &str = "cloud_sync_key";
const KEY_LAST_SYNC: &str = "cloud_last_sync_ms";
pub(super) const KEY_WATERMARK: &str = "cloud_watermark_ms";
pub(super) const KEY_WATERMARK_ITEM: &str = "cloud_watermark_item_id";
pub(super) const KEY_UPLOAD_FLOOR: &str = "cloud_upload_floor_ms";
pub(super) const KEY_UPLOAD_FLOOR_ITEM: &str = "cloud_upload_floor_item_id";
const CREDENTIAL_KEYS: &[&str] = &[
    KEY_EMAIL,
    KEY_USER_ID,
    KEY_ACCESS,
    KEY_REFRESH,
    KEY_EXPIRES,
    KEY_SYNC_KEY,
    KEY_LAST_SYNC,
];

const MSG_NOT_CONFIGURED: &str = "Cloud sync is not configured in this build.";
const MSG_SIGNED_OUT: &str = "Sign in before syncing.";
const MSG_REJECTED: &str = "The email address or password was not accepted.";
const MSG_CONFIRM: &str = "Confirm the email address before signing in.";
const MSG_UNAVAILABLE: &str = "The account service could not be reached.";
const MSG_PASSPHRASE: &str = "The sync passphrase is too short.";
const MSG_STORE: &str = "The sync account could not be stored.";

struct Account {
    email: String,
    user_id: String,
    driver: Arc<Driver>,
}

pub(super) struct EmbeddedCloud {
    config: Option<CloudConfig>,
    account: Mutex<Option<Account>>,
    last_sync_ms: AtomicI64,
    last_error: Mutex<Option<&'static str>>,
    upload_cursor: UploadCursor,
    wake: Notify,
    poller_started: AtomicBool,
}

impl EmbeddedCloud {
    pub(super) fn open(state: &super::state::BackendState) -> Self {
        let cloud = Self {
            config: cloud_config(),
            account: Mutex::new(None),
            last_sync_ms: AtomicI64::new(0),
            last_error: Mutex::new(None),
            upload_cursor: UploadCursor::new(),
            wake: Notify::new(),
            poller_started: AtomicBool::new(false),
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
        tokio::spawn(async move { poll(inner).await });
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

        let previous = inner.state.store.state(KEY_USER_ID).ok().flatten();
        if previous.as_deref() != Some(&user_id) {
            inner
                .state
                .store
                .set_state_all(&[(KEY_WATERMARK, "0"), (KEY_WATERMARK_ITEM, "")])
                .map_err(|_| BackendError::internal(MSG_STORE))?;
        }
        self.upload_cursor
            .reset(&inner.state.store)
            .map_err(|_| BackendError::internal(MSG_STORE))?;
        let key_hex = Zeroizing::new(hex::encode(key.to_bytes()));
        let driver = Arc::new(make_driver(inner, config, key, session));
        *self.account() = Some(Account {
            email,
            user_id,
            driver,
        });
        if self.persist(&inner.state.store, &key_hex).is_err() {
            self.clear_local(&inner.state.store);
            return Err(BackendError::internal(MSG_STORE));
        }
        self.ensure_poller(inner);
        self.wake.notify_one();
        Ok(self.status())
    }

    pub(super) async fn sign_out(&self, inner: &Arc<Inner>) {
        let account = self.account().take();
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
        let driver = self.driver().ok_or_else(|| {
            BackendError::from_code(Some(ErrorCode::AuthFailed), None, Some(MSG_SIGNED_OUT))
        })?;
        let source = StoreSource::new(inner);
        let started = copypaste_core::now_ms();
        let outcome = driver.sync(&source).await;
        self.persist_session(&inner.state.store, &driver);
        match outcome {
            Ok(stats) => {
                let completed = copypaste_core::now_ms();
                self.last_sync_ms.store(completed, Ordering::Release);
                *self.error() = None;
                let _ = inner.state.store.set_state_ms(KEY_LAST_SYNC, completed);
                let _ = source.commit_upload_floor(started);
                Ok(to_wire(stats))
            }
            Err(error) => {
                let message = describe(&error);
                *self.error() = Some(message);
                if terminal_auth_error(&error) {
                    self.clear_local(&inner.state.store);
                }
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
        *self.account() = Some(Account {
            email,
            user_id,
            driver: Arc::new(driver),
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

    fn clear_local(&self, store: &copypaste_core::Store) {
        *self.account() = None;
        let _ = store.clear_state(CREDENTIAL_KEYS);
        self.last_sync_ms.store(0, Ordering::Release);
        *self.error() = None;
    }

    fn driver(&self) -> Option<Arc<Driver>> {
        self.account()
            .as_ref()
            .map(|value| Arc::clone(&value.driver))
    }
    fn account(&self) -> MutexGuard<'_, Option<Account>> {
        self.account
            .lock()
            .unwrap_or_else(|error| error.into_inner())
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

async fn poll(inner: Weak<Inner>) {
    loop {
        let Some(owner) = inner.upgrade() else {
            break;
        };
        let wait = owner
            .cloud
            .driver()
            .map_or(std::time::Duration::from_secs(60), |driver| {
                driver.poll_interval()
            });
        tokio::select! { _ = owner.cloud.wake.notified() => {}, _ = tokio::time::sleep(wait) => {} }
        drop(owner);
        let Some(owner) = inner.upgrade() else {
            break;
        };
        if owner.settings().sync_enabled {
            let _ = owner.cloud.sync_now(&owner).await;
        }
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

fn read_session(store: &copypaste_core::Store) -> Option<Session> {
    Some(Session {
        access_token: store.state(KEY_ACCESS).ok()??,
        refresh_token: store.state(KEY_REFRESH).ok()??,
        user_id: store.state(KEY_USER_ID).ok()??,
        expires_at_ms: store.state_ms(KEY_EXPIRES).ok()?,
    })
}
fn read_key(store: &copypaste_core::Store) -> Option<SyncKey> {
    let bytes: [u8; 32] = hex::decode(store.state(KEY_SYNC_KEY).ok()??)
        .ok()?
        .try_into()
        .ok()?;
    Some(SyncKey::from_bytes(bytes))
}
fn write_session(
    store: &copypaste_core::Store,
    driver: &Driver,
) -> std::result::Result<(), copypaste_core::StoreError> {
    driver.inspect_session(|session| {
        store.set_state_all(&[
            (KEY_ACCESS, &session.access_token),
            (KEY_REFRESH, &session.refresh_token),
            (KEY_EXPIRES, &session.expires_at_ms.to_string()),
        ])
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
        SyncError::Unauthorized | SyncError::InvalidCredentials | SyncError::SessionExpired
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
    use super::*;

    fn configured() -> EmbeddedCloud {
        EmbeddedCloud {
            config: Some(CloudConfig {
                url: "https://example.invalid".into(),
                anon_key: "public-anon".into(),
            }),
            account: Mutex::new(None),
            last_sync_ms: AtomicI64::new(0),
            last_error: Mutex::new(None),
            upload_cursor: UploadCursor::new(),
            wake: Notify::new(),
            poller_started: AtomicBool::new(false),
        }
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
}
