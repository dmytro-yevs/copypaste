//! Canonical provider account-state names shared by host and provider code.

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
    UnreadableUploads,
    EndpointUrl,
    EndpointAnonKey,
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
            Self::UnreadableUploads => "cloud_unreadable_uploads",
            Self::EndpointUrl => "cloud_endpoint_url",
            Self::EndpointAnonKey => "cloud_endpoint_anon_key",
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
    // The unreadable-upload list is only meaningful while an account is
    // waiting for those rows; leaving it behind would show a signed-out device
    // a count it can do nothing about.
    CloudStateKey::UnreadableUploads,
];
