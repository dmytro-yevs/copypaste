//! One authenticated History representation for every platform host.
use crate::{ClipboardPayload, StoredItem};
use copypaste_ipc::{FileDetails, ImageDetails, Item};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemOrigin {
    pub device_id: String,
    pub device_name: Option<String>,
    pub device_class: copypaste_ipc::DeviceClass,
}

/// A source reference is usable only on the device that captured it.
pub fn source_available(reference: &str) -> bool {
    !reference.starts_with("content://") && std::path::Path::new(reference).is_file()
}

pub fn open_item(
    row: StoredItem,
    origin: &ItemOrigin,
    local_device_id: &str,
    key: &crate::ItemKey,
    preview: bool,
) -> Result<(Item, ClipboardPayload), crate::CryptoError> {
    let payload = ClipboardPayload::open(&row, key)?;
    let item = from_payload(&row, &payload, origin, local_device_id, preview);
    Ok((item, payload))
}

/// The payload must have been authenticated against this row's ID.
pub(crate) fn from_payload(
    row: &StoredItem,
    payload: &ClipboardPayload,
    origin: &ItemOrigin,
    local_device_id: &str,
    preview: bool,
) -> Item {
    let privacy = row.clipboard_privacy();
    // Measured on the plaintext bytes, because that is what the cloud path
    // measures: `LocalItem::content` is the opened payload, and the seal that
    // follows is a fixed overhead the cap does not count.
    let too_large_to_sync = crate::file_transfer::offer(payload).is_none()
        && payload.byte_len() > copypaste_ipc::MAX_CONTENT_BYTES;
    let (content, truncated) = if preview && privacy.secret {
        (String::new(), false)
    } else if preview {
        payload.display_preview()
    } else {
        (payload.display_text(), false)
    };
    let imported_image_metadata = row
        .payload_metadata
        .as_deref()
        .and_then(|value| crate::PayloadMetadata::from_json(value, &row.content_type))
        .and_then(|value| value.file);
    let file_details = match payload {
        ClipboardPayload::File { .. } | ClipboardPayload::Image { .. }
            if imported_image_metadata.is_some()
                || matches!(payload, ClipboardPayload::File { .. }) =>
        {
            let metadata = match payload {
                ClipboardPayload::File { metadata, .. } => metadata,
                _ => &imported_image_metadata,
            };
            let source_reference = metadata
                .as_ref()
                .and_then(|metadata| metadata.source_reference.clone());
            Some(FileDetails {
                filename: metadata.as_ref().map(|metadata| metadata.filename.clone()),
                mime_type: metadata.as_ref().map(|metadata| metadata.mime_type.clone()),
                source_available: origin.device_id == local_device_id
                    && source_reference.as_deref().is_some_and(source_available),
                source_reference,
                size_bytes: crate::file_transfer::offer(payload)
                    .map_or(payload.byte_len() as u64, |offer| offer.size_bytes),
                file_count: 1,
            })
        }
        ClipboardPayload::File { .. }
        | ClipboardPayload::Text(_)
        | ClipboardPayload::Image { .. }
        | ClipboardPayload::Unsupported { .. } => None,
    };
    let image_details = if preview {
        None
    } else {
        match payload {
            ClipboardPayload::Image { bytes, .. } => {
                crate::image_metadata(bytes)
                    .ok()
                    .map(|metadata| ImageDetails {
                        width: metadata.width,
                        height: metadata.height,
                        size_bytes: metadata.size_bytes,
                    })
            }
            ClipboardPayload::Text(_)
            | ClipboardPayload::File { .. }
            | ClipboardPayload::Unsupported { .. } => None,
        }
    };
    let semantic = payload
        .plain_text()
        .and_then(|text| crate::classify_semantic(&row.content_type, text));
    let content_class = copypaste_ipc::content_type::classify(&row.content_type);
    Item {
        privacy,
        id: row.id.clone(),
        content,
        content_type: row.content_type.clone(),
        content_class,
        semantic_kind: semantic
            .filter(|_| !privacy.secret || !preview)
            .map(|classification| classification.kind),
        color_rgba: semantic
            .filter(|_| !privacy.secret || !preview)
            .and_then(|classification| classification.color_rgba),
        created_at: row.created_at,
        pinned: row.pinned,
        file_details: if preview && privacy.secret {
            None
        } else {
            file_details
        },
        image_details,
        origin_device_id: origin.device_id.clone(),
        origin_device_name: origin.device_name.clone(),
        origin_device_class: origin.device_class,
        source_app_bundle_id: row.app_bundle_id.clone(),
        source_app_name: row.app_name.clone(),
        source_app_icon_id: row.source_icon_id.clone(),
        too_large_to_sync,
        truncated,
    }
}

/// The payload-derived fields of an authenticated list preview. All mutable
/// row and device metadata is applied when the preview is presented.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StaticHistoryPreview {
    content: String,
    semantic_kind: Option<copypaste_ipc::SemanticKind>,
    color_rgba: Option<u32>,
    file_details: Option<StaticFileDetails>,
    too_large_to_sync: bool,
    truncated: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct StaticFileDetails {
    filename: Option<String>,
    mime_type: Option<String>,
    source_reference: Option<String>,
    size_bytes: u64,
    file_count: u32,
}

impl StaticHistoryPreview {
    pub(crate) fn from_open_payload(row: &StoredItem, payload: &ClipboardPayload) -> Self {
        let origin = ItemOrigin {
            device_id: String::new(),
            device_name: None,
            device_class: copypaste_ipc::DeviceClass::Unknown,
        };
        let item = from_payload(row, payload, &origin, "local", true);
        Self {
            content: item.content,
            semantic_kind: item.semantic_kind,
            color_rgba: item.color_rgba,
            file_details: item.file_details.map(|details| StaticFileDetails {
                filename: details.filename,
                mime_type: details.mime_type,
                source_reference: details.source_reference,
                size_bytes: details.size_bytes,
                file_count: details.file_count,
            }),
            too_large_to_sync: item.too_large_to_sync,
            truncated: item.truncated,
        }
    }

    /// Apply fresh row and device state to an authenticated preview.
    pub fn into_item(self, row: &StoredItem, origin: &ItemOrigin, local_device_id: &str) -> Item {
        let file_details = self.file_details.map(|details| FileDetails {
            source_available: origin.device_id == local_device_id
                && details
                    .source_reference
                    .as_deref()
                    .is_some_and(source_available),
            filename: details.filename,
            mime_type: details.mime_type,
            source_reference: details.source_reference,
            size_bytes: details.size_bytes,
            file_count: details.file_count,
        });
        Item {
            privacy: row.clipboard_privacy(),
            id: row.id.clone(),
            content: self.content,
            content_type: row.content_type.clone(),
            content_class: copypaste_ipc::content_type::classify(&row.content_type),
            semantic_kind: self.semantic_kind,
            color_rgba: self.color_rgba,
            created_at: row.created_at,
            pinned: row.pinned,
            file_details,
            image_details: None,
            origin_device_id: origin.device_id.clone(),
            origin_device_name: origin.device_name.clone(),
            origin_device_class: origin.device_class,
            source_app_bundle_id: row.app_bundle_id.clone(),
            source_app_name: row.app_name.clone(),
            source_app_icon_id: row.source_icon_id.clone(),
            too_large_to_sync: self.too_large_to_sync,
            truncated: self.truncated,
        }
    }
}
