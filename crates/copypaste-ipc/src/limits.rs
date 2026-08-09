//! What one item, one page and one frame may weigh.
//!
//! The text response framing and page budget live together so a maximal page
//! cannot overrun the decoder's line cap and fail every item beside it.

/// Largest plaintext content one item may carry.
///
/// 4 MiB is `copypaste_p2p::protocol::MAX_CONTENT_BYTES`, so anything storable
/// is also transportable to a peer.
pub const MAX_CONTENT_BYTES: usize = 4 * 1024 * 1024;

/// Worst-case JSON expansion of one content byte: a control character below
/// U+0020 with no short escape encodes as `\u00XX`.
const JSON_ESCAPE_WORST_CASE: usize = 6;

/// Room for the envelope around one item — ids, timestamps, flags, field names.
const ITEM_ENVELOPE_BYTES: usize = 4 * 1024;

/// Frames larger than this are rejected before allocation.
///
/// Derived rather than chosen, and the assertion below is what keeps it
/// derived. The cost is the bound on what one connection may buffer.
pub const MAX_FRAME_BYTES: usize = 32 * 1024 * 1024;

/// Content one `Page` reply may carry, across all of its items.
///
/// [`MAX_PAGE`] bounds a page's item *count*, which bounds nothing about its
/// size: a thousand maximal items is four gigabytes against a frame measured in
/// megabytes. A server fills a page until this is reached and stops early,
/// resuming from `next_cursor`. Equal to [`MAX_CONTENT_BYTES`] so one maximal
/// item is a whole page rather than a page that can never be filled — a smaller
/// budget would stall the list on such an item for good.
pub const MAX_PAGE_CONTENT_BYTES: usize = MAX_CONTENT_BYTES;

const _: () = assert!(
    MAX_FRAME_BYTES
        >= MAX_PAGE_CONTENT_BYTES * JSON_ESCAPE_WORST_CASE
            + MAX_PAGE as usize * ITEM_ENVELOPE_BYTES,
    "a full page must fit a frame with every content byte escaped"
);

/// Server-side clamp on any caller-supplied page size (manifest 04 §3.3).
///
/// Here rather than in either server because both answer the same frontend —
/// the daemon over the socket, the embedded Android backend in process — and a
/// client asking for ten million rows must get the same answer from both.
pub const MAX_PAGE: u32 = 1_000;

/// Applied when `list` is called with `limit = 0`.
pub const DEFAULT_LIST_PAGE: u32 = 50;

/// Applied when `search` is called with `limit = 0`.
pub const DEFAULT_SEARCH_PAGE: u32 = 20;

pub fn clamp_page(limit: u32, default: u32) -> u32 {
    if limit == 0 {
        default
    } else {
        limit.min(MAX_PAGE)
    }
}

pub const LIST_PREVIEW_BYTES: usize = 1024;

pub fn bound_preview(content: &mut String) -> bool {
    if content.len() <= LIST_PREVIEW_BYTES {
        return false;
    }
    let mut cut = 0;
    for (at, cluster) in
        unicode_segmentation::UnicodeSegmentation::grapheme_indices(content.as_str(), true)
    {
        if at + cluster.len() > LIST_PREVIEW_BYTES && cut > 0 {
            break;
        }
        cut = at + cluster.len();
    }
    content.truncate(cut);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{content_type, Item, Response, ResponseData};

    #[test]
    fn content_within_the_preview_bound_is_untouched() {
        let mut content = "a".repeat(LIST_PREVIEW_BYTES);
        assert!(!bound_preview(&mut content));
        assert_eq!(content.len(), LIST_PREVIEW_BYTES);
    }

    #[test]
    fn content_over_the_preview_bound_is_cut_to_it() {
        let mut content = "a".repeat(LIST_PREVIEW_BYTES + 1);
        assert!(bound_preview(&mut content));
        assert_eq!(content.len(), LIST_PREVIEW_BYTES);
    }

    #[test]
    fn the_cut_never_splits_a_grapheme_cluster() {
        let family = "\u{1f468}\u{200d}\u{1f469}\u{200d}\u{1f467}";
        let flag = "\u{1f1fa}\u{1f1e6}";
        for cluster in [family, flag] {
            let repeats = LIST_PREVIEW_BYTES / cluster.len() + 2;
            let mut content = cluster.repeat(repeats);
            assert!(bound_preview(&mut content));
            assert!(content.len() <= LIST_PREVIEW_BYTES, "{}", content.len());
            assert_eq!(
                content.len() % cluster.len(),
                0,
                "cut mid-cluster at {}",
                content.len()
            );
        }
    }

    #[test]
    fn a_single_cluster_over_the_bound_survives_whole() {
        let cluster: String = std::iter::once("\u{1f468}".to_string())
            .chain(std::iter::repeat_n("\u{200d}\u{1f466}".to_string(), 400))
            .collect();
        assert!(cluster.len() > LIST_PREVIEW_BYTES);
        let mut content = cluster.clone();
        assert!(bound_preview(&mut content));
        assert_eq!(content, cluster);
    }

    #[test]
    fn page_sizes_are_clamped() {
        assert_eq!(clamp_page(0, DEFAULT_LIST_PAGE), DEFAULT_LIST_PAGE);
        assert_eq!(clamp_page(0, DEFAULT_SEARCH_PAGE), DEFAULT_SEARCH_PAGE);
        assert_eq!(clamp_page(10, DEFAULT_LIST_PAGE), 10);
        assert_eq!(clamp_page(u32::MAX, DEFAULT_LIST_PAGE), MAX_PAGE);
    }

    fn item_of(content: String) -> Item {
        Item {
            id: "8f14e45f-ceea-467a-9f6a-1a2b3c4d5e6f".into(),
            content,
            content_type: content_type::TEXT.to_string(),
            created_at: 1_767_225_600_000,
            pinned: false,
            is_sensitive: false,
            sensitive_finding: None,
            origin_device_id: "3c6e0b8a-9c15-424f-b8b8-1a2b3c4d5e6f".into(),
            origin_device_name: Some("a device with a fairly long display name".into()),
            source_app_bundle_id: None,
            source_app_name: None,
            too_large_to_sync: false,
            truncated: false,
        }
    }

    /// **The assertion whose absence is why this survived.**
    ///
    /// The capture and frame limits once disagreed, so the daemon stored items whose reply
    /// overran the client's decoder — an EOF or a decode error rather than a
    /// typed refusal, and on `List` it took the whole page down with it.
    #[test]
    fn one_maximal_item_fits_a_frame_with_every_byte_escaped() {
        // U+0001 has no short escape, so serde_json emits the escaped
        // six-byte form for one input byte - the worst a string can expand.
        let content = "\u{1}".repeat(MAX_CONTENT_BYTES);
        assert_eq!(content.len(), MAX_CONTENT_BYTES);

        let line = serde_json::to_string(&Response::ok(1, ResponseData::Item(item_of(content))))
            .expect("a response serialises");

        assert!(
            line.len() > MAX_CONTENT_BYTES * 5,
            "the escaping this test exists to measure did not happen: {}",
            line.len()
        );
        assert!(
            line.len() <= MAX_FRAME_BYTES,
            "a storable item produced a {} byte frame, over the {MAX_FRAME_BYTES} byte cap",
            line.len()
        );
    }

    /// The ordinary case has room to spare, so the cap above is not one a real
    /// item can creep up on.
    #[test]
    fn an_unescaped_maximal_item_leaves_the_frame_mostly_empty() {
        let line = serde_json::to_string(&Response::ok(
            1,
            ResponseData::Item(item_of("a".repeat(MAX_CONTENT_BYTES))),
        ))
        .expect("a response serialises");
        assert!(
            line.len() < MAX_CONTENT_BYTES + ITEM_ENVELOPE_BYTES,
            "{}",
            line.len()
        );
    }
}
