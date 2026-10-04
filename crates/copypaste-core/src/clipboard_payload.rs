//! Decrypted item content for every clipboard writer.

use zeroize::Zeroizing;

use crate::{
    decrypt, open_binary, CryptoError, FileMetadata, ItemKey, PayloadMetadata, StoredItem,
};

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
    /// Authenticate and decode one stored row.
    ///
    /// `copypaste_ipc::ContentClass` remains the one vocabulary owner.
    /// This type lives in `copypaste-core` because that crate already owns both
    /// `StoredItem` and its text/binary AEAD readers; adding a platform crate or
    /// a second decryption adapter would duplicate that trust boundary.
    pub fn open(row: &StoredItem, key: &ItemKey) -> Result<Self, CryptoError> {
        use copypaste_ipc::ContentClass;

        let binary = || open_binary(&row.content_ciphertext, key, &row.id);
        match copypaste_ipc::content_type::classify(&row.content_type) {
            ContentClass::Text => {
                let bytes = decrypt(&row.content_ciphertext, &row.nonce, key, &row.id)?;
                Ok(Self::Text(text_from_bytes(bytes)))
            }
            ContentClass::Image => Ok(Self::Image {
                content_type: row.content_type.clone(),
                bytes: binary()?,
            }),
            ContentClass::File => Ok(Self::File {
                bytes: binary()?,
                metadata: row
                    .payload_metadata
                    .as_deref()
                    .and_then(|metadata| PayloadMetadata::from_json(metadata, &row.content_type))
                    .and_then(|metadata| metadata.file),
            }),
            ContentClass::Other => Ok(Self::Unsupported { bytes: binary()? }),
        }
    }

    #[must_use]
    pub fn display_text(&self) -> String {
        match self {
            Self::Text(text) => text.to_string(),
            Self::Image { content_type, .. } => {
                format!("[{}]", copypaste_ipc::content_type::label(content_type))
            }
            Self::File { metadata, .. } => metadata
                .as_ref()
                .and_then(|metadata| metadata.source_reference.clone())
                .unwrap_or_else(|| {
                    format!(
                        "[{}]",
                        copypaste_ipc::content_type::label(copypaste_ipc::content_type::FILE)
                    )
                }),
            Self::Unsupported { .. } => {
                format!("[{}]", copypaste_ipc::content_type::label(""))
            }
        }
    }

    /// Build a list/search preview without cloning the full authenticated body.
    #[must_use]
    pub fn display_preview(&self) -> (String, bool) {
        match self {
            Self::Text(text) => {
                let (preview, truncated) = copypaste_ipc::limits::preview_text(text);
                (preview.to_owned(), truncated)
            }
            Self::File { metadata, .. } => metadata
                .as_ref()
                .and_then(|metadata| metadata.source_reference.as_deref())
                .map(|reference| {
                    let (preview, truncated) = copypaste_ipc::limits::preview_text(reference);
                    (preview.to_owned(), truncated)
                })
                .unwrap_or_else(|| (self.display_text(), false)),
            _ => (self.display_text(), false),
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
    pub fn source_reference(&self) -> Option<&str> {
        match self {
            Self::File { metadata, .. } => metadata
                .as_ref()
                .and_then(|metadata| metadata.source_reference.as_deref()),
            Self::Text(_) | Self::Image { .. } | Self::Unsupported { .. } => None,
        }
    }

    /// Write the authenticated bytes of one file clip to a destination selected
    /// by the user. The destination must not exist so a stale dialog result can
    /// never overwrite an unrelated file.
    pub fn save_file_to(&self, destination: &std::path::Path) -> std::io::Result<()> {
        let Self::File { bytes, .. } = self else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "the clipboard payload is not a file",
            ));
        };
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(destination)?;
        if let Err(error) = std::io::Write::write_all(&mut file, bytes) {
            drop(file);
            let _ = std::fs::remove_file(destination);
            return Err(error);
        }
        Ok(())
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

fn text_from_bytes(mut bytes: Zeroizing<Vec<u8>>) -> Zeroizing<String> {
    // A valid UTF-8 body can keep the decryption buffer. Preserve the existing
    // lossy behavior for legacy text, and zeroize its original invalid bytes.
    match String::from_utf8(std::mem::take(&mut *bytes)) {
        Ok(text) => Zeroizing::new(text),
        Err(error) => {
            let bytes = Zeroizing::new(error.into_bytes());
            Zeroizing::new(String::from_utf8_lossy(&bytes).into_owned())
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
    fn valid_text_keeps_the_decryption_allocation() {
        let bytes = Zeroizing::new(b"authenticated text".to_vec());
        let allocation = bytes.as_ptr();
        let text = text_from_bytes(bytes);
        assert_eq!(text.as_ptr(), allocation);
        assert_eq!(&*text, "authenticated text");
    }

    #[test]
    fn invalid_utf8_keeps_the_existing_lossy_text_behavior() {
        let text = text_from_bytes(Zeroizing::new(vec![b'a', 0xff, b'b']));
        assert_eq!(&*text, "a\u{fffd}b");
    }

    #[test]
    fn a_preview_does_not_retain_a_full_size_display_copy() {
        let body = "a".repeat(1024 * 1024);
        let (row, key) = stored(copypaste_ipc::content_type::TEXT, body.as_bytes(), None);
        let payload = ClipboardPayload::open(&row, &key).unwrap();
        let (preview, truncated) = payload.display_preview();
        assert!(truncated);
        assert_eq!(preview.len(), copypaste_ipc::limits::LIST_PREVIEW_BYTES);
        assert_eq!(preview.capacity(), preview.len());
        assert_eq!(payload.plain_text(), Some(body.as_str()));

        let mut corrupted = row;
        *corrupted.content_ciphertext.last_mut().unwrap() ^= 1;
        assert!(ClipboardPayload::open(&corrupted, &key).is_err());
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
            assert_eq!(payload.display_preview(), (expected.to_string(), false));
            assert_eq!(payload.plain_text(), None, "{content_type}");
        }
    }

    #[test]
    fn a_file_source_reference_is_the_display_value_and_its_bytes_can_be_saved() {
        let metadata = FileMetadata::with_source_reference(
            "note.bin",
            "application/octet-stream",
            "/Users/person/Documents/note.bin",
        )
        .unwrap();
        let (row, key) = stored(
            copypaste_ipc::content_type::FILE,
            b"file bytes",
            Some(metadata),
        );
        let payload = ClipboardPayload::open(&row, &key).unwrap();
        assert_eq!(payload.display_text(), "/Users/person/Documents/note.bin");
        assert_eq!(
            payload.source_reference(),
            Some("/Users/person/Documents/note.bin")
        );
        assert_eq!(payload.plain_text(), None);

        let dir = tempfile::tempdir().unwrap();
        let destination = dir.path().join("saved.bin");
        payload.save_file_to(&destination).unwrap();
        assert_eq!(std::fs::read(&destination).unwrap(), b"file bytes");
        assert_eq!(
            payload.save_file_to(&destination).unwrap_err().kind(),
            std::io::ErrorKind::AlreadyExists
        );
    }
}
