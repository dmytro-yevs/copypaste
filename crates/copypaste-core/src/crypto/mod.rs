pub mod chunks;
pub mod encrypt;
mod keys;
pub mod pairing_qr;
pub mod sync_key;
pub use keys::{
    derive_storage_key_v1, derive_storage_key_v2, derive_sync_key_v2, derive_telemetry_key_v2,
    derive_v2, hkdf_v2_pair_salt, DeviceKeypair, KeyError, HKDF_SALT_V1, HKDF_SALT_V2_BASE,
    HKDF_VERSION,
};
pub use pairing_qr::{
    strip_deeplink, PairingPayload, PairingQrError, PairingToken, QrProvisioning,
    PAIRING_DEEPLINK_PREFIX, PAIRING_QR_MAGIC, PAIRING_TOKEN_LEN,
};
pub use sync_key::{
    decrypt_from_cloud, decrypt_from_cloud_trying, derive_sync_key, derive_sync_key_for_account,
    derive_sync_key_versioned, encrypt_for_cloud, SyncKey, SyncKeyError, ARGON2_M_COST_KIB,
    ARGON2_P_COST, ARGON2_SYNC_SALT,
    ARGON2_T_COST, CLOUD_AAD_SCHEMA_VERSION, SYNC_KEY_DERIVATION_VERSION_V1,
    SYNC_KEY_DERIVATION_VERSION_V2,
};
