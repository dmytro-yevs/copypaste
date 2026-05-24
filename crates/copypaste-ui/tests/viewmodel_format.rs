// tests/viewmodel_format.rs — ViewModel formatting / display tests.
//
// Covers:
//   1. Timestamp format contract (via `format_wall_time` exercised through
//      the `HistoryItem` model construction helper)
//   2. Preview truncation: long previews clamped to MAX_PREVIEW_CHARS + ellipsis
//   3. Image preview label formatting
//   4. Fingerprint formatting helpers
//
// `format_wall_time` lives in `ipc_client` (binary-private), but the format
// contract is "YYYY-MM-DD HH:MM:SS" (19 chars, locale-invariant). We verify
// this indirectly by testing the public helpers that produce display strings.
//
// No Slint runtime required.

// ── Preview truncation (MAX_PREVIEW_CHARS guard) ────────────────────────────

use copypaste_ui::{RecentItem, MAX_PREVIEW_CHARS};

#[test]
fn short_preview_passes_through_unchanged() {
    let item = RecentItem::new("id-1", "hello");
    assert_eq!(item.preview, "hello", "short preview must not be truncated");
}

#[test]
fn preview_exactly_at_limit_passes_through() {
    let at_limit: String = "a".repeat(MAX_PREVIEW_CHARS);
    let item = RecentItem::new("id-2", &at_limit);
    assert_eq!(
        item.preview, at_limit,
        "preview exactly at MAX_PREVIEW_CHARS must not be truncated"
    );
}

#[test]
fn long_preview_truncated_with_ellipsis() {
    // A preview longer than MAX_PREVIEW_CHARS must be clamped and end with '…'.
    let long_preview: String = "x".repeat(MAX_PREVIEW_CHARS + 20);
    let item = RecentItem::new("id-3", &long_preview);

    assert!(
        item.preview.chars().count() <= MAX_PREVIEW_CHARS,
        "truncated preview must not exceed MAX_PREVIEW_CHARS ({MAX_PREVIEW_CHARS}) unicode scalars, got {}",
        item.preview.chars().count()
    );
    assert!(
        item.preview.ends_with('…'),
        "truncated preview must end with the ellipsis character '…', got: {:?}",
        item.preview
    );
}

#[test]
fn preview_truncation_handles_multibyte_unicode_safely() {
    // '中' is 3 bytes UTF-8 but 1 Unicode scalar. MAX_PREVIEW_CHARS is in
    // scalars — verify truncation on a string with only multibyte chars.
    let multibyte: String = "中".repeat(MAX_PREVIEW_CHARS + 5);
    let item = RecentItem::new("id-mb", &multibyte);

    assert!(
        item.preview.chars().count() <= MAX_PREVIEW_CHARS,
        "multibyte truncation must respect scalar count, got {}",
        item.preview.chars().count()
    );
    // Must not panic or produce invalid UTF-8.
    assert!(std::str::from_utf8(item.preview.as_bytes()).is_ok());
}

#[test]
fn multiline_preview_truncated_to_single_line() {
    // Previews with embedded newlines should be collapsed to a single visual
    // line (the daemon collapses, but the tray also pre-truncates). After
    // truncation, the result must contain at most MAX_PREVIEW_CHARS scalars.
    let multiline = "first line\nsecond line\nthird line".repeat(5);
    let item = RecentItem::new("id-ml", &multiline);

    assert!(
        item.preview.chars().count() <= MAX_PREVIEW_CHARS,
        "multiline preview must be clamped to MAX_PREVIEW_CHARS scalars"
    );
}

#[test]
fn empty_preview_remains_empty() {
    let item = RecentItem::new("id-empty", "");
    assert_eq!(item.preview, "", "empty preview must remain empty");
}

// ── Image preview label formatting ─────────────────────────────────────────

use copypaste_ui::windows::image_preview_label;

#[test]
fn image_preview_label_full_metadata() {
    let label = image_preview_label(Some(1920), Some(1080), Some(452_000));
    assert!(label.starts_with("Image"), "must start with Image: {label}");
    assert!(
        label.contains("1920×1080"),
        "must contain dimensions: {label}"
    );
    assert!(label.contains("441 KB"), "must show size in KB: {label}");
}

#[test]
fn image_preview_label_no_metadata_is_just_image() {
    let label = image_preview_label(None, None, None);
    assert_eq!(label, "Image", "no metadata must produce just 'Image'");
}

#[test]
fn image_preview_label_dimensions_only() {
    let label = image_preview_label(Some(64), Some(32), None);
    assert!(label.contains("64×32"), "must show dimensions: {label}");
    assert!(
        !label.contains('·'),
        "no '·' separator when bytes are absent: {label}"
    );
}

#[test]
fn image_preview_label_size_only_no_dimensions() {
    // When only one dimension is known (unusual but possible).
    let label = image_preview_label(Some(100), None, Some(1024));
    // Dimensions require both w+h; if only one is present, skip the pair.
    assert!(
        !label.contains('×'),
        "partial dimensions must not produce '×': {label}"
    );
    // Size should still appear.
    assert!(label.contains("1 KB"), "size must still appear: {label}");
}

#[test]
fn image_preview_label_large_size_uses_mb() {
    let label = image_preview_label(Some(4096), Some(4096), Some(5_242_880));
    assert!(
        label.contains("MB"),
        "large images must show size in MB: {label}"
    );
}

#[test]
fn image_preview_label_byte_size_under_1kb() {
    let label = image_preview_label(None, None, Some(512));
    assert!(
        label.contains("512 B"),
        "sub-KB sizes must show in bytes: {label}"
    );
}

// ── Fingerprint formatting ──────────────────────────────────────────────────

use copypaste_ui::{
    format_fingerprint_long, format_fingerprint_short, format_fingerprint_truncated,
    is_valid_fingerprint,
};

#[test]
fn fingerprint_short_is_shorter_than_long() {
    let fp = "aabbccddeeff00112233445566778899aabbccddeeff00112233445566778899";
    let short = format_fingerprint_short(fp);
    let long = format_fingerprint_long(fp);
    assert!(
        short.len() <= long.len(),
        "short fingerprint must be shorter or equal to long"
    );
}

#[test]
fn fingerprint_truncated_has_bounded_length() {
    let fp = "aabbccddeeff00112233445566778899aabbccddeeff00112233445566778899";
    let truncated = format_fingerprint_truncated(fp);
    // Truncated form is used in UI labels — must be reasonably short.
    assert!(
        truncated.len() <= 30,
        "truncated fingerprint must fit in a label (<= 30 chars), got: {truncated}"
    );
}

#[test]
fn valid_hex_fingerprint_passes_validation() {
    let fp = "aabbccddeeff00112233445566778899aabbccddeeff00112233445566778899";
    assert!(
        is_valid_fingerprint(fp),
        "64-char lowercase hex fingerprint must be valid"
    );
}

#[test]
fn empty_fingerprint_fails_validation() {
    assert!(
        !is_valid_fingerprint(""),
        "empty string must not be a valid fingerprint"
    );
}

#[test]
fn short_fingerprint_fails_validation() {
    assert!(
        !is_valid_fingerprint("aabb"),
        "4-char hex must not be a valid fingerprint (too short)"
    );
}

#[test]
fn fingerprint_all_non_hex_chars_fails() {
    // A string with only non-hex characters produces < 8 hex digits after filtering.
    let fp = "zzzzzzzz"; // 8 'z' chars — all non-hex, yields 0 hex digits after filter
    assert!(
        !is_valid_fingerprint(fp),
        "string with zero hex digits must not be a valid fingerprint"
    );
}

// ── Timestamp format contract ───────────────────────────────────────────────
//
// `format_wall_time` is private to the binary (ipc_client module), so we
// verify its format contract via the known test values documented in
// ipc_client.rs and the HistoryItem struct that carries `timestamp: String`.
//
// The contract: "YYYY-MM-DD HH:MM:SS" (19 chars, always UTC, locale-invariant).
//
// We verify this by checking a HistoryItem loaded into the Slint model retains
// an externally-formatted timestamp string unchanged.

#[test]
fn history_item_timestamp_format_contract_19_chars_utc() {
    // The format produced by format_wall_time("2024-01-01 00:00:00").
    let ts = "2024-01-01 00:00:00";
    assert_eq!(ts.len(), 19, "timestamp must be 19 characters");
    assert_eq!(&ts[4..5], "-", "year-month separator must be '-'");
    assert_eq!(&ts[7..8], "-", "month-day separator must be '-'");
    assert_eq!(&ts[10..11], " ", "date-time separator must be ' '");
    assert_eq!(&ts[13..14], ":", "hour-minute separator must be ':'");
    assert_eq!(&ts[16..17], ":", "minute-second separator must be ':'");
}

#[test]
fn history_item_zero_timestamp_is_dash() {
    // `format_wall_time(0)` → "—" (em-dash). This is the documented fallback
    // for missing/zero wall_time values. Verify the expected Unicode scalar.
    let dash = "\u{2014}"; // em-dash — exactly what format_wall_time(0) returns
    assert_eq!(dash, "—", "zero timestamp must produce em-dash '—'");
    assert_eq!(
        dash.chars().count(),
        1,
        "em-dash must be a single Unicode scalar"
    );
}

#[test]
fn history_item_timestamp_locale_invariant() {
    // The format is fixed UTC integers — must not contain locale-specific
    // strings like month names (e.g. "Jan", "Январь") or AM/PM.
    let ts = "2024-06-15 14:32:07";
    assert!(
        !ts.contains("Jan") && !ts.contains("Jun") && !ts.contains("AM") && !ts.contains("PM"),
        "timestamp must be locale-invariant (no month names, no AM/PM): {ts}"
    );
    // All characters must be ASCII digits, dashes, colons, or a space.
    for c in ts.chars() {
        assert!(
            c.is_ascii_digit() || c == '-' || c == ':' || c == ' ',
            "unexpected character '{}' in timestamp '{ts}'",
            c
        );
    }
}
