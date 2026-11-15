//! Encrypted file objects in cloud storage: opaque names and sealed chunks.
//!
//! An item's file is a sealed manifest (index 0: size and SHA-256) and data
//! objects 1..=n of at most [`OBJECT_PLAINTEXT_BYTES`]. Each object is
//! XChaCha20-Poly1305 under a key derived from the account key, with the
//! object id, index, chunk count, size and key epoch as associated data, so
//! objects cannot be swapped, reordered or truncated. The last chunk is padded
//! to a [`PADDING_BLOCK`] boundary inside the ciphertext.

use chacha20poly1305::{
    aead::{Aead, AeadInPlace, KeyInit, Payload},
    Key, Tag, XChaCha20Poly1305, XNonce,
};
use hkdf::Hkdf;
use hmac::{Hmac, Mac};
use rand::{rngs::OsRng, Rng};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use zeroize::Zeroizing;

use super::{
    error::CloudCryptoError,
    row::{NONCE_LEN, TAG_LEN},
    KEY_LEN,
};

/// Plaintext bytes per data object.
pub const OBJECT_PLAINTEXT_BYTES: usize = 4 * 1024 * 1024;
/// Data objects are padded to a multiple of this inside the ciphertext.
pub const PADDING_BLOCK: usize = 64 * 1024;
/// Largest file uploaded to cloud storage.
pub const MAX_CLOUD_FILE_BYTES: u64 = 64 * 1024 * 1024;

const INFO_SEAL: &[u8] = b"copypaste/v2/cloud-file/xchacha20poly1305";
const INFO_NAME: &[u8] = b"copypaste/v2/cloud-file/object-name";

/// What the manifest object carries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FileManifest {
    pub size_bytes: u64,
    pub sha256: String,
}

impl FileManifest {
    /// Data objects after the manifest.
    #[must_use]
    pub fn chunk_count(&self) -> u64 {
        self.size_bytes.div_ceil(OBJECT_PLAINTEXT_BYTES as u64)
    }
}

/// Keys for one account key epoch. Zeroized on drop.
pub struct FileKeys {
    seal: Zeroizing<[u8; KEY_LEN]>,
    name: Zeroizing<[u8; KEY_LEN]>,
    epoch: u64,
}

/// The only derivation of cloud file keys. `base` is the account key material
/// of `epoch`; epoch 0 is the passphrase-derived sync key.
#[must_use]
pub fn file_keys(base: &[u8; KEY_LEN], epoch: u64) -> FileKeys {
    let expand = |info: &[u8]| {
        let mut key = Zeroizing::new([0u8; KEY_LEN]);
        Hkdf::<Sha256>::from_prk(base)
            .expect("a 32-byte key is a valid PRK")
            .expand(info, key.as_mut())
            .expect("32 bytes is a valid HKDF output length");
        key
    };
    FileKeys {
        seal: expand(INFO_SEAL),
        name: expand(INFO_NAME),
        epoch,
    }
}

impl FileKeys {
    /// Opaque, stable name for one file of an item: no item id, hash or file
    /// name. `file` is the member index of a multi-file clip, 0 otherwise.
    #[must_use]
    pub fn object_id(&self, item_id: &str, file: u32) -> String {
        let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(self.name.as_ref())
            .expect("HMAC accepts any key length");
        mac.update(item_id.as_bytes());
        mac.update(&[0]);
        mac.update(&file.to_be_bytes());
        hex::encode(&mac.finalize().into_bytes()[..16])
    }

    fn aad(&self, object_id: &str, index: u64, count: u64, size: u64) -> Vec<u8> {
        format!(
            "copypaste/v2/cloud-file\0{object_id}\0{index}\0{count}\0{size}\0{}",
            self.epoch
        )
        .into_bytes()
    }

    fn seal(&self, aad: &[u8], plaintext: &[u8]) -> Result<Vec<u8>, CloudCryptoError> {
        let cipher = XChaCha20Poly1305::new(Key::from_slice(self.seal.as_ref()));
        let nonce: [u8; NONCE_LEN] = OsRng.gen();
        let sealed = cipher
            .encrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: plaintext,
                    aad,
                },
            )
            .map_err(|_| CloudCryptoError::Internal("file seal"))?;
        let mut object = Vec::with_capacity(NONCE_LEN + sealed.len());
        object.extend_from_slice(&nonce);
        object.extend_from_slice(&sealed);
        Ok(object)
    }

    fn open(&self, aad: &[u8], object: &[u8]) -> Result<Zeroizing<Vec<u8>>, CloudCryptoError> {
        if object.len() < NONCE_LEN {
            return Err(CloudCryptoError::AuthFailed);
        }
        let (nonce, sealed) = object.split_at(NONCE_LEN);
        XChaCha20Poly1305::new(Key::from_slice(self.seal.as_ref()))
            .decrypt(XNonce::from_slice(nonce), Payload { msg: sealed, aad })
            .map(Zeroizing::new)
            .map_err(|_| CloudCryptoError::AuthFailed)
    }

    pub fn seal_manifest(
        &self,
        object_id: &str,
        manifest: &FileManifest,
    ) -> Result<Vec<u8>, CloudCryptoError> {
        let json = serde_json::to_vec(manifest).map_err(|_| CloudCryptoError::Malformed)?;
        self.seal(&self.aad(object_id, 0, 0, 0), &json)
    }

    pub fn open_manifest(
        &self,
        object_id: &str,
        object: &[u8],
    ) -> Result<FileManifest, CloudCryptoError> {
        let json = self.open(&self.aad(object_id, 0, 0, 0), object)?;
        let manifest: FileManifest =
            serde_json::from_slice(&json).map_err(|_| CloudCryptoError::AuthFailed)?;
        if manifest.size_bytes > MAX_CLOUD_FILE_BYTES || manifest.sha256.len() != 64 {
            return Err(CloudCryptoError::AuthFailed);
        }
        Ok(manifest)
    }

    /// Seals data object `index` (1-based) of a file described by `manifest`.
    /// The object is built and encrypted in one buffer, so sealing holds no
    /// second copy of the chunk.
    pub fn seal_chunk(
        &self,
        object_id: &str,
        manifest: &FileManifest,
        index: u64,
        plaintext: &[u8],
    ) -> Result<Vec<u8>, CloudCryptoError> {
        if plaintext.len() as u64 != chunk_len(manifest, index)? {
            return Err(CloudCryptoError::Malformed);
        }
        let padded = plaintext.len().next_multiple_of(PADDING_BLOCK);
        let nonce: [u8; NONCE_LEN] = OsRng.gen();
        let mut object = Zeroizing::new(Vec::with_capacity(NONCE_LEN + padded + TAG_LEN));
        object.extend_from_slice(&nonce);
        object.extend_from_slice(plaintext);
        object.resize(NONCE_LEN + padded, 0);
        let tag = XChaCha20Poly1305::new(Key::from_slice(self.seal.as_ref()))
            .encrypt_in_place_detached(
                XNonce::from_slice(&nonce),
                &self.chunk_aad(object_id, manifest, index),
                &mut object[NONCE_LEN..],
            )
            .map_err(|_| CloudCryptoError::Internal("file seal"))?;
        object.extend_from_slice(&tag);
        Ok(std::mem::take(&mut *object))
    }

    /// Opens data object `index` in place and returns its unpadded bytes.
    pub fn open_chunk(
        &self,
        object_id: &str,
        manifest: &FileManifest,
        index: u64,
        object: Vec<u8>,
    ) -> Result<Zeroizing<Vec<u8>>, CloudCryptoError> {
        let expected =
            chunk_len(manifest, index).map_err(|_| CloudCryptoError::AuthFailed)? as usize;
        let mut buffer = Zeroizing::new(object);
        let Some(body_end) = buffer
            .len()
            .checked_sub(TAG_LEN)
            .filter(|end| *end >= NONCE_LEN)
        else {
            return Err(CloudCryptoError::AuthFailed);
        };
        let (head, tag) = buffer.split_at_mut(body_end);
        let tag = Tag::clone_from_slice(tag);
        let (nonce, body) = head.split_at_mut(NONCE_LEN);
        XChaCha20Poly1305::new(Key::from_slice(self.seal.as_ref()))
            .decrypt_in_place_detached(
                XNonce::from_slice(nonce),
                &self.chunk_aad(object_id, manifest, index),
                body,
                &tag,
            )
            .map_err(|_| CloudCryptoError::AuthFailed)?;
        buffer.truncate(body_end);
        buffer.drain(..NONCE_LEN);
        if buffer.len() != expected.next_multiple_of(PADDING_BLOCK)
            || buffer[expected..].iter().any(|byte| *byte != 0)
        {
            return Err(CloudCryptoError::AuthFailed);
        }
        buffer.truncate(expected);
        Ok(buffer)
    }

    fn chunk_aad(&self, object_id: &str, manifest: &FileManifest, index: u64) -> Vec<u8> {
        self.aad(
            object_id,
            index,
            manifest.chunk_count(),
            manifest.size_bytes,
        )
    }
}

fn chunk_len(manifest: &FileManifest, index: u64) -> Result<u64, CloudCryptoError> {
    if index == 0 || index > manifest.chunk_count() {
        return Err(CloudCryptoError::Malformed);
    }
    let offset = (index - 1) * OBJECT_PLAINTEXT_BYTES as u64;
    Ok((manifest.size_bytes - offset).min(OBJECT_PLAINTEXT_BYTES as u64))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_open_only_in_place_and_under_the_same_key() {
        let keys = file_keys(&[3; KEY_LEN], 0);
        let id = keys.object_id("file-offer-1", 0);
        assert_eq!(id.len(), 32);
        assert_ne!(id, keys.object_id("file-offer-2", 0));
        assert_ne!(id, keys.object_id("file-offer-1", 1));
        let manifest = FileManifest {
            size_bytes: OBJECT_PLAINTEXT_BYTES as u64 + 5,
            sha256: "a".repeat(64),
        };
        let sealed = keys.seal_manifest(&id, &manifest).unwrap();
        assert_eq!(keys.open_manifest(&id, &sealed).unwrap(), manifest);

        let tail = keys.seal_chunk(&id, &manifest, 2, b"tail!").unwrap();
        assert_eq!(tail.len(), NONCE_LEN + PADDING_BLOCK + 16, "padded");
        assert_eq!(
            &*keys.open_chunk(&id, &manifest, 2, tail.clone()).unwrap(),
            b"tail!"
        );
        // Another position, item, epoch or account key refuses it.
        assert!(keys.open_chunk(&id, &manifest, 1, tail.clone()).is_err());
        assert!(keys
            .open_chunk(
                &keys.object_id("file-offer-2", 0),
                &manifest,
                2,
                tail.clone()
            )
            .is_err());
        assert!(file_keys(&[3; KEY_LEN], 1)
            .open_chunk(&id, &manifest, 2, tail)
            .is_err());
        assert!(file_keys(&[4; KEY_LEN], 0)
            .open_manifest(&id, &sealed)
            .is_err());
    }
}
