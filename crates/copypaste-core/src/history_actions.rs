//! Authenticated copy and save policy shared by all platform hosts.
use crate::history_error::HistoryError;
use crate::history_item::{self, ItemOrigin};
use crate::{ClipboardPayload, ItemKey, Store, StoredItem};
use std::path::Path;

pub struct OpenedItem {
    row: StoredItem,
    payload: ClipboardPayload,
}

impl OpenedItem {
    pub fn row(&self) -> &StoredItem {
        &self.row
    }
    pub fn into_item(self, origin: &ItemOrigin, local_device_id: &str) -> copypaste_ipc::Item {
        history_item::from_payload(&self.row, &self.payload, origin, local_device_id, false)
    }
}

pub fn open(store: &Store, key: &ItemKey, id: &str) -> Result<OpenedItem, HistoryError> {
    let row = store
        .get(id)
        .map_err(HistoryError::Store)?
        .ok_or(HistoryError::NotFound)?;
    let payload = ClipboardPayload::open(&row, key).map_err(HistoryError::Decrypt)?;
    if oversized_text(&payload) {
        return Err(HistoryError::ContentTooLarge);
    }
    Ok(OpenedItem { row, payload })
}

pub fn oversized_text(payload: &ClipboardPayload) -> bool {
    matches!(payload, ClipboardPayload::Text(_))
        && payload.byte_len() > copypaste_ipc::MAX_CONTENT_BYTES
}

pub struct CopyOptions<'a> {
    pub origin: &'a ItemOrigin,
    pub local_device_id: &'a str,
    pub plain_text: bool,
    pub allow_content_uri: bool,
    pub max_file_size_bytes: u64,
}

/// Owns staged plaintext until the native clipboard writer has consumed it.
pub struct PreparedCopy {
    pub item: copypaste_ipc::Item,
    pub content_type: String,
    payload: ClipboardPayload,
    staged: Option<crate::file_transfer::PreparedFileCopy>,
}
impl PreparedCopy {
    pub fn payload(&self) -> &ClipboardPayload {
        self.staged
            .as_ref()
            .map_or(&self.payload, |file| &file.payload)
    }
}

pub fn prepare_copy(
    store: &Store,
    key: &ItemKey,
    opened: OpenedItem,
    options: CopyOptions<'_>,
) -> Result<PreparedCopy, HistoryError> {
    let OpenedItem { row, mut payload } = opened;
    if options.plain_text && !matches!(payload, ClipboardPayload::Text(_)) {
        return Err(HistoryError::Unsupported);
    }
    let item = history_item::from_payload(
        &row,
        &payload,
        options.origin,
        options.local_device_id,
        false,
    );
    let staged = if options.plain_text
        || crate::file_transfer::offer(&payload).is_none()
        || store
            .file_reference(&row.id)
            .map_err(crate::file_transfer::FileTransferError::from)?
            .is_some()
    {
        None
    } else {
        crate::file_transfer::prepare_copy(store, key, &row, options.max_file_size_bytes)?
    };
    if let ClipboardPayload::File {
        metadata: Some(metadata),
        ..
    } = &mut payload
    {
        let available = crate::origin_or(&row.origin_device_id, options.local_device_id)
            == options.local_device_id
            && metadata.source_reference.as_deref().is_some_and(|source| {
                (options.allow_content_uri && source.starts_with("content://"))
                    || history_item::source_available(source)
            });
        if !available {
            metadata.source_reference = None;
        }
    }
    let content_type = if options.plain_text {
        copypaste_ipc::content_type::TEXT.to_owned()
    } else {
        row.content_type
    };
    Ok(PreparedCopy {
        item,
        content_type,
        payload,
        staged,
    })
}

pub fn save_file(
    store: &Store,
    key: &ItemKey,
    id: &str,
    raw_destination: &str,
    max_file_size_bytes: u64,
) -> Result<(), HistoryError> {
    let destination = raw_destination.trim();
    if destination.is_empty() {
        return Err(HistoryError::BadPath);
    }
    let path = Path::new(destination);
    if path.exists() {
        return Err(HistoryError::Exists);
    }
    if path.parent().is_none_or(|parent| !parent.is_dir()) {
        return Err(HistoryError::MissingDirectory);
    }
    let opened = open(store, key, id)?;
    if crate::file_transfer::offer(&opened.payload).is_some() {
        return crate::file_transfer::save_local(store, key, id, path, max_file_size_bytes, || {
            false
        })
        .map_err(HistoryError::File);
    }
    if !matches!(
        opened.payload,
        ClipboardPayload::Image { .. } | ClipboardPayload::File { .. }
    ) {
        return Err(HistoryError::Unsupported);
    }
    opened
        .payload
        .save_file_to(path)
        .map_err(|error| match error.kind() {
            std::io::ErrorKind::AlreadyExists => HistoryError::Exists,
            std::io::ErrorKind::NotFound => HistoryError::MissingDirectory,
            _ => HistoryError::Save,
        })
}
