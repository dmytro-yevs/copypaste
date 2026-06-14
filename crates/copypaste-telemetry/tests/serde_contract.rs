//! Serde round-trip and wire-format contract tests for the telemetry types.
//!
//! Coverage gap closed by CopyPaste-bn6: the telemetry crate had no tests
//! pinning the JSON wire shape of `ReportableError` and `OsTag`. A silent
//! field rename or `serde` attribute change on these types would break any
//! backend that consumes the serialised payload — these tests catch that.

use copypaste_telemetry::{OsTag, ReportableError};

// ---------------------------------------------------------------------------
// ReportableError — serde round-trip
// ---------------------------------------------------------------------------

/// Every field must survive a `serde_json` round-trip without loss.
#[test]
fn reportable_error_round_trips() {
    let evt = ReportableError::new(
        "copypaste-daemon",
        "0.3.0-dev",
        "keychain.read_failed",
        OsTag::MacOs,
    );

    let json = serde_json::to_string(&evt).expect("serialize ReportableError");
    let back: ReportableError = serde_json::from_str(&json).expect("deserialize ReportableError");

    assert_eq!(back.crate_name, evt.crate_name);
    assert_eq!(back.crate_version, evt.crate_version);
    assert_eq!(back.error_class, evt.error_class);
    assert_eq!(back.os, evt.os);
}

/// The serialised JSON must contain the expected field names and values.
/// This pins the wire contract: if a field is renamed in the struct
/// (`error_class` → `error_kind`, etc.) the downstream consumer breaks.
#[test]
fn reportable_error_field_names_are_stable() {
    let evt = ReportableError::new(
        "copypaste-core",
        "0.6.0",
        "db.open_failed",
        OsTag::Android,
    );

    let json = serde_json::to_string(&evt).expect("serialize");

    assert!(
        json.contains("\"crate_name\""),
        "wire must have 'crate_name' key; got: {json}"
    );
    assert!(
        json.contains("\"copypaste-core\""),
        "wire must include crate name value; got: {json}"
    );
    assert!(
        json.contains("\"crate_version\""),
        "wire must have 'crate_version' key; got: {json}"
    );
    assert!(
        json.contains("\"error_class\""),
        "wire must have 'error_class' key; got: {json}"
    );
    assert!(
        json.contains("\"db.open_failed\""),
        "wire must include error_class value; got: {json}"
    );
    assert!(
        json.contains("\"os\""),
        "wire must have 'os' key; got: {json}"
    );
}

// ---------------------------------------------------------------------------
// OsTag — serde snake_case contract
// ---------------------------------------------------------------------------

/// `OsTag` variants must serialise as lowercase strings (enforced by
/// `#[serde(rename_all = "lowercase")]`). Pinning the exact strings
/// means a variant rename (`MacOs` → `Macos`) is caught immediately.
#[test]
fn os_tag_serializes_as_lowercase_strings() {
    let cases = [
        (OsTag::MacOs, "\"macos\""),
        (OsTag::Windows, "\"windows\""),
        (OsTag::Android, "\"android\""),
        (OsTag::Unknown, "\"unknown\""),
    ];

    for (tag, expected) in cases {
        let actual = serde_json::to_string(&tag).expect("serialize OsTag");
        assert_eq!(
            actual, expected,
            "OsTag::{tag:?} must serialise as {expected}, got: {actual}"
        );
    }
}

/// Every `OsTag` must round-trip through `serde_json`.
#[test]
fn os_tag_round_trips() {
    for tag in [OsTag::MacOs, OsTag::Windows, OsTag::Android, OsTag::Unknown] {
        let json = serde_json::to_string(&tag).expect("serialize");
        let back: OsTag = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, tag, "OsTag round-trip failed for {tag:?}");
    }
}

/// `OsTag::current()` must return a valid, serialisable tag.
///
/// On macOS (the primary dev platform) it must be `MacOs`; everywhere else
/// we just assert it produces a value that round-trips without panicking.
#[test]
fn os_tag_current_is_valid() {
    let tag = OsTag::current();
    let json = serde_json::to_string(&tag).expect("OsTag::current() must serialise");
    let back: OsTag = serde_json::from_str(&json).expect("OsTag::current() must deserialise");
    assert_eq!(back, tag);

    #[cfg(target_os = "macos")]
    assert_eq!(tag, OsTag::MacOs, "on macOS, OsTag::current() must be MacOs");
}

// ---------------------------------------------------------------------------
// ReportableError — empty-string edge cases
// ---------------------------------------------------------------------------

/// Empty strings in all string fields must serialise and deserialise without
/// error — the scrubber and backend must handle "empty taxonomy" gracefully.
#[test]
fn reportable_error_empty_strings_are_valid() {
    let evt = ReportableError::new("", "", "", OsTag::Unknown);
    let json = serde_json::to_string(&evt).expect("serialize empty-string event");
    let back: ReportableError = serde_json::from_str(&json).expect("deserialize empty-string event");
    assert_eq!(back.crate_name, "");
    assert_eq!(back.error_class, "");
    assert_eq!(back.os, OsTag::Unknown);
}

/// Unicode characters in `error_class` must round-trip without corruption —
/// relevant when error messages from external libraries contain non-ASCII.
#[test]
fn reportable_error_unicode_error_class_round_trips() {
    let evt = ReportableError::new(
        "copypaste-daemon",
        "1.0.0",
        "错误.io_失败",   // CJK + ASCII mix (regression guard)
        OsTag::Unknown,
    );
    let json = serde_json::to_string(&evt).expect("serialize unicode event");
    let back: ReportableError = serde_json::from_str(&json).expect("deserialize unicode event");
    assert_eq!(back.error_class, "错误.io_失败");
}
