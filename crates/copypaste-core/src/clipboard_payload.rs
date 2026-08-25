//! Decrypted item content, classified once for every clipboard writer.

use zeroize::Zeroizing;

use crate::{decrypt, open_binary, CryptoError, FileMetadata, ItemKey, StoredItem};

/// An authenticated history payload ready for presentation or a native write.
///
/// Binary display labels are deliberately available only through
/// [`Self::display_text`]. A clipboard writer must match the variant, so an
/// image, file, or future payload cannot be pasted as `[image]`, `[file]`, or
/// `[unsupported]` by accidentally reusing presentation text.
#[derive(Debug)]
pub enum ClipboardPayload {
    Text(Zeroizing<String>),
    Image {
        content_type: String,
        bytes: Zeroizing<Vec<u8>>,
    },
    File {
        bytes: Zeroizing<Vec<u8>>,
        metadata: Option<FileMetadata>,
    },
    Unsupported {
        bytes: Zeroizing<Vec<u8>>,
    },
}

impl ClipboardPayload {
    /// Authenticate and classify one stored row.
    ///
    /// `copypaste_ipc::content_type::Kind` remains the one vocabulary owner.
    /// This type lives in `copypaste-core` because that crate already owns both
    /// `StoredItem` and its text/binary AEAD readers; adding a platform crate or
    /// a second decryption adapter would duplicate that trust boundary.
    pub fn open(row: &StoredItem, key: &ItemKey) -> Result<Self, CryptoError> {
        use copypaste_ipc::content_type::Kind;

        let binary = || open_binary(&row.content_ciphertext, key, &row.id);
        match copypaste_ipc::content_type::classify(&row.content_type) {
            Kind::Text => {
                let bytes = decrypt(&row.content_ciphertext, &row.nonce, key, &row.id)?;
                Ok(Self::Text(Zeroizing::new(
                    String::from_utf8_lossy(&bytes).into_owned(),
                )))
            }
            Kind::Image => Ok(Self::Image {
                content_type: row.content_type.clone(),
                bytes: binary()?,
            }),
            Kind::File => Ok(Self::File {
                bytes: binary()?,
                metadata: row
                    .payload_metadata
                    .as_deref()
                    .and_then(FileMetadata::from_json),
            }),
            Kind::Other => Ok(Self::Unsupported { bytes: binary()? }),
        }
    }

    #[must_use]
    pub fn display_text(&self) -> String {
        match self {
            Self::Text(text) => text.to_string(),
            Self::Image { content_type, .. } => {
                format!("[{}]", copypaste_ipc::content_type::label(content_type))
            }
            Self::File { .. } => format!(
                "[{}]",
                copypaste_ipc::content_type::label(copypaste_ipc::content_type::FILE)
            ),
            Self::Unsupported { .. } => {
                format!("[{}]", copypaste_ipc::content_type::label(""))
            }
        }
    }

    /// The only variants with a meaningful plain-text representation.
    #[must_use]
    pub fn plain_text(&self) -> Option<&str> {
        match self {
            Self::Text(text) => Some(text.as_str()),
            Self::Image { .. } | Self::File { .. } | Self::Unsupported { .. } => None,
        }
    }

    #[must_use]
    pub fn byte_len(&self) -> usize {
        match self {
            Self::Text(text) => text.len(),
            Self::Image { bytes, .. } | Self::File { bytes, .. } | Self::Unsupported { bytes } => {
                bytes.len()
            }
        }
    }
}

/// A platform clipboard refusal with no content, MIME type, or path attached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ClipboardWriteError {
    #[error("this clipboard cannot write that content type")]
    UnsupportedContent,
    #[error("the system clipboard could not be written")]
    Failed,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{encrypt, seal_binary, NewItem, Store};

    fn stored(
        content_type: &str,
        bytes: &[u8],
        file: Option<FileMetadata>,
    ) -> (StoredItem, ItemKey) {
        let dir = tempfile::tempdir().unwrap();
        let keyring = crate::Keyring::from_secret(&[7; 32]);
        let store = Store::open(&dir.path().join("payload.db"), &keyring.db_key()).unwrap();
        let id = if copypaste_ipc::content_type::is_binary(content_type) {
            crate::binary_item_id(bytes)
        } else {
            "text-item".to_string()
        };
        let (content_ciphertext, nonce) = if copypaste_ipc::content_type::is_binary(content_type) {
            (
                seal_binary(bytes, &keyring.item_key(), &id).unwrap(),
                Vec::new(),
            )
        } else {
            let (nonce, ciphertext) = encrypt(bytes, &keyring.item_key(), &id).unwrap();
            (ciphertext, nonce)
        };
        store
            .insert(NewItem {
                id: id.clone(),
                content_ciphertext,
                nonce,
                content_type: content_type.to_string(),
                content_hash: crate::compute_content_hash(bytes),
                is_sensitive: false,
                search_text: copypaste_ipc::content_type::is_text(content_type)
                    .then(|| String::from_utf8_lossy(bytes).into_owned()),
                created_at: 1,
                app_bundle_id: None,
                app_name: None,
                payload_metadata: file.map(|metadata| serde_json::to_string(&metadata).unwrap()),
            })
            .unwrap();
        let row = store.get(&id).unwrap().unwrap();
        let key = keyring.item_key();
        (row, key)
    }

    #[test]
    fn text_is_the_only_plain_text_clipboard_payload() {
        let (row, key) = stored(copypaste_ipc::content_type::TEXT, b"full body", None);
        let payload = ClipboardPayload::open(&row, &key).unwrap();
        assert_eq!(payload.plain_text(), Some("full body"));
        assert_eq!(payload.display_text(), "full body");
    }

    #[test]
    fn image_file_and_unknown_keep_bytes_separate_from_display_labels() {
        let file = FileMetadata::new("note.bin", "application/octet-stream").unwrap();
        for (content_type, bytes, metadata, expected) in [
            (
                copypaste_ipc::content_type::IMAGE_PNG,
                b"image bytes".as_slice(),
                None,
                "[image]",
            ),
            (
                copypaste_ipc::content_type::FILE,
                b"file bytes".as_slice(),
                Some(file),
                "[file]",
            ),
            (
                "application/x-future",
                b"future bytes".as_slice(),
                None,
                "[unsupported]",
            ),
        ] {
            let (row, key) = stored(content_type, bytes, metadata);
            let payload = ClipboardPayload::open(&row, &key).unwrap();
            assert_eq!(payload.byte_len(), bytes.len());
            assert_eq!(payload.display_text(), expected);
            assert_eq!(payload.plain_text(), None, "{content_type}");
        }
    }
}
