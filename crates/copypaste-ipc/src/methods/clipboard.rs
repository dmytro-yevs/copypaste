//! Clipboard-item METHOD_* constants and the [`StatsResponse`] DTO.

use serde::{Deserialize, Serialize};

// ── Core clipboard methods ──────────────────────────────────────────────────

/// Fetch a paginated list of clipboard items.
pub const METHOD_LIST: &str = "list";

/// Fetch one page of clipboard history items (with pagination).
///
/// Params: `{ limit: u32, offset: u32 }`.  Response: `{ items: […], total: u32,
/// own_device_id: String }`.  The UI uses this (not the legacy `list`) for the
/// paginated history view so it can load incrementally.
pub const METHOD_HISTORY_PAGE: &str = "history_page";

/// Full-text search over clipboard items.
pub const METHOD_SEARCH: &str = "search";

/// Copy a clipboard item back to the system clipboard by id.
///
/// Params: `{ id: String }`.  Different from the legacy `copy` method in that
/// it uses the item's stable UUID rather than an integer index, and returns a
/// richer response (including the decrypted text for paste-back).
pub const METHOD_COPY_ITEM: &str = "copy_item";

/// Copy a clipboard item back to the system clipboard by id.
pub const METHOD_COPY: &str = "copy";

/// Delete a single clipboard item by id.
///
/// Params: `{ id: String }`.  Deletes the item with the given UUID from the
/// encrypted database and removes it from the FTS index.
pub const METHOD_DELETE_ITEM: &str = "delete_item";

/// Delete a single clipboard item by id.
pub const METHOD_DELETE: &str = "delete";

/// Delete all clipboard items (clear history).
pub const METHOD_DELETE_ALL: &str = "delete_all";

/// Return the total count of stored clipboard items.
pub const METHOD_COUNT: &str = "count";

/// Return aggregate statistics about the clipboard database.
///
/// This is the CLI diagnostic view. The UI uses [`METHOD_DB_STATS`] instead,
/// which returns only `{item_count, size_bytes}` for the settings panel.
/// These two methods are intentionally distinct: `stats` is richer and intended
/// for human-readable terminal output; `db_stats` is minimal and typed for the
/// UI's storage summary widget (c4q2.23).
///
/// Params: none (empty `{}`).
/// Response: [`StatsResponse`].
///
/// [`METHOD_DB_STATS`]: crate::methods::METHOD_DB_STATS
pub const METHOD_STATS: &str = "stats";

/// Success payload for [`METHOD_STATS`].
///
/// All counts are live values from the encrypted database at the time of the
/// call. The daemon serialises this struct directly, so field names are the
/// stable wire contract between the daemon and the CLI (c4q2.23 — formerly an
/// ad-hoc `serde_json::json!({...})` with no typed schema).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct StatsResponse {
    /// Total number of clipboard items (all rows, including sensitive ones).
    pub total_items: i64,
    /// Number of items flagged as sensitive (password / secret patterns).
    pub sensitive_items: i64,
    /// IPC/schema version string (not a semver release; currently `"1"`).
    pub version: String,
    /// Daemon build version string (`env!("CARGO_PKG_VERSION")`).
    pub build_version: String,
}

// ── Daemon health ───────────────────────────────────────────────────────────

/// Query the running daemon's health / readiness state.
pub const METHOD_STATUS: &str = "status";

// ── Import / export ─────────────────────────────────────────────────────────

/// Export clipboard items as a JSON blob.
pub const METHOD_EXPORT: &str = "export";

/// Bulk-import clipboard items from a JSON blob.
pub const METHOD_IMPORT: &str = "import";

// ── Pinning ─────────────────────────────────────────────────────────────────

/// Pin or unpin a clipboard item (takes `{id, pinned: bool}`).
pub const METHOD_PIN_ITEM: &str = "pin_item";

// ── Private / pause mode ────────────────────────────────────────────────────

/// Enable or disable clipboard recording pause mode.
pub const METHOD_SET_PRIVATE_MODE: &str = "set_private_mode";

/// Query the current private-mode state.
pub const METHOD_GET_PRIVATE_MODE: &str = "get_private_mode";

// ── Item media access ───────────────────────────────────────────────────────

/// Fetch the full image bytes for a `content_type == "image"` clipboard item.
///
/// Params: `{ id: String }`.  Returns `{ data_uri: String }` (a `data:image/…`
/// URL with base64-encoded bytes).  Use [`METHOD_GET_ITEM_THUMBNAIL`] for the
/// pre-computed low-resolution preview.
pub const METHOD_GET_ITEM_IMAGE: &str = "get_item_image";

/// Fetch the full binary payload for a `content_type == "file"` clipboard item.
///
/// Params: `{ id: String }`.  Returns `{ filename: String, mime: String,
/// data_b64: String }` where `data_b64` is standard base64.  The daemon reads
/// the encrypted blob, decrypts it, and returns the raw bytes.
pub const METHOD_GET_ITEM_FILE: &str = "get_item_file";

/// Fetch the pre-computed thumbnail for a clipboard image item.
///
/// Params: `{ id: String }`.  Returns `{ thumbnail: String | null }` where
/// `thumbnail` is a `data:image/webp;base64,…` URL.  `null` when thumbnails are
/// unavailable for this item (older daemon, non-image item, or generation
/// failed at capture time).  Callers fall back to [`METHOD_GET_ITEM_IMAGE`].
pub const METHOD_GET_ITEM_THUMBNAIL: &str = "get_item_thumbnail";

/// Resolve a macOS app bundle identifier to a 32×32 PNG icon (base64).
///
/// Params: `{ bundle_id: String }`.  Returns `{ png_b64: String | null }`.
/// `null` when the app is not installed or the daemon cannot extract the icon.
/// Results are cached in the daemon so repeated calls are fast.
pub const METHOD_GET_APP_ICON: &str = "get_app_icon";

// ── Own device identity ──────────────────────────────────────────────────────

/// Return this device's mTLS certificate fingerprint (hex SHA-256).
///
/// Returns `{ fingerprint: String }`.  Null when P2P is disabled (no cert).
/// Superseded by [`METHOD_GET_OWN_DEVICE_INFO`] which returns the full
/// identity; retained for back-compat with older callers.
pub const METHOD_GET_OWN_FINGERPRINT: &str = "get_own_fingerprint";

/// Return rich identity for THIS device: name, model, OS, version, IPs,
/// and mTLS fingerprint.
///
/// Returns `{ fingerprint, device_name, device_model, os_version,
/// app_version, local_ip, public_ip }`.  All fields except `app_version`
/// are optional — gracefully handle absent ones.
pub const METHOD_GET_OWN_DEVICE_INFO: &str = "get_own_device_info";

// ── File ingest ─────────────────────────────────────────────────────────────

/// Ingest a file directly into the clipboard history from the desktop UI.
///
/// Params: `{ filename: String, mime: String, data_b64: String }` where
/// `data_b64` is standard base64-encoded raw file bytes. The daemon encrypts,
/// stores, and deduplicates it the same way a pasteboard-captured file is
/// stored via `handle_file`.
///
/// Response: `{ id: String }` — the stable clipboard item UUID.
pub const METHOD_ADD_FILE_ITEM: &str = "add_file_item";

// ── Content-type → macOS UTI mapping ─────────────────────────────────────────

/// Map the IPC wire `content_type` string to a macOS UTI suitable for
/// `setData:forType:` on `NSPasteboard`.
///
/// This is the single source of truth for the `content_type` → UTI contract.
/// It lives in `copypaste-ipc` (not the daemon) so the mapping is shared and
/// tested once: the daemon uses it on the paste-back path, and any future
/// client/IPC method that needs to surface a UTI derives it from the same
/// function rather than re-inventing the table (CopyPaste-c4q2.10).
///
/// The function is pure string logic with **no** OS dependency, so it is
/// available on every platform (not `cfg(macos)`-gated) and unit-testable
/// anywhere.
///
/// Heuristic (audit HIGH #2: a bare `"image"` is not a UTI and macOS refuses to
/// set pasteboard data for it):
/// - Anything already shaped like a UTI (`public.*`, `com.*`, `org.*`) passes
///   through unchanged.
/// - The bare wire values map: `"image"` → `public.png`,
///   `"text"` → `public.utf8-plain-text`.
/// - Everything else falls back to `public.data` so the write never silently
///   no-ops.
pub fn map_content_type_to_uti(content_type: &str) -> String {
    if content_type.starts_with("public.")
        || content_type.starts_with("com.")
        || content_type.starts_with("org.")
    {
        return content_type.to_string();
    }
    match content_type {
        "image" => "public.png".to_string(),
        "text" => "public.utf8-plain-text".to_string(),
        _ => "public.data".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // PG-62: verify all previously-missing METHOD_* constants have the correct
    // wire names (matching the string literals used in the UI's ipc.ts api object).
    #[test]
    fn pg62_history_page_method_has_correct_wire_name() {
        assert_eq!(METHOD_HISTORY_PAGE, "history_page");
    }

    #[test]
    fn pg62_copy_item_method_has_correct_wire_name() {
        assert_eq!(METHOD_COPY_ITEM, "copy_item");
    }

    #[test]
    fn pg62_delete_item_method_has_correct_wire_name() {
        assert_eq!(METHOD_DELETE_ITEM, "delete_item");
    }

    #[test]
    fn pg62_item_media_methods_have_correct_wire_names() {
        assert_eq!(METHOD_GET_ITEM_IMAGE, "get_item_image");
        assert_eq!(METHOD_GET_ITEM_FILE, "get_item_file");
        assert_eq!(METHOD_GET_ITEM_THUMBNAIL, "get_item_thumbnail");
        assert_eq!(METHOD_GET_APP_ICON, "get_app_icon");
    }

    #[test]
    fn pg62_own_device_methods_have_correct_wire_names() {
        assert_eq!(METHOD_GET_OWN_FINGERPRINT, "get_own_fingerprint");
        assert_eq!(METHOD_GET_OWN_DEVICE_INFO, "get_own_device_info");
    }

    // ── c4q2.23: StatsResponse ───────────────────────────────────────────────

    /// c4q2.23: StatsResponse must survive a JSON round-trip with all fields
    /// intact and must NOT include any field beyond the four declared ones.
    #[test]
    fn stats_response_roundtrip() {
        let resp = StatsResponse {
            total_items: 42,
            sensitive_items: 3,
            version: "1".to_string(),
            build_version: "0.6.0".to_string(),
        };
        let s = serde_json::to_string(&resp).unwrap();
        let back: StatsResponse = serde_json::from_str(&s).unwrap();
        assert_eq!(resp, back);
        assert!(s.contains("\"total_items\":42"), "wire: {s}");
        assert!(s.contains("\"sensitive_items\":3"), "wire: {s}");
        assert!(s.contains("\"version\":\"1\""), "wire: {s}");
        assert!(s.contains("\"build_version\":\"0.6.0\""), "wire: {s}");
    }

    /// Default StatsResponse has all-zero/empty fields.
    #[test]
    fn stats_response_default_is_zero() {
        let resp = StatsResponse::default();
        assert_eq!(resp.total_items, 0);
        assert_eq!(resp.sensitive_items, 0);
        assert_eq!(resp.version, "");
        assert_eq!(resp.build_version, "");
    }

    /// METHOD_STATS has the correct wire name.
    #[test]
    fn stats_method_has_correct_wire_name() {
        assert_eq!(METHOD_STATS, "stats");
    }

    // ── map_content_type_to_uti (CopyPaste-c4q2.10) ─────────────────────────

    #[test]
    fn uti_maps_bare_wire_values() {
        assert_eq!(map_content_type_to_uti("image"), "public.png");
        assert_eq!(map_content_type_to_uti("text"), "public.utf8-plain-text");
    }

    #[test]
    fn uti_passes_through_existing_utis() {
        // Already-UTI shapes must pass through unchanged so a precise UTI from
        // the capture side is never downgraded.
        for uti in [
            "public.utf8-plain-text",
            "public.png",
            "public.jpeg",
            "com.adobe.pdf",
            "org.gnu.gnu-zip-archive",
        ] {
            assert_eq!(map_content_type_to_uti(uti), uti);
        }
    }

    #[test]
    fn uti_unknown_falls_back_to_public_data() {
        // Anything unrecognised maps to public.data so the pasteboard write
        // never silently no-ops.
        assert_eq!(map_content_type_to_uti("file"), "public.data");
        assert_eq!(map_content_type_to_uti(""), "public.data");
        assert_eq!(map_content_type_to_uti("video"), "public.data");
    }
}
