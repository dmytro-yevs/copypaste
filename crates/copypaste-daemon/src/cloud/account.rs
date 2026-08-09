use std::sync::atomic::Ordering;
use std::sync::Arc;

use copypaste_cloud::auth::Session;
use copypaste_cloud::rest::SupabaseRest;
use copypaste_cloud::sync::{CloudSync, SyncError};
use copypaste_cloud::{CloudConfig, SyncKey};
use tokio_util::sync::CancellationToken;

use super::{sensitive_guard, Cloud, Driver};
use crate::meta::{Meta, MetaError};
use crate::AppState;

const KEY_EMAIL: &str = "cloud_email";
const KEY_USER_ID: &str = "cloud_user_id";
const KEY_ACCESS: &str = "cloud_access_token";
pub(crate) const KEY_REFRESH: &str = "cloud_refresh_token";
const KEY_EXPIRES: &str = "cloud_expires_at_ms";
pub(crate) const KEY_SYNC_KEY: &str = "cloud_sync_key";
const KEY_SESSION_USER_ID: &str = "cloud_session_user_id";
const KEY_SYNC_KEY_USER_ID: &str = "cloud_sync_key_user_id";
pub(crate) const KEY_CURSOR_USER_ID: &str = "cloud_cursor_user_id";
pub(crate) const KEY_LAST_SYNC: &str = "cloud_last_sync_ms";
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

#[derive(Debug)]
pub(crate) enum ActivateError {
    Stale,
    Store(MetaError),
    AccountMismatch,
}

impl From<MetaError> for ActivateError {
    fn from(error: MetaError) -> Self {
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

    pub(crate) fn activate(
        &self,
        state: &AppState,
        attempt: SignInAttempt,
        config: CloudConfig,
        email: String,
        user_id: String,
        key: SyncKey,
        session: Session,
        key_hex: &str,
    ) -> Result<bool, ActivateError> {
        if session.user_id != user_id {
            return Err(ActivateError::AccountMismatch);
        }
        let driver = Arc::new(CloudSync::new(
            SupabaseRest::new(config.clone()),
            copypaste_cloud::auth::SupabaseAuth::new(config.clone()),
            key,
            config,
            session,
            sensitive_guard(state),
        ));
        let mut account = self.lock_account();
        if self.account_revision.load(Ordering::Acquire) != attempt.0 {
            return Err(ActivateError::Stale);
        }

        let meta = &state.meta;
        let switched = meta.state(KEY_CURSOR_USER_ID)?.as_deref() != Some(&user_id);
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
        let _cursor = self.lock_upload_floor();
        meta.set_state_all(&entries)?;
        self.upload_floor_epoch.fetch_add(1, Ordering::AcqRel);

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

    pub(crate) fn persist_session(&self, meta: &Meta, expected: &Arc<Driver>) {
        let account_guard = self.lock_account();
        let Some(account) = account_guard
            .as_ref()
            .filter(|account| Arc::ptr_eq(&account.driver, expected))
        else {
            return;
        };
        if let Err(error) = write_session(meta, &account.driver, &account.user_id) {
            tracing::warn!(error = ?error, "could not persist the rotated cloud session");
        }
        drop(account_guard);
        self.notify_session_changed();
    }

    pub(crate) fn sign_out(&self, meta: &Meta) -> Option<Arc<Driver>> {
        let mut account = self.lock_account();
        self.account_revision.fetch_add(1, Ordering::AcqRel);
        if let Some(current) = account.as_ref() {
            current.driver.fence_session(None);
        }
        let previous = account.take();
        if let Some(previous) = previous.as_ref() {
            previous.cancel.cancel();
        }
        if let Err(error) = meta.clear_state(CREDENTIAL_KEYS) {
            tracing::warn!(error = ?error, "could not clear the stored cloud account");
        }
        self.last_sync_ms.store(0, Ordering::Release);
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
        let meta = &state.meta;
        let (Ok(Some(email)), Ok(Some(user_id))) = (meta.state(KEY_EMAIL), meta.state(KEY_USER_ID))
        else {
            return false;
        };
        let Some(session) = read_session(meta, &user_id) else {
            return false;
        };
        let Some(key) = read_key(meta, &user_id) else {
            tracing::warn!("a stored cloud account has no account-bound sync key; not signing in");
            return false;
        };
        let driver = Arc::new(CloudSync::new(
            SupabaseRest::new(config.clone()),
            copypaste_cloud::auth::SupabaseAuth::new(config.clone()),
            key,
            config,
            session,
            sensitive_guard(state),
        ));
        if meta.state(KEY_CURSOR_USER_ID).ok().flatten().as_deref() != Some(&user_id) {
            let _cursor = self.lock_upload_floor();
            if meta
                .set_state_all(&[
                    (KEY_CURSOR_USER_ID, &user_id),
                    (super::KEY_WATERMARK, "0"),
                    (super::KEY_WATERMARK_ITEM, ""),
                    (super::KEY_UPLOAD_FLOOR, "0"),
                    (super::KEY_UPLOAD_FLOOR_ITEM, ""),
                    (KEY_LAST_SYNC, ""),
                ])
                .is_err()
            {
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
        if let Ok(ms) = meta.state_ms(KEY_LAST_SYNC) {
            self.last_sync_ms.store(ms, Ordering::Release);
        }
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
        let key_hex = hex::encode(key.to_bytes());
        let attempt = self.begin_sign_in();
        self.activate(
            state, attempt, config, email, user_id, key, session, &key_hex,
        )
        .unwrap_or_else(|_| panic!("test account activation failed"));
    }

    #[cfg(test)]
    pub(crate) fn persist(&self, meta: &Meta, key_hex: &str) -> Result<(), MetaError> {
        let account = self.lock_account();
        let Some(account) = account.as_ref() else {
            return Ok(());
        };
        persist_account(meta, account, key_hex)
    }
}

#[cfg(test)]
fn persist_account(meta: &Meta, account: &Account, key_hex: &str) -> Result<(), MetaError> {
    account.driver.inspect_session(|session| {
        if session.user_id != account.user_id {
            return Ok(());
        }
        let expires = session.expires_at_ms.to_string();
        meta.set_state_all(&[
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

pub(super) fn write_session(
    meta: &Meta,
    driver: &Driver,
    expected_user_id: &str,
) -> Result<(), MetaError> {
    driver.inspect_session(|session| {
        if session.user_id != expected_user_id {
            return Ok(());
        }
        let expires = session.expires_at_ms.to_string();
        meta.set_state_all(&[
            (KEY_ACCESS, &session.access_token),
            (KEY_REFRESH, &session.refresh_token),
            (KEY_EXPIRES, &expires),
            (KEY_SESSION_USER_ID, expected_user_id),
        ])
    })
}

fn read_session(meta: &Meta, expected_user_id: &str) -> Option<Session> {
    let owner = meta.state(KEY_SESSION_USER_ID).ok()??;
    if owner != expected_user_id {
        return None;
    }
    Some(Session {
        access_token: meta.state(KEY_ACCESS).ok()??,
        refresh_token: meta.state(KEY_REFRESH).ok()??,
        user_id: owner,
        expires_at_ms: meta.state_ms(KEY_EXPIRES).ok()?,
    })
}

fn read_key(meta: &Meta, expected_user_id: &str) -> Option<SyncKey> {
    if meta.state(KEY_SYNC_KEY_USER_ID).ok()?? != expected_user_id {
        return None;
    }
    let bytes: [u8; 32] = hex::decode(meta.state(KEY_SYNC_KEY).ok()??)
        .ok()?
        .try_into()
        .ok()?;
    Some(SyncKey::from_bytes(bytes))
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
        CloudConfig {
            url: "https://example.invalid".into(),
            anon_key: "anon".into(),
        }
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
                (KEY_CURSOR_USER_ID, "old-user"),
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
                state_worker.cloud.activate(
                    &state_worker,
                    stale_attempt,
                    config(),
                    "stale@example.com".into(),
                    "stale-user".into(),
                    key,
                    session("stale-user", "stale"),
                    &hex::encode([2; 32]),
                )
            });

            gate.wait();
            let current_attempt = state.cloud.begin_sign_in();
            state
                .cloud
                .activate(
                    &state,
                    current_attempt,
                    config(),
                    "current@example.com".into(),
                    "current-user".into(),
                    SyncKey::from_bytes([3; 32]),
                    session("current-user", "current"),
                    &hex::encode([3; 32]),
                )
                .unwrap();
            gate.wait();
            assert!(matches!(stale.join().unwrap(), Err(ActivateError::Stale)));
        });

        assert!(stale_source.set_watermark(9_000).is_err());
        assert_eq!(
            state.meta.state(KEY_USER_ID).unwrap().as_deref(),
            Some("current-user")
        );
        assert_eq!(
            state.meta.state(KEY_SESSION_USER_ID).unwrap().as_deref(),
            Some("current-user")
        );
        assert_eq!(
            state.meta.state(KEY_SYNC_KEY_USER_ID).unwrap().as_deref(),
            Some("current-user")
        );
        assert_eq!(
            state.meta.state(KEY_CURSOR_USER_ID).unwrap().as_deref(),
            Some("current-user")
        );
        assert_eq!(
            state.meta.state(KEY_SYNC_KEY).unwrap().as_deref(),
            Some(hex::encode([3; 32]).as_str())
        );
        assert_eq!(
            state.meta.state(KEY_ACCESS).unwrap().as_deref(),
            Some("access-current")
        );
        assert_eq!(state.meta.state_ms(super::super::KEY_WATERMARK).unwrap(), 0);
        assert_eq!(
            state.cloud.status().email.as_deref(),
            Some("current@example.com")
        );
    }
}
