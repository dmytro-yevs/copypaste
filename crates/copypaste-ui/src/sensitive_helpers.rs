//! v0.3 T3 — UI-side preference for hiding sensitive history items.
//!
//! The daemon marks items with `is_sensitive` (heuristics over content type
//! and PII patterns), but the *display* choice — render `••••` placeholders
//! vs. plain previews — belongs to the UI layer. Persisting it daemon-side
//! would require a new IPC field; keeping it UI-side avoids that round-trip
//! and matches the v0.3 plan ("sensitive-item redaction toggle" lives in
//! Settings).
//!
//! Storage: a tiny JSON file under
//! `~/Library/Application Support/CopyPaste/ui_prefs.json` (macOS) or the
//! platform equivalent. Schema is intentionally minimal so future toggles
//! can be appended without bumping a version.
//!
//! Failure is non-fatal — a missing or unreadable file falls back to the
//! built-in default (`hide_sensitive = true`, "redacted by default" is the
//! safer choice). Errors are logged via `tracing` and otherwise swallowed
//! so a corrupt prefs file never blocks the UI from starting.

use std::path::PathBuf;

/// Built-in default: redact sensitive items unless the user opts out.
/// "Secure by default" — surfacing API keys / passwords on first launch
/// would be the worse failure mode.
pub const DEFAULT_HIDE_SENSITIVE: bool = true;

/// In-memory shape of the preferences file. Serialised as a flat JSON
/// object so future keys can be added with `#[serde(default)]` without a
/// migration step.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UiPrefs {
    #[serde(default = "default_hide_sensitive")]
    pub hide_sensitive: bool,
}

fn default_hide_sensitive() -> bool {
    DEFAULT_HIDE_SENSITIVE
}

impl Default for UiPrefs {
    fn default() -> Self {
        Self {
            hide_sensitive: DEFAULT_HIDE_SENSITIVE,
        }
    }
}

/// Resolve the on-disk preferences path. Returns `None` when `$HOME` is
/// unset (CI runners, sandboxed test envs) — callers should treat that as
/// "no persistence available" and fall back to the in-memory default.
pub fn prefs_path() -> Option<PathBuf> {
    let base = home::home_dir()?;
    #[cfg(target_os = "macos")]
    let dir = base.join("Library/Application Support/CopyPaste");
    #[cfg(not(target_os = "macos"))]
    let dir = base.join(".config/copypaste");
    Some(dir.join("ui_prefs.json"))
}

/// Load preferences from disk, falling back to defaults on any error.
/// Never panics, never surfaces an `Err` — the UI must boot even if the
/// prefs file is corrupt.
pub fn load() -> UiPrefs {
    load_from(prefs_path().as_deref())
}

/// Testable inner loader — accepts an explicit path so unit tests can
/// point at a `tempfile::NamedTempFile` without touching the user's real
/// prefs.
pub fn load_from(path: Option<&std::path::Path>) -> UiPrefs {
    let Some(path) = path else {
        return UiPrefs::default();
    };
    let Ok(bytes) = std::fs::read(path) else {
        return UiPrefs::default();
    };
    match serde_json::from_slice::<UiPrefs>(&bytes) {
        Ok(prefs) => prefs,
        Err(e) => {
            tracing::warn!(error = %e, path = %path.display(), "ui_prefs.json malformed, using defaults");
            UiPrefs::default()
        }
    }
}

/// Persist preferences to disk. Failure is logged and swallowed — losing
/// a write is preferable to crashing the settings save flow.
pub fn save(prefs: &UiPrefs) {
    let Some(path) = prefs_path() else { return };
    save_to(path.as_path(), prefs);
}

/// Testable inner writer — see `load_from` for rationale.
pub fn save_to(path: &std::path::Path, prefs: &UiPrefs) {
    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            tracing::warn!(error = %e, path = %parent.display(), "ui_prefs parent dir create failed");
            return;
        }
    }
    match serde_json::to_vec_pretty(prefs) {
        Ok(bytes) => {
            if let Err(e) = std::fs::write(path, bytes) {
                tracing::warn!(error = %e, path = %path.display(), "ui_prefs write failed");
            }
        }
        Err(e) => {
            tracing::warn!(error = %e, "ui_prefs serialise failed");
        }
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn default_redacts_by_default() {
        assert!(UiPrefs::default().hide_sensitive);
    }

    #[test]
    fn load_from_missing_file_returns_default() {
        let dir = tempdir().unwrap();
        let missing = dir.path().join("nope.json");
        assert!(load_from(Some(&missing)).hide_sensitive);
    }

    #[test]
    fn load_from_none_returns_default() {
        assert!(load_from(None).hide_sensitive);
    }

    #[test]
    fn load_from_garbage_returns_default() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("ui_prefs.json");
        std::fs::write(&p, b"not json").unwrap();
        // Must not panic; falls back to default.
        assert!(load_from(Some(&p)).hide_sensitive);
    }

    #[test]
    fn round_trip_preserves_toggle() {
        let dir = tempdir().unwrap();
        let p = dir.path().join("ui_prefs.json");
        save_to(&p, &UiPrefs { hide_sensitive: false });
        let loaded = load_from(Some(&p));
        assert!(!loaded.hide_sensitive);
    }

    #[test]
    fn save_to_creates_parent_dir() {
        let dir = tempdir().unwrap();
        let nested = dir.path().join("a/b/c/ui_prefs.json");
        save_to(&nested, &UiPrefs::default());
        assert!(nested.exists(), "save_to must mkdir -p the parent");
    }

    #[test]
    fn prefs_path_lands_under_app_support_on_macos() {
        if let Some(p) = prefs_path() {
            let s = p.to_string_lossy();
            #[cfg(target_os = "macos")]
            assert!(
                s.contains("Library/Application Support/CopyPaste"),
                "expected macOS Application Support path, got {s}"
            );
            assert!(s.ends_with("ui_prefs.json"));
        }
    }
}
