use copypaste_core::{Store, StoreError};
use thiserror::Error;
use zeroize::Zeroizing;

use crate::auth::Session;
use crate::SyncKey;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CloudStateKey {
    Email,
    UserId,
    AccessToken,
    RefreshToken,
    ExpiresAtMs,
    SyncKey,
    SessionUserId,
    SyncKeyUserId,
    CursorUserId,
    LastSyncMs,
    WatermarkMs,
    WatermarkItemId,
    UploadFloorMs,
    UploadFloorItemId,
}

impl CloudStateKey {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Email => "cloud_email",
            Self::UserId => "cloud_user_id",
            Self::AccessToken => "cloud_access_token",
            Self::RefreshToken => "cloud_refresh_token",
            Self::ExpiresAtMs => "cloud_expires_at_ms",
            Self::SyncKey => "cloud_sync_key",
            Self::SessionUserId => "cloud_session_user_id",
            Self::SyncKeyUserId => "cloud_sync_key_user_id",
            Self::CursorUserId => "cloud_cursor_user_id",
            Self::LastSyncMs => "cloud_last_sync_ms",
            Self::WatermarkMs => "cloud_watermark_ms",
            Self::WatermarkItemId => "cloud_watermark_item_id",
            Self::UploadFloorMs => "cloud_upload_floor_ms",
            Self::UploadFloorItemId => "cloud_upload_floor_item_id",
        }
    }
}

pub const CREDENTIAL_KEYS: &[CloudStateKey] = &[
    CloudStateKey::Email,
    CloudStateKey::UserId,
    CloudStateKey::AccessToken,
    CloudStateKey::RefreshToken,
    CloudStateKey::ExpiresAtMs,
    CloudStateKey::SyncKey,
    CloudStateKey::SessionUserId,
    CloudStateKey::SyncKeyUserId,
];

pub const SIGN_OUT_KEYS: &[CloudStateKey] = &[
    CloudStateKey::Email,
    CloudStateKey::UserId,
    CloudStateKey::AccessToken,
    CloudStateKey::RefreshToken,
    CloudStateKey::ExpiresAtMs,
    CloudStateKey::SyncKey,
    CloudStateKey::SessionUserId,
    CloudStateKey::SyncKeyUserId,
    CloudStateKey::LastSyncMs,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AccountChange {
    SameAccount,
    SwitchedAccount,
}

#[derive(Debug, Error)]
pub enum CredentialError {
    #[error("cloud credential storage is unavailable")]
    Store(#[source] StoreError),
    #[error("the stored cloud credentials do not belong to the active account")]
    AccountMismatch,
}

impl From<StoreError> for CredentialError {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}

pub struct StoredCredentials {
    pub email: String,
    pub session: Session,
    pub sync_key: SyncKey,
}

pub trait CredentialStore {
    fn cloud_credentials(&self) -> Result<Option<StoredCredentials>, CredentialError>;

    fn replace_cloud_credentials(
        &self,
        email: &str,
        session: &Session,
        sync_key: &[u8; 32],
        change: AccountChange,
    ) -> Result<(), CredentialError>;

    fn bind_cloud_cursor(&self, user_id: &str) -> Result<(), CredentialError>;

    fn update_cloud_session(&self, session: &Session) -> Result<(), CredentialError>;

    fn clear_cloud_credentials(&self) -> Result<(), CredentialError>;
}

impl CredentialStore for Store {
    fn cloud_credentials(&self) -> Result<Option<StoredCredentials>, CredentialError> {
        let values = CREDENTIAL_KEYS
            .iter()
            .map(|key| self.state(key.as_str()))
            .collect::<Result<Vec<_>, _>>()?;
        if values.iter().all(Option::is_none) {
            return Ok(None);
        }

        let complete = (|| {
            let email = values[0].clone()?;
            let user_id = values[1].clone()?;
            let access_token = values[2].clone()?;
            let refresh_token = values[3].clone()?;
            let expires_at_ms = values[4].as_deref()?.parse::<i64>().ok()?.max(0);
            let key_hex = values[5].as_deref()?;
            if user_id.is_empty() {
                return None;
            }
            if values[6].as_deref()? != user_id || values[7].as_deref()? != user_id {
                return None;
            }
            let key_bytes: [u8; 32] = hex::decode(key_hex).ok()?.try_into().ok()?;
            if hex::encode(key_bytes) != key_hex {
                return None;
            }
            Some(StoredCredentials {
                email,
                session: Session {
                    access_token,
                    refresh_token,
                    user_id,
                    expires_at_ms,
                },
                sync_key: SyncKey::from_bytes(key_bytes),
            })
        })();

        if complete.is_none() {
            self.clear_cloud_credentials()?;
        }
        Ok(complete)
    }

    fn replace_cloud_credentials(
        &self,
        email: &str,
        session: &Session,
        sync_key: &[u8; 32],
        change: AccountChange,
    ) -> Result<(), CredentialError> {
        let expires_at_ms = session.expires_at_ms.max(0).to_string();
        let key_hex = Zeroizing::new(hex::encode(sync_key));
        let mut entries = vec![
            (CloudStateKey::Email.as_str(), email),
            (CloudStateKey::UserId.as_str(), session.user_id.as_str()),
            (
                CloudStateKey::AccessToken.as_str(),
                session.access_token.as_str(),
            ),
            (
                CloudStateKey::RefreshToken.as_str(),
                session.refresh_token.as_str(),
            ),
            (CloudStateKey::ExpiresAtMs.as_str(), &expires_at_ms),
            (CloudStateKey::SyncKey.as_str(), key_hex.as_str()),
            (
                CloudStateKey::SessionUserId.as_str(),
                session.user_id.as_str(),
            ),
            (
                CloudStateKey::SyncKeyUserId.as_str(),
                session.user_id.as_str(),
            ),
            (
                CloudStateKey::CursorUserId.as_str(),
                session.user_id.as_str(),
            ),
            (CloudStateKey::UploadFloorMs.as_str(), "0"),
            (CloudStateKey::UploadFloorItemId.as_str(), ""),
        ];
        if change == AccountChange::SwitchedAccount {
            entries.extend([
                (CloudStateKey::WatermarkMs.as_str(), "0"),
                (CloudStateKey::WatermarkItemId.as_str(), ""),
                (CloudStateKey::LastSyncMs.as_str(), ""),
            ]);
        }
        self.set_state_all(&entries)?;
        Ok(())
    }

    fn bind_cloud_cursor(&self, user_id: &str) -> Result<(), CredentialError> {
        self.set_state_all(&[
            (CloudStateKey::CursorUserId.as_str(), user_id),
            (CloudStateKey::WatermarkMs.as_str(), "0"),
            (CloudStateKey::WatermarkItemId.as_str(), ""),
            (CloudStateKey::UploadFloorMs.as_str(), "0"),
            (CloudStateKey::UploadFloorItemId.as_str(), ""),
            (CloudStateKey::LastSyncMs.as_str(), ""),
        ])?;
        Ok(())
    }

    fn update_cloud_session(&self, session: &Session) -> Result<(), CredentialError> {
        let account_user_id = self.state(CloudStateKey::UserId.as_str())?;
        match account_user_id.as_deref() {
            Some(user_id) if user_id == session.user_id => {}
            Some(_) => return Err(CredentialError::AccountMismatch),
            None => {
                self.clear_cloud_credentials()?;
                return Err(CredentialError::AccountMismatch);
            }
        }
        let session_owner = self.state(CloudStateKey::SessionUserId.as_str())?;
        let key_owner = self.state(CloudStateKey::SyncKeyUserId.as_str())?;
        if session_owner.as_deref() != Some(session.user_id.as_str())
            || key_owner.as_deref() != Some(session.user_id.as_str())
        {
            self.clear_cloud_credentials()?;
            return Err(CredentialError::AccountMismatch);
        }
        let expires_at_ms = session.expires_at_ms.max(0).to_string();
        self.set_state_all(&[
            (
                CloudStateKey::AccessToken.as_str(),
                session.access_token.as_str(),
            ),
            (
                CloudStateKey::RefreshToken.as_str(),
                session.refresh_token.as_str(),
            ),
            (CloudStateKey::ExpiresAtMs.as_str(), &expires_at_ms),
            (
                CloudStateKey::SessionUserId.as_str(),
                session.user_id.as_str(),
            ),
        ])?;
        Ok(())
    }

    fn clear_cloud_credentials(&self) -> Result<(), CredentialError> {
        let names = SIGN_OUT_KEYS
            .iter()
            .map(|key| key.as_str())
            .collect::<Vec<_>>();
        self.clear_state(&names)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn store() -> Store {
        Store::open_in_memory(&[7; 32]).unwrap()
    }

    fn session(user_id: &str) -> Session {
        Session {
            access_token: format!("access-{user_id}"),
            refresh_token: format!("refresh-{user_id}"),
            user_id: user_id.into(),
            expires_at_ms: 123_000,
        }
    }

    #[test]
    fn schema_names_and_key_serialization_are_canonical() {
        assert_eq!(
            [
                CloudStateKey::Email,
                CloudStateKey::UserId,
                CloudStateKey::AccessToken,
                CloudStateKey::RefreshToken,
                CloudStateKey::ExpiresAtMs,
                CloudStateKey::SyncKey,
                CloudStateKey::SessionUserId,
                CloudStateKey::SyncKeyUserId,
                CloudStateKey::CursorUserId,
                CloudStateKey::LastSyncMs,
                CloudStateKey::WatermarkMs,
                CloudStateKey::WatermarkItemId,
                CloudStateKey::UploadFloorMs,
                CloudStateKey::UploadFloorItemId,
            ]
            .map(CloudStateKey::as_str),
            [
                "cloud_email",
                "cloud_user_id",
                "cloud_access_token",
                "cloud_refresh_token",
                "cloud_expires_at_ms",
                "cloud_sync_key",
                "cloud_session_user_id",
                "cloud_sync_key_user_id",
                "cloud_cursor_user_id",
                "cloud_last_sync_ms",
                "cloud_watermark_ms",
                "cloud_watermark_item_id",
                "cloud_upload_floor_ms",
                "cloud_upload_floor_item_id",
            ]
        );
        let store = store();
        store
            .replace_cloud_credentials(
                "a@example.com",
                &session("user-a"),
                &[0xab; 32],
                AccountChange::SwitchedAccount,
            )
            .unwrap();
        assert_eq!(
            store
                .state(CloudStateKey::SyncKey.as_str())
                .unwrap()
                .as_deref(),
            Some("abababababababababababababababababababababababababababababababab")
        );
    }

    #[test]
    fn matching_user_binding_round_trips_every_secret() {
        let store = store();
        store
            .replace_cloud_credentials(
                "a@example.com",
                &session("user-a"),
                &[7; 32],
                AccountChange::SwitchedAccount,
            )
            .unwrap();
        let restored = store.cloud_credentials().unwrap().unwrap();
        assert_eq!(restored.email, "a@example.com");
        assert_eq!(restored.session.user_id, "user-a");
        assert_eq!(restored.session.access_token, "access-user-a");
        assert_eq!(*restored.sync_key.to_bytes(), [7; 32]);
    }

    #[test]
    fn mismatched_binding_is_rejected_and_cleaned() {
        let store = store();
        store
            .replace_cloud_credentials(
                "a@example.com",
                &session("user-a"),
                &[7; 32],
                AccountChange::SwitchedAccount,
            )
            .unwrap();
        store
            .set_state(CloudStateKey::SyncKeyUserId.as_str(), "user-b")
            .unwrap();
        assert!(store.cloud_credentials().unwrap().is_none());
        for key in SIGN_OUT_KEYS {
            assert_eq!(store.state(key.as_str()).unwrap(), None);
        }
    }

    #[test]
    fn partial_and_noncanonical_state_is_rejected_and_cleaned() {
        let store = store();
        store
            .set_state(CloudStateKey::AccessToken.as_str(), "access")
            .unwrap();
        assert!(store.cloud_credentials().unwrap().is_none());
        for key in SIGN_OUT_KEYS {
            assert_eq!(store.state(key.as_str()).unwrap(), None);
        }
        store
            .replace_cloud_credentials(
                "a@example.com",
                &session("user-a"),
                &[0xab; 32],
                AccountChange::SwitchedAccount,
            )
            .unwrap();
        store
            .set_state(
                CloudStateKey::SyncKey.as_str(),
                "ABABABABABABABABABABABABABABABABABABABABABABABABABABABABABAB",
            )
            .unwrap();
        assert!(store.cloud_credentials().unwrap().is_none());
    }

    #[test]
    fn missing_credentials_do_not_clear_the_cursor_owner() {
        let store = store();
        store
            .set_state(CloudStateKey::CursorUserId.as_str(), "user-a")
            .unwrap();

        assert!(store.cloud_credentials().unwrap().is_none());
        assert_eq!(
            store.state(CloudStateKey::CursorUserId.as_str()).unwrap(),
            Some("user-a".into())
        );
    }

    #[test]
    fn account_switch_replaces_every_owned_value() {
        let store = store();
        store
            .replace_cloud_credentials(
                "a@example.com",
                &session("user-a"),
                &[1; 32],
                AccountChange::SwitchedAccount,
            )
            .unwrap();
        store
            .set_state(CloudStateKey::WatermarkMs.as_str(), "42")
            .unwrap();
        store
            .set_state(CloudStateKey::WatermarkItemId.as_str(), "old-watermark")
            .unwrap();
        store
            .set_state(CloudStateKey::UploadFloorMs.as_str(), "43")
            .unwrap();
        store
            .set_state(CloudStateKey::UploadFloorItemId.as_str(), "old-floor")
            .unwrap();
        store
            .set_state(CloudStateKey::LastSyncMs.as_str(), "44")
            .unwrap();
        store
            .replace_cloud_credentials(
                "b@example.com",
                &session("user-b"),
                &[2; 32],
                AccountChange::SwitchedAccount,
            )
            .unwrap();
        let restored = store.cloud_credentials().unwrap().unwrap();
        assert_eq!(restored.email, "b@example.com");
        assert_eq!(restored.session.user_id, "user-b");
        assert_eq!(*restored.sync_key.to_bytes(), [2; 32]);
        assert_eq!(
            store.state(CloudStateKey::CursorUserId.as_str()).unwrap(),
            Some("user-b".into())
        );
        assert_eq!(
            store.state(CloudStateKey::WatermarkMs.as_str()).unwrap(),
            Some("0".into())
        );
        assert_eq!(
            store
                .state(CloudStateKey::WatermarkItemId.as_str())
                .unwrap(),
            None
        );
        assert_eq!(
            store.state(CloudStateKey::UploadFloorMs.as_str()).unwrap(),
            Some("0".into())
        );
        assert_eq!(
            store
                .state(CloudStateKey::UploadFloorItemId.as_str())
                .unwrap(),
            None
        );
        assert_eq!(
            store.state(CloudStateKey::LastSyncMs.as_str()).unwrap(),
            None
        );
    }

    #[test]
    fn same_account_activation_preserves_download_progress() {
        let store = store();
        store
            .replace_cloud_credentials(
                "a@example.com",
                &session("user-a"),
                &[1; 32],
                AccountChange::SwitchedAccount,
            )
            .unwrap();
        store
            .set_state(CloudStateKey::WatermarkMs.as_str(), "42")
            .unwrap();
        store
            .set_state(CloudStateKey::WatermarkItemId.as_str(), "watermark")
            .unwrap();
        store
            .set_state(CloudStateKey::UploadFloorMs.as_str(), "43")
            .unwrap();
        store
            .set_state(CloudStateKey::UploadFloorItemId.as_str(), "floor")
            .unwrap();
        store
            .set_state(CloudStateKey::LastSyncMs.as_str(), "44")
            .unwrap();

        store
            .replace_cloud_credentials(
                "a@example.com",
                &session("user-a"),
                &[2; 32],
                AccountChange::SameAccount,
            )
            .unwrap();

        assert_eq!(
            store.state(CloudStateKey::WatermarkMs.as_str()).unwrap(),
            Some("42".into())
        );
        assert_eq!(
            store
                .state(CloudStateKey::WatermarkItemId.as_str())
                .unwrap(),
            Some("watermark".into())
        );
        assert_eq!(
            store.state(CloudStateKey::LastSyncMs.as_str()).unwrap(),
            Some("44".into())
        );
        assert_eq!(
            store.state(CloudStateKey::UploadFloorMs.as_str()).unwrap(),
            Some("0".into())
        );
        assert_eq!(
            store
                .state(CloudStateKey::UploadFloorItemId.as_str())
                .unwrap(),
            None
        );
    }

    #[test]
    fn a_stale_session_cannot_overwrite_the_replacement_account() {
        let store = store();
        store
            .replace_cloud_credentials(
                "b@example.com",
                &session("user-b"),
                &[2; 32],
                AccountChange::SwitchedAccount,
            )
            .unwrap();
        let error = store.update_cloud_session(&session("user-a")).unwrap_err();
        assert!(matches!(error, CredentialError::AccountMismatch));
        let restored = store.cloud_credentials().unwrap().unwrap();
        assert_eq!(restored.session.user_id, "user-b");
        assert_eq!(restored.session.access_token, "access-user-b");
    }

    #[test]
    fn clearing_credentials_preserves_the_cursor_owner_and_cursors() {
        let store = store();
        store
            .replace_cloud_credentials(
                "a@example.com",
                &session("user-a"),
                &[7; 32],
                AccountChange::SwitchedAccount,
            )
            .unwrap();
        store
            .set_state(CloudStateKey::WatermarkMs.as_str(), "42")
            .unwrap();

        store.clear_cloud_credentials().unwrap();

        for key in SIGN_OUT_KEYS {
            assert_eq!(store.state(key.as_str()).unwrap(), None);
        }
        assert_eq!(
            store
                .state(CloudStateKey::CursorUserId.as_str())
                .unwrap()
                .as_deref(),
            Some("user-a")
        );
        assert_eq!(
            store
                .state(CloudStateKey::WatermarkMs.as_str())
                .unwrap()
                .as_deref(),
            Some("42")
        );
    }
}
