use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex, MutexGuard};

use copypaste_cloud::credentials::{
    AccountChange, CloudStateKey, CredentialError, CredentialStore, StoredCredentials,
};
use copypaste_core::Store;
use tokio_util::sync::CancellationToken;

use super::{Driver, EmbeddedCloud};

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
    Store(CredentialError),
    AccountMismatch,
}

impl From<CredentialError> for ActivateError {
    fn from(error: CredentialError) -> Self {
        Self::Store(error)
    }
}

impl From<copypaste_core::StoreError> for ActivateError {
    fn from(error: copypaste_core::StoreError) -> Self {
        Self::Store(error.into())
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
        key: &[u8; 32],
    ) -> std::result::Result<bool, ActivateError> {
        if !driver.inspect_session(|session| session.user_id == user_id) {
            return Err(ActivateError::AccountMismatch);
        }
        let mut account = self.account();
        if self.account_revision.load(Ordering::Acquire) != attempt.0 {
            return Err(ActivateError::Stale);
        }
        let switched = store
            .state(CloudStateKey::CursorUserId.as_str())?
            .as_deref()
            != Some(&user_id);
        let session = driver.inspect_session(Clone::clone);
        self.upload_cursor.replace_account(|| {
            store.replace_cloud_credentials(
                &email,
                &session,
                key,
                if switched {
                    AccountChange::SwitchedAccount
                } else {
                    AccountChange::SameAccount
                },
            )
        })?;
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
            // The count belongs to the history *this account* has scanned, and
            // the new one has scanned none of it. Leaving it would show the
            // previous account's figure until the first round finishes.
            self.unreadable_uploads.store(0, Ordering::Release);
        }
        *self.error() = None;
        Ok(switched)
    }

    pub(super) fn restore(&self, state: &super::super::state::BackendState) {
        let Some(config) = self.active_config() else {
            return;
        };
        let store = &state.store;
        let StoredCredentials {
            email,
            session,
            sync_key: key,
        } = match store.cloud_credentials() {
            Ok(Some(stored)) => stored,
            Ok(None) => return,
            Err(error) => {
                tracing::warn!(?error, "embedded cloud account could not be restored");
                return;
            }
        };
        let user_id = session.user_id.clone();
        let driver = Arc::new(copypaste_cloud::sync::CloudSync::new(
            copypaste_cloud::rest::SupabaseRest::new(config.clone()),
            copypaste_cloud::auth::SupabaseAuth::new(config.clone()),
            key,
            config,
            session,
            super::sensitive_guard(&state.detector),
        ));
        let mut account = self.account();
        if store
            .state(CloudStateKey::CursorUserId.as_str())
            .ok()
            .flatten()
            .as_deref()
            != Some(&user_id)
            && self
                .upload_cursor
                .replace_account(|| store.bind_cloud_cursor(&user_id))
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
        if let Ok(ms) = store.state_ms(CloudStateKey::LastSyncMs.as_str()) {
            self.last_sync_ms.store(ms, Ordering::Release);
        }
        self.note_unreadable_uploads(
            copypaste_cloud::sync::UnreadableUploads::decode(
                store
                    .state(CloudStateKey::UnreadableUploads.as_str())
                    .ok()
                    .flatten()
                    .as_deref(),
            )
            .total,
        );
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
        let _ = store.clear_cloud_credentials();
        self.last_sync_ms.store(0, Ordering::Release);
        self.unreadable_uploads.store(0, Ordering::Release);
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
            let _ = store.set_state_all(&[(CloudStateKey::LastSyncMs.as_str(), &completed)]);
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
            let _ = store.clear_cloud_credentials();
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
    pub(super) fn persist(&self, store: &Store, key: &[u8; 32]) -> Result<(), CredentialError> {
        let account = self.account();
        let Some(account) = account.as_ref() else {
            return Ok(());
        };
        persist_account(store, account, key)
    }
}

#[cfg(test)]
fn persist_account(
    store: &Store,
    account: &Account,
    key: &[u8; 32],
) -> Result<(), CredentialError> {
    account.driver.inspect_session(|session| {
        if session.user_id != account.user_id {
            return Err(CredentialError::AccountMismatch);
        }
        store.replace_cloud_credentials(&account.email, session, key, AccountChange::SameAccount)
    })
}

fn write_session(
    store: &Store,
    driver: &Driver,
    expected_user_id: &str,
) -> Result<(), CredentialError> {
    driver.inspect_session(|session| {
        if session.user_id != expected_user_id {
            return Err(CredentialError::AccountMismatch);
        }
        store.update_cloud_session(session)
    })
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Barrier, Mutex};

    use copypaste_cloud::auth::{Session, SupabaseAuth};
    use copypaste_cloud::credentials::SIGN_OUT_KEYS;
    use copypaste_cloud::rest::SupabaseRest;
    use copypaste_cloud::sync::CloudSync;
    use copypaste_cloud::SyncKey;
    use tokio::sync::Notify;

    use super::*;
    use crate::backend::embedded::cloud::{sensitive_guard, UploadCursor};

    fn configured() -> Arc<EmbeddedCloud> {
        let hosted = copypaste_cloud::CloudConfig::new("https://example.invalid", "anon").unwrap();
        Arc::new(EmbeddedCloud {
            hosted: Some(hosted.clone()),
            config: Mutex::new(Some(hosted)),
            account: AccountSlot::default(),
            account_revision: std::sync::atomic::AtomicU64::new(0),
            last_sync_ms: std::sync::atomic::AtomicI64::new(0),
            last_error: Mutex::new(None),
            upload_cursor: UploadCursor::new(),
            wake: Notify::new(),
            poller_started: std::sync::atomic::AtomicBool::new(false),
            shutdown: CancellationToken::new(),
            rounds: copypaste_core::sync::RoundGate::new(),
            unreadable_uploads: std::sync::atomic::AtomicU32::new(0),
        })
    }

    fn driver(
        cloud: &EmbeddedCloud,
        state: &crate::backend::embedded::state::BackendState,
        user_id: &str,
    ) -> Arc<Driver> {
        let config = cloud.active_config().unwrap();
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
                    &[7; 32],
                )
            });

            gate.wait();
            cloud.take_for_sign_out(&state.store);
            gate.wait();
            assert!(matches!(delayed.join().unwrap(), Err(ActivateError::Stale)));
        });

        assert!(cloud.account().is_none());
        for key in SIGN_OUT_KEYS {
            assert_eq!(state.store.state(key.as_str()).unwrap(), None);
        }
    }

    /// N1, the embedded half. The count is what *this account's* scan of the
    /// history found; a second account has scanned none of it, so carrying the
    /// number over shows the previous account's figure until a round finishes.
    /// Re-activating the same account — a token refresh runs this path — is not
    /// a switch and must keep it.
    #[test]
    fn switching_accounts_clears_the_unreadable_upload_count() {
        let dir = tempfile::TempDir::new().unwrap();
        let state =
            Arc::new(crate::backend::embedded::state::BackendState::open(dir.path()).unwrap());
        let cloud = configured();

        let sign_in = |user: &str| {
            let attempt = cloud.begin_sign_in();
            cloud
                .activate(
                    &state.store,
                    attempt,
                    format!("{user}@example.com"),
                    user.to_string(),
                    driver(&cloud, &state, user),
                    &[7; 32],
                )
                .expect("activation")
        };

        assert!(sign_in("user-1"));
        cloud.note_unreadable_uploads(7);
        assert!(!sign_in("user-1"), "the same account is not a switch");
        assert_eq!(cloud.status().unreadable_uploads, 7);

        assert!(sign_in("user-2"));
        assert_eq!(
            cloud.status().unreadable_uploads,
            0,
            "the new account inherited the old one's count"
        );
    }
}
