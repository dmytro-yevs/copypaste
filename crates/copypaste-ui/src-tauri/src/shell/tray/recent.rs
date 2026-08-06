//! What the menu-bar item is allowed to say about the clipboard.
//!
//! A flagged item is not listed at all — not masked, not a placeholder, not a
//! disabled "1 sensitive clipping hidden" row. The list can withhold one
//! because there it is behind a deliberate reveal; a menu cannot withhold, is
//! not scrollable past, is often on screen during a screen share, and is drawn
//! where nothing this app owns can redact it. The hidden-count row is the
//! tempting version and still leaks: it tells a shoulder a secret was copied a
//! moment ago, which is most of what an observer wanted.
//!
//! [`Clipping::from_item`] is total and answers `None` for a flagged item, so
//! the rule cannot be forgotten at a call site (manifest 06).

use copypaste_ipc::Item;

/// How many clippings the menu offers.
///
/// A menu offers the ten most recent safe clippings. The popup is still the
/// place for search and the full history; ten keeps the tray useful without
/// making it a second history view.
pub const SLOTS: usize = 10;

/// How many rows to ask the backend for.
///
/// More than [`SLOTS`], because flagged items are dropped from the answer: a
/// page of exactly ten would leave the menu short by however many of them the
/// user had just copied, which reads as clippings going missing.
pub const FETCH: u32 = 40;

/// Longest menu label. v1's number, kept: past this a macOS menu grows wider
/// than the screen it drops out of.
const MAX_LABEL_CHARS: usize = 40;

/// One clipping, as a menu is allowed to see it.
///
/// Constructed only by [`Clipping::from_item`]; see the module docs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clipping {
    id: String,
    label: String,
}

impl Clipping {
    /// `None` for a flagged item, always. The plaintext is dropped on the spot
    /// rather than shortened, so no prefix of a secret survives into a label.
    pub fn from_item(item: &Item) -> Option<Self> {
        if item.is_sensitive {
            return None;
        }
        let label = label_for(&item.content);
        if label.is_empty() {
            return None;
        }
        Some(Self {
            id: item.id.clone(),
            label,
        })
    }

    /// The item's id. `copy_item` travels by id, so the plaintext never has to
    /// leave the backend in order to be put on the clipboard.
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn label(&self) -> &str {
        &self.label
    }
}

/// The first [`SLOTS`] clippings a menu may show, newest first.
pub fn menu_clippings(items: &[Item]) -> Vec<Clipping> {
    items
        .iter()
        .filter_map(Clipping::from_item)
        .take(SLOTS)
        .collect()
}

/// One line, bounded.
///
/// Newlines and tabs are collapsed because a macOS menu item renders them as
/// nothing and a two-line clipping would arrive as two words jammed together.
fn label_for(content: &str) -> String {
    let flat = content.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= MAX_LABEL_CHARS {
        return flat;
    }
    let kept: String = flat.chars().take(MAX_LABEL_CHARS - 1).collect();
    format!("{kept}…")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: &str, content: &str, is_sensitive: bool) -> Item {
        Item {
            id: id.into(),
            content: content.into(),
            content_type: "text/plain".into(),
            created_at: 1_700_000_000_000,
            pinned: false,
            is_sensitive,
            origin_device_id: "device-1".into(),
            origin_device_name: None,
            source_app_bundle_id: None,
            source_app_name: None,
            too_large_to_sync: false,
            truncated: false,
        }
    }

    /// The load-bearing test. A tray menu is unblurrable, unrevealable and
    /// often screen-shared, so a flagged item must produce no menu entry at
    /// all — not a masked one, and not a shortened one.
    #[test]
    fn a_sensitive_item_produces_no_menu_entry() {
        let secret = "AKIAIOSFODNN7EXAMPLE";
        assert_eq!(Clipping::from_item(&item("row-1", secret, true)), None);
    }

    /// Its plaintext must not survive by being long enough to be truncated
    /// into a label instead of dropped.
    #[test]
    fn a_long_secret_leaks_no_prefix_into_a_label() {
        let secret = "x".repeat(100_000);
        assert_eq!(Clipping::from_item(&item("row-1", &secret, true)), None);
    }

    /// Filtering is per item, and a flagged one must not take a slot with it —
    /// the entries after it move up rather than the menu going short.
    #[test]
    fn a_flagged_item_is_skipped_and_the_rest_still_fill_the_menu() {
        let mut items = vec![item("row-0", "public zero", false)];
        items.push(item("row-secret", "AKIAIOSFODNN7EXAMPLE", true));
        for i in 1..=SLOTS {
            items.push(item(&format!("row-{i}"), &format!("public {i}"), false));
        }

        let shown = menu_clippings(&items);
        assert_eq!(shown.len(), SLOTS);
        for clipping in &shown {
            assert_ne!(clipping.id(), "row-secret");
            assert!(!clipping.label().contains("AKIA"), "{}", clipping.label());
        }
    }

    #[test]
    fn the_menu_never_offers_more_than_its_slots() {
        let items: Vec<Item> = (0..50)
            .map(|i| item(&format!("row-{i}"), &format!("entry {i}"), false))
            .collect();
        assert_eq!(menu_clippings(&items).len(), SLOTS);
    }

    /// A menu draws neither newlines nor tabs, so a multi-line clipping has to
    /// arrive as one readable line rather than as its words run together.
    #[test]
    fn a_multi_line_clipping_becomes_one_line() {
        let clipping = Clipping::from_item(&item("row-1", "first\n\tsecond   third", false))
            .expect("not flagged");
        assert_eq!(clipping.label(), "first second third");
    }

    #[test]
    fn a_long_label_is_bounded_and_ends_in_an_ellipsis() {
        let clipping =
            Clipping::from_item(&item("row-1", &"ab".repeat(200), false)).expect("not flagged");
        assert_eq!(clipping.label().chars().count(), MAX_LABEL_CHARS);
        assert!(clipping.label().ends_with('…'));
    }

    /// Truncation counts characters, not bytes: slicing a multi-byte clipping
    /// at a byte index panics, and the panic would be in the tray refresh.
    #[test]
    fn truncation_does_not_split_a_multi_byte_character() {
        let clipping =
            Clipping::from_item(&item("row-1", &"é".repeat(200), false)).expect("not flagged");
        assert_eq!(clipping.label().chars().count(), MAX_LABEL_CHARS);
    }

    /// A clipping that is only whitespace would be a blank, clickable row.
    #[test]
    fn a_blank_clipping_is_not_offered() {
        assert_eq!(Clipping::from_item(&item("row-1", "   \n\t ", false)), None);
    }
}
