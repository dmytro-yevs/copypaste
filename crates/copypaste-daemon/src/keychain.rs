use copypaste_core::DeviceKeypair;
use sha2::{Digest, Sha256};
use thiserror::Error;

#[cfg(target_os = "macos")]
use security_framework::base::Error as SfError;
#[cfg(target_os = "macos")]
use security_framework::passwords::{
    delete_generic_password, get_generic_password, set_generic_password,
};

const SERVICE: &str = "com.copypaste.daemon";
const ACCOUNT: &str = "device-secret-key";

/// Compute the canonical device fingerprint from a raw public key.
///
/// Format: first 16 bytes of `SHA-256(public_key)` rendered as
/// lowercase hex pairs separated by `:` (e.g. `aa:bb:cc:...`).
/// This is the user-visible identifier shown during pairing — keep it short
/// enough for humans to compare on two screens.
pub fn own_fingerprint(public_key: &[u8]) -> String {
    let digest = Sha256::digest(public_key);
    digest[..16]
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<Vec<_>>()
        .join(":")
}

#[derive(Debug, Error)]
pub enum KeychainError {
    #[error("Key is wrong length: expected 32 bytes, got {0}")]
    InvalidLength(usize),
    #[cfg(target_os = "macos")]
    #[error("Keychain error: {0}")]
    Keychain(#[from] SfError),
    #[cfg(not(target_os = "macos"))]
    #[error("Keychain not supported on this platform")]
    Unsupported,
    #[error("Core key error: {0}")]
    Key(#[from] copypaste_core::KeyError),
}

/// Load device keypair from Keychain, or generate and store a new one.
pub fn load_or_create() -> Result<DeviceKeypair, KeychainError> {
    #[cfg(target_os = "macos")]
    {
        match get_generic_password(SERVICE, ACCOUNT) {
            Ok(bytes) => {
                if bytes.len() != 32 {
                    return Err(KeychainError::InvalidLength(bytes.len()));
                }
                let arr: [u8; 32] = bytes.try_into().unwrap();
                Ok(DeviceKeypair::from_secret_bytes(&arr)?)
            }
            Err(_) => {
                let kp = DeviceKeypair::generate();
                set_generic_password(SERVICE, ACCOUNT, &kp.secret_key_bytes())?;
                let fp = own_fingerprint(&kp.public_key_bytes());
                // Log only the short prefix to keep full fingerprint out of info logs.
                tracing::info!(
                    "Generated new device keypair; fingerprint_prefix={}",
                    &fp[..23]
                );
                tracing::debug!("full device fingerprint={}", fp);
                Ok(kp)
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    Err(KeychainError::Unsupported)
}

/// Delete the stored keypair — used for testing and factory reset.
#[cfg(target_os = "macos")]
pub fn delete_stored() -> Result<(), KeychainError> {
    delete_generic_password(SERVICE, ACCOUNT).map_err(KeychainError::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn own_fingerprint_is_sha256_prefix() {
        let pk = [0u8; 32];
        let fp = own_fingerprint(&pk);
        // SHA-256 of 32 zero bytes is known: 66687aadf862bd776c8fc18b8e9f8e20...
        assert!(fp.starts_with("66:68:7a:ad:f8:62:bd:77:6c:8f:c1:8b:8e:9f:8e:20"));
        assert_eq!(fp.matches(':').count(), 15); // 16 bytes = 15 colons
    }

    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "requires interactive Keychain access; run manually with `cargo test -- --ignored`"]
    fn load_or_create_returns_keypair() {
        let _ = delete_stored();
        let kp = load_or_create().expect("should create keypair");
        assert_eq!(kp.secret_key_bytes().len(), 32);
        delete_stored().unwrap();
    }

    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "requires interactive Keychain access; run manually with `cargo test -- --ignored`"]
    fn load_or_create_is_idempotent() {
        let _ = delete_stored();
        let kp1 = load_or_create().unwrap();
        let kp2 = load_or_create().unwrap();
        assert_eq!(kp1.secret_key_bytes(), kp2.secret_key_bytes());
        delete_stored().unwrap();
    }
}
