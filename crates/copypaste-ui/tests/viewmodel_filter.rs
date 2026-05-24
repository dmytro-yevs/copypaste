// tests/viewmodel_filter.rs — ViewModel / Rust unit tests for filter + search logic.
//
// Tests the `filter_history_items` function from `copypaste_ui::windows` and
// the `UiPrefs` / sensitive-item redaction logic from `copypaste_ui::sensitive_helpers`.
//
// No Slint runtime is required — these are pure Rust tests.

// ── SearchableHistoryItem / filter_history_items ────────────────────────────

use copypaste_ui::windows::{filter_history_items, SearchableHistoryItem};

/// A minimal stub that satisfies `SearchableHistoryItem`.
struct Item {
    preview: &'static str,
    sensitive: bool,
    pinned: bool,
}

impl SearchableHistoryItem for Item {
    fn preview(&self) -> &str {
        self.preview
    }
}

fn item(preview: &'static str) -> Item {
    Item {
        preview,
        sensitive: false,
        pinned: false,
    }
}

fn sensitive_item(preview: &'static str) -> Item {
    Item {
        preview,
        sensitive: true,
        pinned: false,
    }
}

fn pinned_item(preview: &'static str) -> Item {
    Item {
        preview,
        sensitive: false,
        pinned: true,
    }
}

fn pinned_sensitive_item(preview: &'static str) -> Item {
    Item {
        preview,
        sensitive: true,
        pinned: true,
    }
}

// ── Basic filter_history_items tests (subset already in windows.rs inline tests;
// these are integration-level regression guards in the external test suite) ─

#[test]
fn empty_query_returns_all_items() {
    let items = vec![item("alpha"), item("beta"), item("gamma")];
    assert_eq!(
        filter_history_items(&items, "").len(),
        3,
        "empty query must return every item"
    );
}

#[test]
fn whitespace_only_query_returns_all_items() {
    let items = vec![item("alpha"), item("beta")];
    assert_eq!(
        filter_history_items(&items, "   ").len(),
        2,
        "whitespace-only query is treated as empty"
    );
}

#[test]
fn case_insensitive_substring_match() {
    let items = vec![item("Hello World"), item("RUST IS FUN"), item("nothing")];

    let upper = filter_history_items(&items, "WORLD");
    assert_eq!(upper.len(), 1);
    assert_eq!(upper[0].preview(), "Hello World");

    let lower = filter_history_items(&items, "rust");
    assert_eq!(lower.len(), 1);
    assert_eq!(lower[0].preview(), "RUST IS FUN");
}

#[test]
fn no_match_returns_empty_vec() {
    let items = vec![item("apple"), item("banana")];
    let result = filter_history_items(&items, "cherry");
    assert!(result.is_empty(), "no-match must return empty slice");
}

#[test]
fn partial_match_finds_multiple() {
    let items = vec![
        item("copypaste history"),
        item("copy this text"),
        item("unrelated content"),
        item("copy that too"),
    ];
    let result = filter_history_items(&items, "copy");
    assert_eq!(result.len(), 3, "should match all three 'copy' items");
}

// ── "Hide sensitive" filter (v0.3 T3) ──────────────────────────────────────
//
// The UI layer applies `hide_sensitive` as a pre-filter before calling
// `filter_history_items`. We replicate that logic here to verify the expected
// behaviour:
//   - when hide_sensitive=true  → items with is_sensitive=true are removed
//   - when hide_sensitive=false → all items are returned
//   - pinned items are always shown regardless of is_sensitive

fn apply_sensitive_filter(items: &[Item], hide_sensitive: bool) -> Vec<&Item> {
    if !hide_sensitive {
        return items.iter().collect();
    }
    // Mirror main.rs logic: show item if NOT sensitive OR if pinned.
    items.iter().filter(|i| !i.sensitive || i.pinned).collect()
}

#[test]
fn hide_sensitive_true_removes_sensitive_items() {
    let items = vec![
        item("plain text"),
        sensitive_item("api-key: sk-abc123"),
        item("another plain"),
        sensitive_item("password: hunter2"),
    ];

    let result = apply_sensitive_filter(&items, true);
    assert_eq!(
        result.len(),
        2,
        "hide_sensitive=true must remove 2 sensitive items"
    );
    for r in &result {
        assert!(
            !r.sensitive,
            "no sensitive item must survive hide_sensitive=true"
        );
    }
}

#[test]
fn hide_sensitive_false_returns_all_items() {
    let items = vec![item("plain"), sensitive_item("secret"), item("another")];

    let result = apply_sensitive_filter(&items, false);
    assert_eq!(
        result.len(),
        3,
        "hide_sensitive=false must return all items including sensitive"
    );
}

#[test]
fn pinned_sensitive_items_always_shown_when_hide_sensitive_true() {
    let items = vec![
        sensitive_item("unpinned secret"),
        pinned_sensitive_item("pinned secret — always visible"),
        item("plain text"),
    ];

    let result = apply_sensitive_filter(&items, true);

    // unpinned-sensitive is hidden; pinned-sensitive and plain survive.
    assert_eq!(
        result.len(),
        2,
        "pinned sensitive item must remain visible even when hide_sensitive=true"
    );
    let previews: Vec<&str> = result.iter().map(|i| i.preview()).collect();
    assert!(
        previews.contains(&"pinned secret — always visible"),
        "pinned sensitive item must be present in result"
    );
    assert!(
        !previews.contains(&"unpinned secret"),
        "unpinned sensitive item must be absent when hide_sensitive=true"
    );
}

#[test]
fn pinned_non_sensitive_items_always_shown() {
    let items = vec![
        pinned_item("pinned plain text"),
        item("regular"),
        sensitive_item("hidden secret"),
    ];

    let result = apply_sensitive_filter(&items, true);
    assert_eq!(result.len(), 2);
    assert!(
        result.iter().any(|i| i.preview() == "pinned plain text"),
        "pinned non-sensitive must be visible"
    );
}

// ── Search query combined with sensitive filter ─────────────────────────────

#[test]
fn search_after_sensitive_filter_combines_correctly() {
    let items = vec![
        item("visible rust code"),
        sensitive_item("rust secret key"),
        item("python code"),
    ];

    // First determine which items survive the sensitive filter.
    let visible: Vec<&Item> = apply_sensitive_filter(&items, true);
    // Then apply search on the visible subset using preview() directly.
    let needle = "rust";
    let after_search: Vec<&Item> = visible
        .into_iter()
        .filter(|i| i.preview().to_lowercase().contains(needle))
        .collect();

    assert_eq!(
        after_search.len(),
        1,
        "search after sensitive filter must find only non-sensitive 'rust' items"
    );
    assert_eq!(after_search[0].preview(), "visible rust code");
}

// ── UiPrefs sensitive helpers ───────────────────────────────────────────────

#[test]
fn ui_prefs_default_hide_sensitive_is_true() {
    use copypaste_ui::sensitive_helpers::{UiPrefs, DEFAULT_HIDE_SENSITIVE};
    // DEFAULT_HIDE_SENSITIVE is a compile-time constant; verify it via a
    // runtime assertion on the struct field rather than the constant directly.
    let prefs = UiPrefs::default();
    assert!(
        prefs.hide_sensitive,
        "UiPrefs::default() must have hide_sensitive=true (secure-by-default)"
    );
    // Also assert the constant matches the struct default.
    assert_eq!(
        prefs.hide_sensitive, DEFAULT_HIDE_SENSITIVE,
        "UiPrefs::default().hide_sensitive must equal DEFAULT_HIDE_SENSITIVE"
    );
}

#[test]
fn ui_prefs_round_trip_via_tempfile() {
    use copypaste_ui::sensitive_helpers::{load_from, save_to, UiPrefs};
    use tempfile::tempdir;

    let dir = tempdir().unwrap();
    let path = dir.path().join("ui_prefs.json");

    // Write hide_sensitive=false, read back.
    save_to(
        &path,
        &UiPrefs {
            hide_sensitive: false,
        },
    );
    let loaded = load_from(Some(&path));
    assert!(
        !loaded.hide_sensitive,
        "round-trip: hide_sensitive=false must survive save/load"
    );

    // Write hide_sensitive=true, read back.
    save_to(
        &path,
        &UiPrefs {
            hide_sensitive: true,
        },
    );
    let loaded = load_from(Some(&path));
    assert!(
        loaded.hide_sensitive,
        "round-trip: hide_sensitive=true must survive save/load"
    );
}

#[test]
fn ui_prefs_load_from_missing_file_returns_default() {
    use copypaste_ui::sensitive_helpers::{load_from, DEFAULT_HIDE_SENSITIVE};
    use tempfile::tempdir;

    let dir = tempdir().unwrap();
    let missing = dir.path().join("nonexistent.json");
    let prefs = load_from(Some(&missing));
    assert_eq!(
        prefs.hide_sensitive, DEFAULT_HIDE_SENSITIVE,
        "missing file must fall back to default"
    );
}

#[test]
fn ui_prefs_load_from_corrupt_file_returns_default() {
    use copypaste_ui::sensitive_helpers::{load_from, DEFAULT_HIDE_SENSITIVE};
    use tempfile::tempdir;

    let dir = tempdir().unwrap();
    let path = dir.path().join("corrupt.json");
    std::fs::write(&path, b"not valid json {{{").unwrap();

    let prefs = load_from(Some(&path));
    assert_eq!(
        prefs.hide_sensitive, DEFAULT_HIDE_SENSITIVE,
        "corrupt file must fall back to default without panic"
    );
}

#[test]
fn ui_prefs_load_from_none_returns_default() {
    use copypaste_ui::sensitive_helpers::load_from;
    let prefs = load_from(None);
    assert!(
        prefs.hide_sensitive,
        "None path must return default (hide_sensitive=true)"
    );
}
