//! Human-readable output.
//!
//! Everything here is a pure function of typed [`copypaste_ipc`] values plus an
//! explicit `now`, so the formatting is unit-testable without a daemon and
//! without a wall clock.

use comfy_table::{presets, ContentArrangement, Table};
use copypaste_ipc::{Item, StatusData, PROTOCOL_VERSION};

/// Stand-in printed instead of a sensitive item's content.
///
/// The list view must never render the plaintext of an item the detector
/// flagged (CLAUDE.md rule 4 / port manifest 07 I9). Spelled the same way the
/// core redactor spells it so the two read as one convention.
pub const REDACTED: &str = "***REDACTED***";

/// Marks a pinned row.
pub const PIN_GLYPH: &str = "*";
/// Marks a sensitive row.
pub const SENSITIVE_GLYPH: &str = "!";

/// How much of an item's content a table row shows.
const CONTENT_WIDTH: usize = 56;

/// Compact "how long ago", e.g. `12m ago`.
///
/// `created_at_ms` and `now_ms` are both milliseconds since the Unix epoch, the
/// unit [`Item::created_at`] uses.
pub fn relative_time(created_at_ms: i64, now_ms: i64) -> String {
    let secs = (now_ms - created_at_ms) / 1000;
    if secs < 0 {
        // Clock skew between the daemon's write and this process' read. Saying
        // "now" is less alarming, and less wrong, than "-3s ago".
        return "now".to_string();
    }
    match secs {
        0..=4 => "now".to_string(),
        5..=59 => format!("{secs}s ago"),
        60..=3599 => format!("{}m ago", secs / 60),
        3600..=86_399 => format!("{}h ago", secs / 3600),
        86_400..=604_799 => format!("{}d ago", secs / 86_400),
        604_800..=31_535_999 => format!("{}w ago", secs / 604_800),
        _ => format!("{}y ago", secs / 31_536_000),
    }
}

/// Collapse content to a single display line of at most `max` characters.
///
/// Newlines end the line, control characters become spaces, and either kind of
/// shortening is signalled with an ellipsis so a truncated value is never
/// mistaken for the whole value.
pub fn one_line(content: &str, max: usize) -> String {
    let first = content.split(['\n', '\r']).next().unwrap_or("");
    // A trailing newline is not "more content"; anything else after the first
    // line is, and the reader has to be told the value was cut.
    let had_more = !content[first.len()..].trim_matches(['\n', '\r']).is_empty();

    let cleaned: String = first
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let cleaned = cleaned.trim_end();

    if cleaned.chars().count() > max {
        let head: String = cleaned.chars().take(max.saturating_sub(1)).collect();
        return format!("{head}…");
    }
    if had_more && !cleaned.is_empty() {
        return format!("{cleaned}…");
    }
    if cleaned.is_empty() && had_more {
        return "…".to_string();
    }
    cleaned.to_string()
}

/// What a row shows in its content column.
///
/// Sensitive items are redacted here, once, so no caller can forget.
pub fn item_preview(item: &Item, max: usize) -> String {
    if item.is_sensitive {
        return REDACTED.to_string();
    }
    let line = one_line(&item.content, max);
    if line.is_empty() {
        // Non-text items (images, files) may carry no printable content.
        return format!("[{}]", item.content_type);
    }
    line
}

/// Per-row flag glyphs: pinned and/or sensitive.
pub fn item_flags(item: &Item) -> String {
    let mut flags = String::new();
    if item.pinned {
        flags.push_str(PIN_GLYPH);
    }
    if item.is_sensitive {
        flags.push_str(SENSITIVE_GLYPH);
    }
    flags
}

/// Render items as a table, or `empty` when there are none.
pub fn items_table(items: &[Item], now_ms: i64, empty: &str) -> String {
    if items.is_empty() {
        return empty.to_string();
    }

    let mut table = Table::new();
    table.load_preset(presets::UTF8_HORIZONTAL_ONLY);
    // Disabled, not Dynamic: content is already truncated to a known width, and
    // a fixed arrangement keeps output identical in a pipe and in a terminal.
    table.set_content_arrangement(ContentArrangement::Disabled);
    table.set_header(vec!["ID", "AGE", "FLAGS", "CONTENT"]);

    for item in items {
        table.add_row(vec![
            item.id.clone(),
            relative_time(item.created_at, now_ms),
            item_flags(item),
            item_preview(item, CONTENT_WIDTH),
        ]);
    }

    let mut out = table.to_string();
    let any_pinned = items.iter().any(|i| i.pinned);
    let any_sensitive = items.iter().any(|i| i.is_sensitive);
    if any_pinned || any_sensitive {
        let mut legend = Vec::new();
        if any_pinned {
            legend.push(format!("{PIN_GLYPH} pinned"));
        }
        if any_sensitive {
            legend.push(format!("{SENSITIVE_GLYPH} sensitive (content hidden)"));
        }
        out.push('\n');
        out.push_str(&legend.join("   "));
    }
    out
}

/// Render `status` as an aligned key/value block.
pub fn status_text(status: &StatusData) -> String {
    let mut lines = vec![
        format!("{:<12} {}", "daemon", "running"),
        format!("{:<12} {}", "version", status.version),
        format!(
            "{:<12} {} (this CLI: {})",
            "protocol", status.protocol_version, PROTOCOL_VERSION
        ),
        format!("{:<12} {}", "items", status.item_count),
        format!(
            "{:<12} {}",
            "capture",
            if status.capture_running {
                "running"
            } else {
                "stopped"
            }
        ),
        format!("{:<12} {}", "clipboard", clipboard_backend(status)),
    ];

    if status.protocol_version != PROTOCOL_VERSION {
        // Manifest 04 §6.2: a client must not silently continue across a
        // protocol difference.
        lines.push(String::new());
        lines.push(
            "warning: the daemon and this CLI speak different IPC protocol versions. \
             Upgrade both to the same release and restart the daemon."
                .to_string(),
        );
    }

    lines.join("\n")
}

/// The backend string, annotated when it is not the real system clipboard.
///
/// `StatusData::clipboard_backend` exists so a demo cannot be mistaken for the
/// real thing; passing it through unannotated would defeat that.
fn clipboard_backend(status: &StatusData) -> String {
    let backend = &status.clipboard_backend;
    let lowered = backend.to_ascii_lowercase();
    if lowered.contains("fake") || lowered.contains("mock") || lowered.contains("null") {
        format!("{backend} (not the system clipboard)")
    } else {
        backend.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(content: &str) -> Item {
        Item {
            id: "3f2a91c4-0000-4000-8000-000000000001".into(),
            content: content.into(),
            content_type: "text/plain".into(),
            created_at: 1_000_000,
            pinned: false,
            is_sensitive: false,
        }
    }

    const MIN: i64 = 60_000;
    const HOUR: i64 = 60 * MIN;
    const DAY: i64 = 24 * HOUR;

    #[test]
    fn relative_time_buckets() {
        let now = 10 * DAY;
        assert_eq!(relative_time(now, now), "now");
        assert_eq!(relative_time(now - 3_000, now), "now");
        assert_eq!(relative_time(now - 30_000, now), "30s ago");
        assert_eq!(relative_time(now - 5 * MIN, now), "5m ago");
        assert_eq!(relative_time(now - 3 * HOUR, now), "3h ago");
        assert_eq!(relative_time(now - 2 * DAY, now), "2d ago");
        assert_eq!(relative_time(now - 9 * DAY, now), "1w ago");
    }

    #[test]
    fn relative_time_tolerates_a_future_timestamp() {
        assert_eq!(relative_time(2_000, 1_000), "now");
    }

    #[test]
    fn one_line_keeps_short_single_line_content_verbatim() {
        assert_eq!(one_line("hello", 20), "hello");
    }

    #[test]
    fn one_line_stops_at_the_first_newline() {
        assert_eq!(one_line("first\nsecond", 20), "first…");
    }

    #[test]
    fn one_line_truncates_to_max_chars() {
        let out = one_line(&"x".repeat(100), 10);
        assert_eq!(out.chars().count(), 10);
        assert!(out.ends_with('…'));
    }

    #[test]
    fn one_line_truncates_on_char_boundaries() {
        let out = one_line("日本語のテキストがとても長い場合", 5);
        assert_eq!(out, "日本語の…");
    }

    #[test]
    fn one_line_replaces_control_characters() {
        assert_eq!(one_line("a\tb\u{7}c", 20), "a b c");
    }

    #[test]
    fn one_line_ignores_a_trailing_newline_as_content() {
        // A trailing newline alone must not make the value look truncated.
        assert_eq!(one_line("hello\n", 20), "hello");
        assert_eq!(one_line("hello\n\n", 20), "hello");
    }

    #[test]
    fn sensitive_content_is_never_rendered() {
        let mut it = item("sk-live-4eC39HqLyjWDarjtT1zdp7dc");
        it.is_sensitive = true;
        let preview = item_preview(&it, 56);
        assert_eq!(preview, REDACTED);
        assert!(!preview.contains("sk-live"));
    }

    #[test]
    fn sensitive_content_is_absent_from_the_whole_table() {
        let mut it = item("AKIAIOSFODNN7EXAMPLE");
        it.is_sensitive = true;
        let table = items_table(&[it], 1_000_000, "no items");
        assert!(!table.contains("AKIAIOSFODNN7EXAMPLE"), "{table}");
        assert!(table.contains(REDACTED), "{table}");
        assert!(table.contains(SENSITIVE_GLYPH), "{table}");
    }

    #[test]
    fn empty_content_falls_back_to_the_content_type() {
        let mut it = item("");
        it.content_type = "image/png".into();
        assert_eq!(item_preview(&it, 56), "[image/png]");
    }

    #[test]
    fn flags_mark_pinned_and_sensitive() {
        let mut it = item("x");
        assert_eq!(item_flags(&it), "");
        it.pinned = true;
        assert_eq!(item_flags(&it), PIN_GLYPH);
        it.is_sensitive = true;
        assert_eq!(item_flags(&it), format!("{PIN_GLYPH}{SENSITIVE_GLYPH}"));
    }

    #[test]
    fn empty_list_renders_the_empty_message() {
        assert_eq!(items_table(&[], 0, "no items yet"), "no items yet");
    }

    #[test]
    fn table_shows_the_full_id_so_it_can_be_copied_into_another_command() {
        let it = item("hello");
        let table = items_table(&[it.clone()], 1_000_000, "none");
        assert!(table.contains(&it.id), "{table}");
    }

    #[test]
    fn table_includes_headers_and_a_relative_age() {
        let mut it = item("hello");
        it.created_at = 0;
        let table = items_table(&[it], 5 * MIN, "none");
        assert!(table.contains("CONTENT"), "{table}");
        assert!(table.contains("5m ago"), "{table}");
    }

    #[test]
    fn table_legend_appears_only_when_a_flag_is_used() {
        let plain = items_table(&[item("hello")], 1_000_000, "none");
        assert!(!plain.contains("pinned"), "{plain}");

        let mut pinned = item("hello");
        pinned.pinned = true;
        let out = items_table(&[pinned], 1_000_000, "none");
        assert!(out.contains("pinned"), "{out}");
    }

    fn status(protocol_version: u32, backend: &str) -> StatusData {
        StatusData {
            version: "2.0.0-alpha.1".into(),
            protocol_version,
            item_count: 42,
            capture_running: true,
            clipboard_backend: backend.into(),
        }
    }

    #[test]
    fn status_reports_the_essentials() {
        let text = status_text(&status(PROTOCOL_VERSION, "macos-pasteboard"));
        assert!(text.contains("2.0.0-alpha.1"), "{text}");
        assert!(text.contains("42"), "{text}");
        assert!(text.contains("running"), "{text}");
        assert!(!text.contains("warning"), "{text}");
    }

    #[test]
    fn status_warns_on_a_protocol_difference() {
        let text = status_text(&status(PROTOCOL_VERSION + 1, "macos-pasteboard"));
        assert!(text.contains("warning"), "{text}");
        assert!(text.contains("different IPC protocol versions"), "{text}");
    }

    #[test]
    fn status_flags_a_fake_clipboard_backend() {
        let text = status_text(&status(PROTOCOL_VERSION, "fake"));
        assert!(text.contains("not the system clipboard"), "{text}");
    }

    #[test]
    fn status_never_prints_a_path() {
        let text = status_text(&status(PROTOCOL_VERSION, "macos-pasteboard"));
        assert!(!text.contains('/'), "{text}");
    }
}
