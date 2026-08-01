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
//! The obvious alternative is a rule — "remember to blank `content` when
//! `is_sensitive`" — enforced by review at each of the nine commands that
//! return items. That is the shape of defect manifest 06 records: v1 had the
//! rule and shipped a path that forgot it.
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

use copypaste_ipc::{DiscoveredDevice, Item, PairingData, PeerInfo, StatusData, SyncResult};
use serde::Serialize;

/// One history item, as the WebView is allowed to see it.
///
/// Constructed only by `From<Item>`; see the module docs for why that matters.
#[derive(Debug, Clone, Serialize)]
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
    /// Which device captured this. Not secret, and the whole point of it is to
    /// be shown: an item that arrived from the Mac and one captured on this
    /// phone are different things to a user, and the android access doc's §5
    /// rule 5 turns a gap in the history from a mystery into an explanation.
    origin_device_id: String,
    origin_device_name: Option<String>,
    /// Cloud sync will not carry this item. Passed through so the row can say
    /// so before the first attempt rather than after it silently never arrives
    /// (`CopyPaste-f72f`).
    too_large_to_sync: bool,
}

impl From<Item> for UiItem {
    fn from(item: Item) -> Self {
        // The whole point of the file, in one branch. `item.content` is moved
        // into the `Some` or dropped on the spot; there is no path that keeps
        // it and no way to opt out.
        let content = if item.is_sensitive {
            None
        } else {
            Some(item.content)
        };
        Self {
            id: item.id,
            content,
            content_type: item.content_type,
            created_at: item.created_at,
            pinned: item.pinned,
            is_sensitive: item.is_sensitive,
            origin_device_id: item.origin_device_id,
            origin_device_name: item.origin_device_name,
            too_large_to_sync: item.too_large_to_sync,
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

/// One peer's sync outcome, verbatim from the wire type.
pub type UiSyncResult = SyncResult;

/// A device seen on the LAN, verbatim from the wire type.
///
/// Passed through rather than wrapped because none of it is secret and none of
/// it is trusted: it is unauthenticated mDNS chatter either way, and a wrapper
/// would only invite someone to sanitise it into looking confirmed.
pub type UiDiscovered = DiscoveredDevice;

/// A freshly minted pairing.
///
/// Unlike [`UiItem`] this **does** carry a secret — `code` is the pre-shared
/// key in transferable form — and it crosses the bridge on purpose, because
/// showing the code to the user is the entire feature. It is passed straight
/// through rather than wrapped so that no one is tempted to add a
/// "get the code again" command: the backend mints it once, this is the single
/// delivery, and the frontend must not persist it.
pub type UiPairing = PairingData;

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
            origin_device_id: "device-1".into(),
            origin_device_name: Some("Mac".into()),
            too_large_to_sync: false,
        }
    }

    #[test]
    fn ordinary_content_crosses_the_boundary_intact() {
        let item = UiItem::from(wire("hello", false));
        assert!(item.has_content());
        let json = serde_json::to_string(&item).unwrap();
        assert!(json.contains("hello"), "{json}");
    }

    /// The load-bearing test: a sensitive item's plaintext must not appear in
    /// the JSON that reaches the WebView, in any form — not the value, not a
    /// mask of it, not a truncated preview.
    #[test]
    fn sensitive_plaintext_never_reaches_the_serialised_form() {
        let secret = "AKIAIOSFODNN7EXAMPLE";
        let item = UiItem::from(wire(secret, true));

        assert!(!item.has_content());
        let json = serde_json::to_string(&item).unwrap();
        assert!(!json.contains(secret), "sensitive plaintext leaked: {json}");
        assert!(!json.contains("AKIA"), "a prefix leaked: {json}");
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
}
