use std::sync::atomic::Ordering;
use std::sync::Arc;

use copypaste_cloud::auth::Session;
use copypaste_cloud::credentials::{
    AccountChange, CloudStateKey, CredentialError, CredentialStore,
};
use copypaste_cloud::rest::SupabaseRest;
use copypaste_cloud::sync::{CloudSync, SyncError};
use copypaste_cloud::{CloudConfig, SyncKey};
use tokio_util::sync::CancellationToken;

use super::{sensitive_guard, Cloud, Driver};
use crate::AppState;

pub(crate) const MSG_ACCOUNT_CHANGED: &str = "the sync account changed during this operation";

pub(super) struct Account {
    pub(super) email: String,
    pub(super) user_id: String,
    pub(super) driver: Arc<Driver>,
    pub(super) cancel: CancellationToken,
}

pub(crate) struct AccountRound {
    pub(crate) driver: Arc<Driver>,
    pub(crate) cancel: CancellationToken,
}

#[derive(Clone, Copy)]
pub(crate) struct SignInAttempt(u64);

pub(crate) struct ActivateRequest<'a> {
    pub(crate) state: &'a AppState,
    pub(crate) attempt: SignInAttempt,
    pub(crate) config: CloudConfig,
    pub(crate) email: String,
    pub(crate) user_id: String,
    pub(crate) key: SyncKey,
    pub(crate) session: Session,
}

#[derive(Debug)]
pub(crate) enum ActivateError {
    Stale,
    Store(CredentialError),
    AccountMismatch,
}

impl From<CredentialError> for ActivateError {
    fn from(error: CredentialError) -> Self {
        Self::Store(error)
    }
}

impl Cloud {
    pub(crate) fn begin_sign_in(&self) -> SignInAttempt {
        let _account = self.lock_account();
        SignInAttempt(
            self.account_revision
                .fetch_add(1, Ordering::AcqRel)
                .wrapping_add(1),
        )
    }

    pub(crate) fn activate(&self, request: ActivateRequest<'_>) -> Result<bool, ActivateError> {
        let ActivateRequest {
            state,
            attempt,
            config,
            email,
            user_id,
            key,
            session,
        } = request;
        if session.user_id != user_id {
            return Err(ActivateError::AccountMismatch);
        }
        let mut account = self.lock_account();
        if self.account_revision.load(Ordering::Acquire) != attempt.0 {
            return Err(ActivateError::Stale);
        }

        let switched = state
            .store
            .state(CloudStateKey::CursorUserId.as_str())?
            .as_deref()
            != Some(&user_id);
        let _cursor = self.lock_upload_floor();
        let key_bytes = key.to_bytes();
        state.store.replace_cloud_credentials(
            &email,
            &session,
            &key_bytes,
            if switched {
                AccountChange::SwitchedAccount
            } else {
                AccountChange::SameAccount
            },
        )?;
        self.upload_floor_epoch.fetch_add(1, Ordering::AcqRel);

        let driver = Arc::new(CloudSync::new(
            SupabaseRest::new(config.clone()),
            copypaste_cloud::auth::SupabaseAuth::new(config.clone()),
            key,
            config,
            session,
            sensitive_guard(state),
        ));

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
            self.note_unreadable_uploads(0);
        }
        *self.lock_error() = None;
        drop(_cursor);
        drop(account);
        self.notify_session_changed();
        Ok(switched)
    }

    pub(crate) fn round(&self) -> Option<AccountRound> {
        self.lock_account().as_ref().map(|account| AccountRound {
            driver: Arc::clone(&account.driver),
            cancel: account.cancel.clone(),
        })
    }

    pub(crate) fn with_current_driver<T>(
        &self,
        expected: &Arc<Driver>,
        action: impl FnOnce() -> Result<T, SyncError>,
    ) -> Result<T, SyncError> {
        let account = self.lock_account();
        if !account
            .as_ref()
            .is_some_and(|account| Arc::ptr_eq(&account.driver, expected))
        {
            return Err(SyncError::Source(MSG_ACCOUNT_CHANGED));
        }
        action()
    }

    pub(crate) fn persist_session(&self, store: &copypaste_core::Store, expected: &Arc<Driver>) {
        let account_guard = self.lock_account();
        let Some(account) = account_guard
            .as_ref()
            .filter(|account| Arc::ptr_eq(&account.driver, expected))
        else {
            return;
        };
        if let Err(error) = write_session(store, &account.driver, &account.user_id) {
            tracing::warn!(error = ?error, "could not persist the rotated cloud session");
        }
        drop(account_guard);
        self.notify_session_changed();
    }

    pub(crate) fn sign_out(&self, store: &copypaste_core::Store) -> Option<Arc<Driver>> {
        let mut account = self.lock_account();
        self.account_revision.fetch_add(1, Ordering::AcqRel);
        if let Some(current) = account.as_ref() {
            current.driver.fence_session(None);
        }
        let previous = account.take();
        if let Some(previous) = previous.as_ref() {
            previous.cancel.cancel();
        }
        if let Err(error) = store.clear_cloud_credentials() {
            tracing::warn!(error = ?error, "could not clear the stored cloud account");
        }
        self.last_sync_ms.store(0, Ordering::Release);
        self.note_unreadable_uploads(0);
        *self.lock_error() = None;
        drop(account);
        self.notify_session_changed();
        previous.map(|account| account.driver)
    }

    pub(crate) fn restore(&self, state: &AppState) -> bool {
        let Some(config) = self.config.clone() else {
            return false;
        };
        let mut account = self.lock_account();
        let stored = match state.store.cloud_credentials() {
            Ok(Some(stored)) => stored,
            Ok(None) => return false,
            Err(error) => {
                tracing::warn!(?error, "could not read the stored cloud account");
                return false;
            }
        };
        let email = stored.email;
        let user_id = stored.session.user_id.clone();
        let session = stored.session;
        let key = stored.sync_key;
        let driver = Arc::new(CloudSync::new(
            SupabaseRest::new(config.clone()),
            copypaste_cloud::auth::SupabaseAuth::new(config.clone()),
            key,
            config,
            session,
            sensitive_guard(state),
        ));
        if state
            .store
            .state(CloudStateKey::CursorUserId.as_str())
            .ok()
            .flatten()
            .as_deref()
            != Some(&user_id)
        {
            let _cursor = self.lock_upload_floor();
            if state.store.bind_cloud_cursor(&user_id).is_err() {
                return false;
            }
            self.upload_floor_epoch.fetch_add(1, Ordering::AcqRel);
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
        if let Ok(ms) = state.store.state_ms(CloudStateKey::LastSyncMs.as_str()) {
            self.last_sync_ms.store(ms, Ordering::Release);
        }
        // Seeded from the persisted record rather than waiting for the first
        // round: a device restarted with rows it cannot upload should say so
        // before it has had a chance to rediscover them.
        self.note_unreadable_uploads(
            copypaste_cloud::sync::UnreadableUploads::decode(
                state
                    .store
                    .state(CloudStateKey::UnreadableUploads.as_str())
                    .ok()
                    .flatten()
                    .as_deref(),
            )
            .total,
        );
        drop(account);
        self.notify_session_changed();
        tracing::info!("restored a cloud sync account");
        true
    }

    #[cfg(test)]
    pub(crate) fn install(
        &self,
        state: &AppState,
        config: CloudConfig,
        email: String,
        user_id: String,
        key: SyncKey,
        session: Session,
    ) {
        let attempt = self.begin_sign_in();
        self.activate(ActivateRequest {
            state,
            attempt,
            config,
            email,
            user_id,
            key,
            session,
        })
        .unwrap_or_else(|_| panic!("test account activation failed"));
    }

    #[cfg(test)]
    pub(crate) fn persist(
        &self,
        store: &copypaste_core::Store,
        key: &[u8; 32],
    ) -> Result<(), CredentialError> {
        let account = self.lock_account();
        let Some(account) = account.as_ref() else {
            return Ok(());
        };
        persist_account(store, account, key)
    }
}

#[cfg(test)]
fn persist_account(
    store: &copypaste_core::Store,
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

impl From<copypaste_core::StoreError> for ActivateError {
    fn from(error: copypaste_core::StoreError) -> Self {
        Self::Store(error.into())
    }
}

pub(super) fn write_session(
    store: &copypaste_core::Store,
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
    use std::sync::{Arc, Barrier};

    use copypaste_cloud::auth::Session;
    use copypaste_cloud::sync::CloudSource;

    use super::*;
    use crate::cloud::source::StoreSource;
    use crate::testutil::test_state_with_cloud;

    fn config() -> CloudConfig {
        CloudConfig::new("https://example.invalid", "anon").unwrap()
    }

    fn session(user_id: &str, token: &str) -> Session {
        Session {
            access_token: format!("access-{token}"),
            refresh_token: format!("refresh-{token}"),
            user_id: user_id.into(),
            expires_at_ms: 123_000,
        }
    }

    #[test]
    fn a_concurrent_account_switch_cannot_mix_or_restore_the_stale_account() {
        let (state, _dir) = test_state_with_cloud("account-race", Cloud::new(Some(config())));
        state.cloud.install(
            &state,
            config(),
            "old@example.com".into(),
            "old-user".into(),
            SyncKey::from_bytes([1; 32]),
            session("old-user", "old"),
        );
        state
            .meta
            .set_state_all(&[
                (CloudStateKey::CursorUserId.as_str(), "old-user"),
                (super::super::KEY_WATERMARK, "7000"),
                (super::super::KEY_WATERMARK_ITEM, "z"),
            ])
            .unwrap();
        let stale_driver = state.cloud.driver().unwrap();
        let stale_source = StoreSource::for_round(Arc::clone(&state), stale_driver);

        let stale_attempt = state.cloud.begin_sign_in();
        let gate = Arc::new(Barrier::new(2));
        std::thread::scope(|scope| {
            let gate_worker = Arc::clone(&gate);
            let state_worker = Arc::clone(&state);
            let stale = scope.spawn(move || {
                gate_worker.wait();
                gate_worker.wait();
                let key = SyncKey::from_bytes([2; 32]);
                state_worker.cloud.activate(ActivateRequest {
                    state: &state_worker,
                    attempt: stale_attempt,
                    config: config(),
                    email: "stale@example.com".into(),
                    user_id: "stale-user".into(),
                    key,
                    session: session("stale-user", "stale"),
                })
            });

            gate.wait();
            let current_attempt = state.cloud.begin_sign_in();
            state
                .cloud
                .activate(ActivateRequest {
                    state: &state,
                    attempt: current_attempt,
                    config: config(),
                    email: "current@example.com".into(),
                    user_id: "current-user".into(),
                    key: SyncKey::from_bytes([3; 32]),
                    session: session("current-user", "current"),
                })
                .unwrap();
            gate.wait();
            assert!(matches!(stale.join().unwrap(), Err(ActivateError::Stale)));
        });

        assert!(stale_source.set_watermark(9_000).is_err());
        assert_eq!(
            state
                .store
                .state(CloudStateKey::UserId.as_str())
                .unwrap()
                .as_deref(),
            Some("current-user")
        );
        assert_eq!(
            state
                .store
                .state(CloudStateKey::SessionUserId.as_str())
                .unwrap()
                .as_deref(),
            Some("current-user")
        );
        assert_eq!(
            state
                .store
                .state(CloudStateKey::SyncKeyUserId.as_str())
                .unwrap()
                .as_deref(),
            Some("current-user")
        );
        assert_eq!(
            state
                .store
                .state(CloudStateKey::CursorUserId.as_str())
                .unwrap()
                .as_deref(),
            Some("current-user")
        );
        assert_eq!(
            state
                .store
                .state(CloudStateKey::SyncKey.as_str())
                .unwrap()
                .as_deref(),
            Some(hex::encode([3; 32]).as_str())
        );
        assert_eq!(
            state
                .store
                .state(CloudStateKey::AccessToken.as_str())
                .unwrap()
                .as_deref(),
            Some("access-current")
        );
        assert_eq!(state.meta.state_ms(super::super::KEY_WATERMARK).unwrap(), 0);
        assert_eq!(
            state.cloud.status().email.as_deref(),
            Some("current@example.com")
        );
    }
}
