//! UI-facing history DTOs.

#[cfg(target_os = "android")]
use base64::{engine::general_purpose::STANDARD, Engine as _};
use copypaste_ipc::{
    ContentClass, DiscoveredDevice, ImagePreview, Item, PeerInfo, StatusData, SyncResult,
};
use copypaste_source_app::AppIcon;
use serde::Serialize;

use crate::backend::UiError;

/// One history item, as the WebView is allowed to see it.
///
/// Constructed only by `From<Item>`; see the module docs for why that matters.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(rename = "Item", export_to = "ipc.ts"))]
pub struct UiItem {
    id: String,
    content: String,
    content_type: String,
    content_class: ContentClass,
    /// Milliseconds since the Unix epoch.
    created_at: i64,
    pinned: bool,
    /// Which device captured this. Not secret, and the whole point of it is to
    /// be shown: an item that arrived from the Mac and one captured on this
    /// phone are different things to a user, and the android access doc's §5
    /// rule 5 turns a gap in the history from a mystery into an explanation.
    origin_device_id: String,
    origin_device_name: Option<String>,
    /// Cosmetic local source attribution. The webview receives only a bundle
    /// identifier, never an arbitrary filesystem icon path.
    source_app_bundle_id: Option<String>,
    /// Display label resolved by the platform for the captured source app.
    /// It is cosmetic; identities and exclusion rules always use the id.
    source_app_name: Option<String>,
    /// Cloud sync will not carry this item. Passed through so the row can say
    /// so before the first attempt rather than after it silently never arrives
    /// (`CopyPaste-f72f`).
    too_large_to_sync: bool,
    truncated: bool,
}

/// A thumbnail produced on demand from an image item.
///
/// The field is base64 rather than a path or an original payload; React turns
/// it into a short-lived Blob URL and revokes it when the virtual row leaves.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "typescript",
    ts(rename = "ImagePreview", export_to = "ipc.ts")
)]
pub struct UiImagePreview {
    png_base64: String,
    width: u32,
    height: u32,
}

/// A bounded PNG icon for the application that created a captured clip.
///
/// The native resolver accepts only a captured bundle/package identifier and
/// returns bytes, never an application path. The WebView receives a transient
/// Blob URL just as it does for history image previews.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "typescript",
    ts(rename = "SourceAppIcon", export_to = "ipc.ts")
)]
pub struct UiSourceAppIcon {
    png_base64: String,
    width: u32,
    height: u32,
}

/// A user-launchable application selectable as a capture exclusion.
///
/// `package_id` is the existing wire name for the platform identity: Android
/// package id, macOS bundle id, or Windows process image name. No local path
/// crosses into the WebView.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "typescript",
    ts(rename = "InstalledSourceApp", export_to = "ipc.ts")
)]
pub struct UiInstalledSourceApp {
    package_id: String,
    label: String,
}

impl UiInstalledSourceApp {
    pub(crate) fn new(package_id: String, label: String) -> Self {
        Self { package_id, label }
    }
}

impl UiSourceAppIcon {
    pub(crate) fn from_app_icon(icon: AppIcon) -> Self {
        Self {
            png_base64: icon.png_base64,
            width: icon.width,
            height: icon.height,
        }
    }

    pub(crate) fn into_app_icon(self) -> Option<AppIcon> {
        AppIcon::from_base64(self.png_base64, self.width, self.height)
    }

    #[cfg(target_os = "android")]
    pub(crate) fn from_base64(png_base64: String, width: u32, height: u32) -> Option<Self> {
        const MAX_ICON_EDGE: u32 = 384;
        const MAX_ICON_BYTES: usize = 512 * 1024;
        let png = STANDARD.decode(&png_base64).ok()?;
        if png.len() > MAX_ICON_BYTES
            || width == 0
            || height == 0
            || width > MAX_ICON_EDGE
            || height > MAX_ICON_EDGE
            || !png.starts_with(b"\x89PNG\r\n\x1a\n")
        {
            return None;
        }
        Some(Self {
            png_base64,
            width,
            height,
        })
    }
}

impl From<ImagePreview> for UiImagePreview {
    fn from(preview: ImagePreview) -> Self {
        Self {
            png_base64: preview.png_base64,
            width: preview.width,
            height: preview.height,
        }
    }
}

impl From<Item> for UiItem {
    fn from(item: Item) -> Self {
        Self {
            id: item.id,
            content: item.content,
            content_class: copypaste_ipc::content_type::classify(&item.content_type),
            content_type: item.content_type,
            created_at: item.created_at,
            pinned: item.pinned,
            origin_device_id: item.origin_device_id,
            origin_device_name: item.origin_device_name,
            source_app_bundle_id: item.source_app_bundle_id,
            source_app_name: item.source_app_name,
            too_large_to_sync: item.too_large_to_sync,
            truncated: item.truncated,
        }
    }
}

impl UiItem {
    /// The item's id. Safe to show and to log — it is a UUID, not content.
    pub fn id(&self) -> &str {
        &self.id
    }
}

/// Convert a page of wire items for the WebView.
pub fn ui_items(items: Vec<Item>) -> Vec<UiItem> {
    items.into_iter().map(UiItem::from).collect()
}

/// A page of history, and the number of rows in it that would not decrypt.
///
/// The count is what the *view* needs: without it a short page and a small
/// history look the same, which is parity finding 17. It is not an error — the
/// rows that did open are still the user's data — so the frontend renders it as
/// a state rather than a failure.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(feature = "typescript", ts(rename = "ItemPage", export_to = "ipc.ts"))]
pub struct UiPage {
    items: Vec<UiItem>,
    /// The full number of live history items, before this page's cap.
    total: u64,
    /// Named exactly as the wire names it (`ItemPage::skipped_undecryptable`),
    /// so the frontend, the bridge and the daemon all say one thing.
    skipped_undecryptable: u32,
    /// Where to resume, or `null` at the end of the list.
    ///
    /// The **only** end-of-list test the frontend may use. `items.length <
    /// limit` is not one: `skipped_undecryptable` rows were read and dropped,
    /// so a short page can still have a list behind it, and stopping there
    /// would hide the rest of the history behind a few unreadable rows.
    next_cursor: Option<String>,
}

impl From<crate::backend::Page> for UiPage {
    fn from(page: crate::backend::Page) -> Self {
        let total = page.items.len() as u64;
        Self::with_total(page, total)
    }
}

impl UiPage {
    pub fn with_total(page: crate::backend::Page, total: u64) -> Self {
        Self {
            items: ui_items(page.items),
            total,
            skipped_undecryptable: page.skipped_undecryptable,
            next_cursor: page.next_cursor,
        }
    }
    pub fn items(&self) -> &[UiItem] {
        &self.items
    }

    pub fn skipped_undecryptable(&self) -> u32 {
        self.skipped_undecryptable
    }

    pub fn next_cursor(&self) -> Option<&str> {
        self.next_cursor.as_deref()
    }
}

/// Daemon/backend state, verbatim from the wire type. Nothing here is secret.
pub type UiStatus = StatusData;

/// A known peer, verbatim from the wire type.
pub type UiPeer = PeerInfo;

/// One peer's sync outcome with only a structured, display-safe error.
#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "typescript", derive(ts_rs::TS))]
#[cfg_attr(
    feature = "typescript",
    ts(rename = "SyncResult", export_to = "ipc.ts")
)]
pub struct UiSyncResult {
    pairing_id: String,
    name: String,
    sent: u32,
    received: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "typescript", ts(optional))]
    skipped_too_large: Option<u32>,
    duration_ms: Option<u64>,
    error: Option<UiError>,
}

impl From<SyncResult> for UiSyncResult {
    fn from(result: SyncResult) -> Self {
        let error = result
            .error
            .as_ref()
            .map(|_| UiError::from_error_code(result.error_code));
        Self {
            pairing_id: result.pairing_id,
            name: result.name,
            sent: result.sent,
            received: result.received,
            skipped_too_large: result.skipped_too_large,
            duration_ms: result.duration_ms,
            error,
        }
    }
}

/// A device seen on the LAN, verbatim from the wire type.
///
/// Passed through rather than wrapped because none of it is secret and none of
/// it is trusted: it is unauthenticated mDNS chatter either way, and a wrapper
/// would only invite someone to sanitise it into looking confirmed.
pub type UiDiscovered = DiscoveredDevice;

#[cfg(test)]
mod tests {
    use super::*;

    fn wire(content: &str) -> Item {
        Item {
            id: "item-1".into(),
            content: content.into(),
            content_type: "text/plain".into(),
            created_at: 1_700_000_000_000,
            pinned: false,
            origin_device_id: "device-1".into(),
            origin_device_name: Some("Mac".into()),
            source_app_bundle_id: Some("com.apple.Safari".into()),
            source_app_name: Some("Safari".into()),
            too_large_to_sync: false,
            truncated: false,
        }
    }

    #[test]
    fn ordinary_content_crosses_the_boundary_intact() {
        let item = UiItem::from(wire("hello"));
        let json = serde_json::to_string(&item).unwrap();
        assert!(json.contains("hello"), "{json}");
        assert!(json.contains(r#""content_class":"text""#), "{json}");
    }

    #[test]
    fn unknown_content_type_crosses_as_other_without_a_ui_guess() {
        let mut item = wire("future bytes");
        item.content_type = "application/x-future".into();
        let json = serde_json::to_string(&UiItem::from(item)).unwrap();
        assert!(json.contains(r#""content_class":"other""#), "{json}");
    }

    #[test]
    fn a_page_keeps_its_total_when_the_visible_rows_are_capped() {
        let page = UiPage::with_total(
            crate::backend::Page {
                items: vec![wire("public")],
                ..Default::default()
            },
            214,
        );
        let json = serde_json::to_string(&page).unwrap();
        assert!(json.contains("\"total\":214"), "{json}");
    }

    #[test]
    fn the_id_survives_so_the_item_is_still_operable() {
        let item = UiItem::from(wire("value"));
        assert_eq!(item.id(), "item-1");
    }

    #[test]
    fn a_sync_failure_crosses_only_as_code_and_retry_policy() {
        let result = UiSyncResult::from(SyncResult {
            pairing_id: "peer-1".into(),
            name: "Phone".into(),
            sent: 0,
            received: 0,
            skipped_too_large: None,
            duration_ms: Some(750),
            error: Some("timed out at /Users/alice/.copypaste.sock".into()),
            error_code: Some(copypaste_ipc::ErrorCode::PeerUnreachable),
        });
        let json = serde_json::to_string(&result).unwrap();
        assert!(
            json.contains(r#""error":{"code":"peer_unreachable","retryable":true}"#),
            "{json}"
        );
        assert!(json.contains(r#""duration_ms":750"#), "{json}");
        assert!(!json.contains("skipped_too_large"), "{json}");
        assert!(!json.contains("alice"), "{json}");
        assert!(!json.contains("sock"), "{json}");
        assert!(!json.contains("timed out"), "{json}");
    }

    #[test]
    fn an_old_daemon_sync_failure_stays_forward_compatible() {
        let result = UiSyncResult::from(SyncResult {
            pairing_id: "peer-1".into(),
            name: "Phone".into(),
            sent: 0,
            received: 0,
            skipped_too_large: None,
            duration_ms: None,
            error: Some("some future failure".into()),
            error_code: None,
        });
        let json = serde_json::to_string(&result).unwrap();
        assert!(
            json.contains(r#""error":{"code":"unknown","retryable":false}"#),
            "{json}"
        );
        assert!(!json.contains("future failure"), "{json}");
    }

    #[test]
    fn sync_size_refusal_count_reaches_the_webview_only_when_known() {
        let result = UiSyncResult::from(SyncResult {
            pairing_id: "peer-1".into(),
            name: "Phone".into(),
            sent: 1,
            received: 2,
            skipped_too_large: Some(3),
            duration_ms: Some(750),
            error: None,
            error_code: None,
        });
        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains(r#""skipped_too_large":3"#), "{json}");
    }
}
