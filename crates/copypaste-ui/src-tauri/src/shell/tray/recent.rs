//! Recent clipboard entries for the menu-bar item.

use copypaste_ipc::Item;

/// How many clippings the menu offers.
///
/// A menu offers the ten most recent clippings. The popup is still the
/// place for search and the full history; ten keeps the tray useful without
/// making it a second history view.
pub const SLOTS: usize = 10;

/// How many rows to ask the backend for.
///
pub const FETCH: u32 = SLOTS as u32;

/// Longest menu label; beyond this a macOS menu grows wider than its screen.
const MAX_LABEL_CHARS: usize = 40;

/// One clipping, as a menu is allowed to see it.
///
/// Constructed only by [`Clipping::from_item`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Clipping {
    id: String,
    label: String,
}

impl Clipping {
    pub fn from_item(item: &Item) -> Option<Self> {
        let label = label_for(&item.content);
        if label.is_empty() {
            return None;
        }
        Some(Self {
            id: item.id.clone(),
            label,
        })
    }

    /// The item's id.
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

    fn item(id: &str, content: &str) -> Item {
        Item {
            id: id.into(),
            content: content.into(),
            content_type: "text/plain".into(),
            created_at: 1_700_000_000_000,
            pinned: false,
            origin_device_id: "device-1".into(),
            origin_device_name: None,
            source_app_bundle_id: None,
            source_app_name: None,
            too_large_to_sync: false,
            truncated: false,
        }
    }

    #[test]
    fn a_long_clipping_is_bounded_in_the_menu() {
        let content = "x".repeat(100_000);
        let label = Clipping::from_item(&item("row-1", &content))
            .unwrap()
            .label()
            .to_string();
        assert_eq!(label.chars().count(), MAX_LABEL_CHARS);
        assert!(label.ends_with('…'));
    }

    #[test]
    fn the_menu_preserves_the_first_entries() {
        let mut items = vec![item("row-0", "public zero")];
        items.push(item("row-secret", "AKIAIOSFODNN7EXAMPLE"));
        for i in 1..=SLOTS {
            items.push(item(&format!("row-{i}"), &format!("public {i}")));
        }

        let shown = menu_clippings(&items);
        assert_eq!(shown.len(), SLOTS);
        assert_eq!(shown[1].id(), "row-secret");
    }

    #[test]
    fn the_menu_never_offers_more_than_its_slots() {
        let items: Vec<Item> = (0..50)
            .map(|i| item(&format!("row-{i}"), &format!("entry {i}")))
            .collect();
        assert_eq!(menu_clippings(&items).len(), SLOTS);
    }

    /// A menu draws neither newlines nor tabs, so a multi-line clipping has to
    /// arrive as one readable line rather than as its words run together.
    #[test]
    fn a_multi_line_clipping_becomes_one_line() {
        let clipping =
            Clipping::from_item(&item("row-1", "first\n\tsecond   third")).expect("not blank");
        assert_eq!(clipping.label(), "first second third");
    }

    #[test]
    fn a_long_label_is_bounded_and_ends_in_an_ellipsis() {
        let clipping = Clipping::from_item(&item("row-1", &"ab".repeat(200))).expect("not blank");
        assert_eq!(clipping.label().chars().count(), MAX_LABEL_CHARS);
        assert!(clipping.label().ends_with('…'));
    }

    /// Truncation counts characters, not bytes: slicing a multi-byte clipping
    /// at a byte index panics, and the panic would be in the tray refresh.
    #[test]
    fn truncation_does_not_split_a_multi_byte_character() {
        let clipping = Clipping::from_item(&item("row-1", &"é".repeat(200))).expect("not blank");
        assert_eq!(clipping.label().chars().count(), MAX_LABEL_CHARS);
    }

    /// A clipping that is only whitespace would be a blank, clickable row.
    #[test]
    fn a_blank_clipping_is_not_offered() {
        assert_eq!(Clipping::from_item(&item("row-1", "   \n\t ")), None);
    }
}
