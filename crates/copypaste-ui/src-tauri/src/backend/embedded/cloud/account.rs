use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, MutexGuard};

use copypaste_cloud::auth::Session;
use copypaste_cloud::SyncKey;
use copypaste_core::{Store, StoreError};
use tokio_util::sync::CancellationToken;

use super::{Driver, EmbeddedCloud};

pub(super) const KEY_EMAIL: &str = "cloud_email";
pub(super) const KEY_USER_ID: &str = "cloud_user_id";
pub(super) const KEY_ACCESS: &str = "cloud_access_token";
pub(super) const KEY_REFRESH: &str = "cloud_refresh_token";
pub(super) const KEY_EXPIRES: &str = "cloud_expires_at_ms";
pub(super) const KEY_SYNC_KEY: &str = "cloud_sync_key";
const KEY_SESSION_USER_ID: &str = "cloud_session_user_id";
const KEY_SYNC_KEY_USER_ID: &str = "cloud_sync_key_user_id";
pub(super) const KEY_CURSOR_USER_ID: &str = "cloud_cursor_user_id";
pub(super) const KEY_LAST_SYNC: &str = "cloud_last_sync_ms";
pub(super) const CREDENTIAL_KEYS: &[&str] = &[
    KEY_EMAIL,
    KEY_USER_ID,
    KEY_ACCESS,
    KEY_REFRESH,
    KEY_EXPIRES,
    KEY_SYNC_KEY,
    KEY_SESSION_USER_ID,
    KEY_SYNC_KEY_USER_ID,
    KEY_LAST_SYNC,
];
pub(super) const MSG_ACCOUNT_CHANGED: &str = "The sync account changed during this operation.";

pub(super) struct Account {
    pub(super) email: String,
    pub(super) user_id: String,
    pub(super) driver: Arc<Driver>,
    pub(super) cancel: CancellationToken,
}

pub(super) struct AccountRound {
    pub(super) driver: Arc<Driver>,
    pub(super) cancel: CancellationToken,
}

#[derive(Default)]
pub(super) struct AccountSlot(Mutex<Option<Account>>);

impl AccountSlot {
    pub(super) fn lock(&self) -> MutexGuard<'_, Option<Account>> {
        self.0.lock().unwrap_or_else(|error| error.into_inner())
    }

    #[cfg(test)]
    pub(super) fn round(&self) -> Option<(Arc<Driver>, CancellationToken)> {
        self.lock()
            .as_ref()
            .map(|value| (Arc::clone(&value.driver), value.cancel.clone()))
    }

    #[cfg(test)]
    pub(super) fn install(&self, account: Account) {
        let previous = self.lock().replace(account);
        if let Some(previous) = previous {
            previous.cancel.cancel();
        }
    }

    pub(super) fn cancel(&self) {
        if let Some(account) = self.lock().as_ref() {
            account.cancel.cancel();
        }
    }

    pub(super) fn interrupt(&self) {
        if let Some(account) = self.lock().as_mut() {
            account.cancel.cancel();
            account.cancel = CancellationToken::new();
        }
    }

    pub(super) fn with_driver<T>(
        &self,
        expected: &Arc<Driver>,
        cancel: &CancellationToken,
        f: impl FnOnce() -> T,
    ) -> Option<T> {
        let account = self.lock();
        account
            .as_ref()
            .filter(|value| Arc::ptr_eq(&value.driver, expected) && !cancel.is_cancelled())?;
        Some(f())
    }
}

#[derive(Clone, Copy)]
pub(super) struct SignInAttempt(u64);

#[derive(Debug)]
pub(super) enum ActivateError {
    Stale,
    Store(StoreError),
    AccountMismatch,
}

impl From<StoreError> for ActivateError {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}

impl EmbeddedCloud {
    pub(super) fn begin_sign_in(&self) -> SignInAttempt {
        let _account = self.account();
        SignInAttempt(
            self.account_revision
                .fetch_add(1, Ordering::AcqRel)
                .wrapping_add(1),
        )
    }

    pub(super) fn activate(
        &self,
        store: &Store,
        attempt: SignInAttempt,
        email: String,
        user_id: String,
        driver: Arc<Driver>,
        key_hex: &str,
    ) -> std::result::Result<bool, ActivateError> {
        if !driver.inspect_session(|session| session.user_id == user_id) {
            return Err(ActivateError::AccountMismatch);
        }
        let mut account = self.account();
        if self.account_revision.load(Ordering::Acquire) != attempt.0 {
            return Err(ActivateError::Stale);
        }
        let switched = store.state(KEY_CURSOR_USER_ID)?.as_deref() != Some(&user_id);
        let expires = driver.inspect_session(|session| session.expires_at_ms.to_string());
        let (access, refresh) = driver.inspect_session(|session| {
            (session.access_token.clone(), session.refresh_token.clone())
        });
        let mut entries = vec![
            (KEY_EMAIL, email.as_str()),
            (KEY_USER_ID, user_id.as_str()),
            (KEY_ACCESS, access.as_str()),
            (KEY_REFRESH, refresh.as_str()),
            (KEY_EXPIRES, expires.as_str()),
            (KEY_SYNC_KEY, key_hex),
            (KEY_SESSION_USER_ID, user_id.as_str()),
            (KEY_SYNC_KEY_USER_ID, user_id.as_str()),
            (KEY_CURSOR_USER_ID, user_id.as_str()),
            (super::KEY_UPLOAD_FLOOR, "0"),
            (super::KEY_UPLOAD_FLOOR_ITEM, ""),
        ];
        if switched {
            entries.extend([
                (super::KEY_WATERMARK, "0"),
                (super::KEY_WATERMARK_ITEM, ""),
                (KEY_LAST_SYNC, ""),
            ]);
        }
        self.upload_cursor.replace_account(store, &entries)?;
        if let Some(previous) = account.as_ref() {
            previous.cancel.cancel();
        }
        *account = Some(Account {
            email,
            user_id,
            driver,
            cancel: CancellationToken::new(),
        });
        if switched {
            self.last_sync_ms.store(0, Ordering::Release);
        }
        *self.error() = None;
        Ok(switched)
    }

    pub(super) fn restore(&self, state: &super::super::state::BackendState) {
        let Some(config) = self.config.clone() else {
            return;
        };
        let store = &state.store;
        let (Ok(Some(email)), Ok(Some(user_id))) =
            (store.state(KEY_EMAIL), store.state(KEY_USER_ID))
        else {
            return;
        };
        let Some(session) = read_session(store, &user_id) else {
            return;
        };
        let Some(key) = read_key(store, &user_id) else {
            return;
        };
        let driver = Arc::new(copypaste_cloud::sync::CloudSync::new(
            copypaste_cloud::rest::SupabaseRest::new(config.clone()),
            copypaste_cloud::auth::SupabaseAuth::new(config.clone()),
            key,
            config,
            session,
            super::sensitive_guard(&state.detector),
        ));
        let mut account = self.account();
        if store.state(KEY_CURSOR_USER_ID).ok().flatten().as_deref() != Some(&user_id)
            && self
                .upload_cursor
                .replace_account(
                    store,
                    &[
                        (KEY_CURSOR_USER_ID, &user_id),
                        (super::KEY_WATERMARK, "0"),
                        (super::KEY_WATERMARK_ITEM, ""),
                        (super::KEY_UPLOAD_FLOOR, "0"),
                        (super::KEY_UPLOAD_FLOOR_ITEM, ""),
                        (KEY_LAST_SYNC, ""),
                    ],
                )
                .is_err()
        {
            return;
        }
        if let Some(previous) = account.as_ref() {
            previous.cancel.cancel();
        }
        *account = Some(Account {
            email,
            user_id,
            driver,
            cancel: CancellationToken::new(),
        });
        if let Ok(ms) = store.state_ms(KEY_LAST_SYNC) {
            self.last_sync_ms.store(ms, Ordering::Release);
        }
    }

    pub(super) fn round(&self) -> Option<AccountRound> {
        self.account().as_ref().map(|account| AccountRound {
            driver: Arc::clone(&account.driver),
            cancel: account.cancel.clone(),
        })
    }

    pub(super) fn with_driver<T>(
        &self,
        expected: &Arc<Driver>,
        cancel: &CancellationToken,
        action: impl FnOnce() -> T,
    ) -> Option<T> {
        self.account.with_driver(expected, cancel, action)
    }

    pub(super) fn take_for_sign_out(&self, store: &Store) -> Option<Arc<Driver>> {
        let mut account = self.account();
        self.account_revision.fetch_add(1, Ordering::AcqRel);
        if let Some(current) = account.as_ref() {
            current.driver.fence_session(None);
        }
        let previous = account.take();
        if let Some(previous) = previous.as_ref() {
            previous.cancel.cancel();
        }
        let _ = store.clear_state(CREDENTIAL_KEYS);
        self.last_sync_ms.store(0, Ordering::Release);
        *self.error() = None;
        previous.map(|account| account.driver)
    }

    pub(super) fn persist_session(&self, store: &Store, expected: &Arc<Driver>) {
        let account = self.account();
        if let Some(account) = account
            .as_ref()
            .filter(|account| Arc::ptr_eq(&account.driver, expected))
        {
            if let Err(error) = write_session(store, &account.driver, &account.user_id) {
                tracing::warn!(?error, "rotated cloud session was not persisted");
            }
        }
    }

    pub(super) fn note_success(&self, store: &Store, expected: &Arc<Driver>, completed: i64) {
        let account = self.account();
        if account
            .as_ref()
            .is_some_and(|account| Arc::ptr_eq(&account.driver, expected))
        {
            self.last_sync_ms.store(completed, Ordering::Release);
            *self.error() = None;
            let completed = completed.to_string();
            let _ = store.set_state_all(&[(KEY_LAST_SYNC, &completed)]);
        }
    }

    pub(super) fn record_failure(
        &self,
        store: &Store,
        expected: &Arc<Driver>,
        expected_revision: u64,
        message: &'static str,
        terminal: bool,
    ) {
        let mut account = self.account();
        if !account
            .as_ref()
            .is_some_and(|account| Arc::ptr_eq(&account.driver, expected))
        {
            return;
        }
        if expected.session_revision() != expected_revision {
            return;
        }
        if terminal {
            if !expected.fence_session(Some(expected_revision)) {
                return;
            }
            if let Some(current) = account.as_ref() {
                current.cancel.cancel();
            }
            let _expired = account.take();
            let _ = store.clear_state(CREDENTIAL_KEYS);
            self.last_sync_ms.store(0, Ordering::Release);
        }
        *self.error() = Some(message);
    }

    pub(super) fn driver(&self) -> Option<Arc<Driver>> {
        self.account()
            .as_ref()
            .map(|account| Arc::clone(&account.driver))
    }

    pub(super) fn account(&self) -> MutexGuard<'_, Option<Account>> {
        self.account.lock()
    }

    pub(super) fn error(&self) -> MutexGuard<'_, Option<&'static str>> {
        self.last_error
            .lock()
            .unwrap_or_else(|error| error.into_inner())
    }

    #[cfg(test)]
    pub(super) fn persist(&self, store: &Store, key_hex: &str) -> Result<(), StoreError> {
        let account = self.account();
        let Some(account) = account.as_ref() else {
            return Ok(());
        };
        persist_account(store, account, key_hex)
    }
}

#[cfg(test)]
fn persist_account(store: &Store, account: &Account, key_hex: &str) -> Result<(), StoreError> {
    account.driver.inspect_session(|session| {
        if session.user_id != account.user_id {
            return Ok(());
        }
        let expires = session.expires_at_ms.to_string();
        store.set_state_all(&[
            (KEY_EMAIL, &account.email),
            (KEY_USER_ID, &account.user_id),
            (KEY_ACCESS, &session.access_token),
            (KEY_REFRESH, &session.refresh_token),
            (KEY_EXPIRES, &expires),
            (KEY_SYNC_KEY, key_hex),
            (KEY_SESSION_USER_ID, &account.user_id),
            (KEY_SYNC_KEY_USER_ID, &account.user_id),
        ])
    })
}

fn write_session(store: &Store, driver: &Driver, expected_user_id: &str) -> Result<(), StoreError> {
    driver.inspect_session(|session| {
        if session.user_id != expected_user_id {
            return Ok(());
        }
        let expires = session.expires_at_ms.to_string();
        store.set_state_all(&[
            (KEY_ACCESS, &session.access_token),
            (KEY_REFRESH, &session.refresh_token),
            (KEY_EXPIRES, &expires),
            (KEY_SESSION_USER_ID, expected_user_id),
        ])
    })
}

fn read_session(store: &Store, expected_user_id: &str) -> Option<Session> {
    let owner = store.state(KEY_SESSION_USER_ID).ok()??;
    if owner != expected_user_id {
        return None;
    }
    Some(Session {
        access_token: store.state(KEY_ACCESS).ok()??,
        refresh_token: store.state(KEY_REFRESH).ok()??,
        user_id: owner,
        expires_at_ms: store.state_ms(KEY_EXPIRES).ok()?,
    })
}

fn read_key(store: &Store, expected_user_id: &str) -> Option<SyncKey> {
    if store.state(KEY_SYNC_KEY_USER_ID).ok()?? != expected_user_id {
        return None;
    }
    let bytes: [u8; 32] = hex::decode(store.state(KEY_SYNC_KEY).ok()??)
        .ok()?
        .try_into()
        .ok()?;
    Some(SyncKey::from_bytes(bytes))
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Barrier, Mutex};

    use copypaste_cloud::auth::{Session, SupabaseAuth};
    use copypaste_cloud::rest::SupabaseRest;
    use copypaste_cloud::sync::CloudSync;
    use tokio::sync::Notify;

    use super::*;
    use crate::backend::embedded::cloud::{sensitive_guard, UploadCursor};

    fn configured() -> Arc<EmbeddedCloud> {
        Arc::new(EmbeddedCloud {
            config: Some(copypaste_cloud::CloudConfig {
                url: "https://example.invalid".into(),
                anon_key: "anon".into(),
            }),
            account: AccountSlot::default(),
            account_revision: std::sync::atomic::AtomicU64::new(0),
            last_sync_ms: std::sync::atomic::AtomicI64::new(0),
            last_error: Mutex::new(None),
            upload_cursor: UploadCursor::new(),
            wake: Notify::new(),
            poller_started: std::sync::atomic::AtomicBool::new(false),
            shutdown: CancellationToken::new(),
        })
    }

    fn driver(
        cloud: &EmbeddedCloud,
        state: &crate::backend::embedded::state::BackendState,
        user_id: &str,
    ) -> Arc<Driver> {
        let config = cloud.config.clone().unwrap();
        Arc::new(CloudSync::new(
            SupabaseRest::new(config.clone()),
            SupabaseAuth::new(config.clone()),
            SyncKey::from_bytes([7; 32]),
            config,
            Session {
                access_token: "access-delayed".into(),
                refresh_token: "refresh-delayed".into(),
                user_id: user_id.into(),
                expires_at_ms: 123_000,
            },
            sensitive_guard(&state.detector),
        ))
    }

    #[test]
    fn concurrent_sign_out_fences_a_delayed_sign_in() {
        let dir = tempfile::TempDir::new().unwrap();
        let state =
            Arc::new(crate::backend::embedded::state::BackendState::open(dir.path()).unwrap());
        let cloud = configured();
        let attempt = cloud.begin_sign_in();
        let gate = Arc::new(Barrier::new(2));

        std::thread::scope(|scope| {
            let worker_cloud = Arc::clone(&cloud);
            let worker_state = Arc::clone(&state);
            let worker_gate = Arc::clone(&gate);
            let delayed = scope.spawn(move || {
                worker_gate.wait();
                worker_gate.wait();
                worker_cloud.activate(
                    &worker_state.store,
                    attempt,
                    "delayed@example.com".into(),
                    "delayed-user".into(),
                    driver(&worker_cloud, &worker_state, "delayed-user"),
                    &hex::encode([7; 32]),
                )
            });

            gate.wait();
            cloud.take_for_sign_out(&state.store);
            gate.wait();
            assert!(matches!(delayed.join().unwrap(), Err(ActivateError::Stale)));
        });

        assert!(cloud.account().is_none());
        for key in CREDENTIAL_KEYS {
            assert_eq!(state.store.state(key).unwrap(), None);
        }
    }
}
