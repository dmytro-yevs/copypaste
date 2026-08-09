use copypaste_cloud::auth::Session;
use copypaste_cloud::SyncKey;

use super::Driver;

pub(super) const KEY_EMAIL: &str = "cloud_email";
pub(super) const KEY_USER_ID: &str = "cloud_user_id";
pub(super) const KEY_ACCESS: &str = "cloud_access_token";
pub(super) const KEY_REFRESH: &str = "cloud_refresh_token";
pub(super) const KEY_EXPIRES: &str = "cloud_expires_at_ms";
pub(super) const KEY_SYNC_KEY: &str = "cloud_sync_key";
pub(super) const KEY_LAST_SYNC: &str = "cloud_last_sync_ms";
pub(super) const CREDENTIAL_KEYS: &[&str] = &[
    KEY_EMAIL,
    KEY_USER_ID,
    KEY_ACCESS,
    KEY_REFRESH,
    KEY_EXPIRES,
    KEY_SYNC_KEY,
    KEY_LAST_SYNC,
];

pub(super) fn read_session(store: &copypaste_core::Store) -> Option<Session> {
    Some(Session {
        access_token: store.state(KEY_ACCESS).ok()??,
        refresh_token: store.state(KEY_REFRESH).ok()??,
        user_id: store.state(KEY_USER_ID).ok()??,
        expires_at_ms: store.state_ms(KEY_EXPIRES).ok()?,
    })
}

pub(super) fn read_key(store: &copypaste_core::Store) -> Option<SyncKey> {
    let bytes: [u8; 32] = hex::decode(store.state(KEY_SYNC_KEY).ok()??)
        .ok()?
        .try_into()
        .ok()?;
    Some(SyncKey::from_bytes(bytes))
}

pub(super) fn write_session(
    store: &copypaste_core::Store,
    driver: &Driver,
) -> Result<(), copypaste_core::StoreError> {
    driver.inspect_session(|session| {
        store.set_state_all(&[
            (KEY_ACCESS, &session.access_token),
            (KEY_REFRESH, &session.refresh_token),
            (KEY_EXPIRES, &session.expires_at_ms.to_string()),
        ])
    })
}
