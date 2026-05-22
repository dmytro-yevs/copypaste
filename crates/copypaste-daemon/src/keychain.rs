use copypaste_core::DeviceKeypair;
use thiserror::Error;

#[cfg(target_os = "macos")]
use security_framework::passwords::{delete_generic_password, get_generic_password, set_generic_password};
#[cfg(target_os = "macos")]
use security_framework::base::Error as SfError;

const SERVICE: &str = "com.copypaste.daemon";
const ACCOUNT: &str = "device-secret-key";

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
                tracing::info!("Generated new device keypair; fingerprint={}", kp.fingerprint());
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
    #[cfg(target_os = "macos")]
    use super::*;

    #[cfg(target_os = "macos")]
    #[test]
    fn load_or_create_returns_keypair() {
        let _ = delete_stored();
        let kp = load_or_create().expect("should create keypair");
        assert_eq!(kp.secret_key_bytes().len(), 32);
        delete_stored().unwrap();
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn load_or_create_is_idempotent() {
        let _ = delete_stored();
        let kp1 = load_or_create().unwrap();
        let kp2 = load_or_create().unwrap();
        assert_eq!(kp1.secret_key_bytes(), kp2.secret_key_bytes());
        delete_stored().unwrap();
    }
}
