//! The types the WebView is allowed to see — and the boundary that decides it.
//!
//! # Why this file exists at all
//!
//! [`copypaste_ipc::Item`] carries plaintext `content`. That is correct on the
//! wire: the daemon decrypts on the way out and the socket is `0600`. It is not
//! correct in a WebView. Once a string is in the JS heap it is reachable by
//! every component, by the accessibility tree, by devtools, by a heap snapshot,
//! and by anything that later serialises a props object into a log. Manifest 06
//! INV-10 requires that sensitive content be **absent** from the view rather
//! than obscured on top of it, and "obscured on top of it" is exactly what you
//! get if the plaintext crosses the bridge and a component is trusted to hide
//! it.
//!
//! So the plaintext is discarded **here**, at the process boundary, before
//! serialisation. A sensitive item reaches React as an item with no content at
//! all.
//!
//! # Why it is structural rather than a rule
//!
//! The obvious alternative is a per-command "remember to blank `content`"
//! rule. Manifest 06 INV-10 requires a structural boundary instead.
//!
//! Instead:
//!
//! * [`UiItem`] has no public constructor and no public struct literal — its
//!   fields are private, so the only way to make one is [`UiItem::from`], and
//!   that function is total.
//! * Every command signature returns `UiItem` / `Vec<UiItem>`, never
//!   [`copypaste_ipc::Item`]. A new command physically cannot return the wire
//!   type without changing its own signature to say so.
//! * `content` is `Option<String>` rather than a `String` that happens to be
//!   empty, so "there is no content" is a state the frontend's type checker
//!   sees, not a value it has to test for.
//!
//! The one deliberate way back to plaintext is
//! [`crate::commands::history::reveal_item`], which takes an id, returns one
//! item's text, and exists because the user asked for it by pressing a button.
//! Everything else — copy, delete, pin — travels by id and does its work in the
//! backend, so the secret never needs to be in the WebView to be *used*.

#[cfg(any(
    target_os = "android",
    target_os = "macos",
    target_os = "windows",
    test
))]
use base64::{engine::general_purpose::STANDARD, Engine as _};
use copypaste_ipc::{
    DiscoveredDevice, ImagePreview, Item, PeerInfo, SensitiveFinding, StatusData, SyncResult,
};
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
    /// `None` for a sensitive item — not an empty string, and not a mask. The
    /// plaintext was dropped before this value existed.
    content: Option<String>,
    content_type: String,
    /// Milliseconds since the Unix epoch.
    created_at: i64,
    pinned: bool,
    is_sensitive: bool,
    /// Inert detector metadata. A high-confidence item never carries even a
    /// redacted preview across this boundary.
    sensitive_finding: Option<SensitiveFinding>,
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

/// A thumbnail produced on demand from a non-sensitive image item.
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
    #[cfg(any(target_os = "macos", target_os = "windows", test))]
    pub(crate) fn from_png(png: Vec<u8>, width: u32, height: u32) -> Self {
        #[cfg(not(test))]
        {
            const MAX_EDGE: u32 = 512;
            const MAX_BYTES: usize = 512 * 1024;
            debug_assert!(
                width > 0
                    && height > 0
                    && width <= MAX_EDGE
                    && height <= MAX_EDGE
                    && png.len() <= MAX_BYTES
                    && png.starts_with(b"\x89PNG"),
                "icon dimensions or size out of bounds"
            );
        }
        Self {
            png_base64: STANDARD.encode(png),
            width,
            height,
        }
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
        // The whole point of the file, in one branch. `item.content` is moved
        // into the `Some` or dropped on the spot; there is no path that keeps
        // it and no way to opt out.
        let (content, sensitive_finding) = if item.is_sensitive {
            (None, None)
        } else {
            (Some(item.content), item.sensitive_finding)
        };
        Self {
            id: item.id,
            content,
            content_type: item.content_type,
            created_at: item.created_at,
            pinned: item.pinned,
            is_sensitive: item.is_sensitive,
            sensitive_finding,
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

    /// Whether the detector flagged this item.
    pub fn is_sensitive(&self) -> bool {
        self.is_sensitive
    }

    /// Whether any content survived the boundary. Always false for a sensitive
    /// item.
    pub fn has_content(&self) -> bool {
        self.content.is_some()
    }
}

/// Convert a page of wire items, dropping sensitive plaintext as it goes.
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

    fn wire(content: &str, is_sensitive: bool) -> Item {
        Item {
            id: "item-1".into(),
            content: content.into(),
            content_type: "text/plain".into(),
            created_at: 1_700_000_000_000,
            pinned: false,
            is_sensitive,
            sensitive_finding: None,
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
        let item = UiItem::from(wire("hello", false));
        assert!(item.has_content());
        let json = serde_json::to_string(&item).unwrap();
        assert!(json.contains("hello"), "{json}");
    }

    #[test]
    fn inert_finding_metadata_crosses_with_a_redacted_preview() {
        let mut wire = wire("mail alice@example.com", false);
        wire.sensitive_finding = Some(SensitiveFinding {
            label: "email".into(),
            spans: vec![copypaste_ipc::SensitiveSpan { start: 5, end: 22 }],
            spans_truncated: false,
            redacted_preview: "mail ***REDACTED***".into(),
        });

        let json = serde_json::to_string(&UiItem::from(wire)).unwrap();
        assert!(json.contains(r#""label":"email""#), "{json}");
        assert!(json.contains("mail ***REDACTED***"), "{json}");
    }

    /// The load-bearing test: a sensitive item's plaintext must not appear in
    /// the JSON that reaches the WebView, in any form — not the value, not a
    /// mask of it, not a truncated preview.
    #[test]
    fn sensitive_plaintext_never_reaches_the_serialised_form() {
        let secret = "AKIAIOSFODNN7EXAMPLE";
        let mut wire = wire(secret, true);
        wire.sensitive_finding = Some(SensitiveFinding {
            label: "untrusted".into(),
            spans: Vec::new(),
            spans_truncated: false,
            redacted_preview: "surrounding plaintext".into(),
        });
        let item = UiItem::from(wire);

        assert!(!item.has_content());
        let json = serde_json::to_string(&item).unwrap();
        assert!(!json.contains(secret), "sensitive plaintext leaked: {json}");
        assert!(!json.contains("AKIA"), "a prefix leaked: {json}");
        assert!(!json.contains("surrounding plaintext"), "{json}");
        assert!(!json.contains("untrusted"), "{json}");
        assert!(json.contains("\"content\":null"), "{json}");
        // The flag itself must survive: the view needs to know to render a
        // placeholder rather than an empty row.
        assert!(json.contains("\"is_sensitive\":true"), "{json}");
    }

    /// Sensitive content must not survive by being long enough to be
    /// truncated somewhere else instead.
    #[test]
    fn a_long_sensitive_payload_is_dropped_whole() {
        let secret = "x".repeat(100_000);
        let item = UiItem::from(wire(&secret, true));
        let json = serde_json::to_string(&item).unwrap();
        assert!(!json.contains("xxxx"), "part of a long secret survived");
    }

    #[test]
    fn a_page_is_filtered_item_by_item() {
        let items = ui_items(vec![wire("public", false), wire("secret", true)]);
        let json = serde_json::to_string(&items).unwrap();
        assert!(json.contains("public"));
        assert!(!json.contains("secret"), "{json}");
    }

    #[test]
    fn a_page_keeps_its_total_when_the_visible_rows_are_capped() {
        let page = UiPage::with_total(
            crate::backend::Page {
                items: vec![wire("public", false)],
                ..Default::default()
            },
            214,
        );
        let json = serde_json::to_string(&page).unwrap();
        assert!(json.contains("\"total\":214"), "{json}");
    }

    /// The id must survive, because every other operation on a sensitive item
    /// (copy, pin, delete) travels by id instead of by content.
    #[test]
    fn the_id_survives_so_the_item_is_still_operable() {
        let item = UiItem::from(wire("secret", true));
        assert_eq!(item.id(), "item-1");
        assert!(item.is_sensitive());
    }

    #[test]
    fn a_sync_failure_crosses_only_as_code_and_retry_policy() {
        let result = UiSyncResult::from(SyncResult {
            pairing_id: "peer-1".into(),
            name: "Phone".into(),
            sent: 0,
            received: 0,
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
            duration_ms: None,
            error: Some("some future failure".into()),
            error_code: None,
        });
        let json = serde_json::to_string(&result).unwrap();
        assert!(
            json.contains(r#""error":{"code":"unknown","retryable":true}"#),
            "{json}"
        );
        assert!(!json.contains("future failure"), "{json}");
    }
}
