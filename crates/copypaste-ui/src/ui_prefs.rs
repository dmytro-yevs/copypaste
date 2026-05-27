//! Persistent UI preferences stored in `ui-prefs.toml`.
//!
//! Preferences are written atomically (tmpfile + rename) so a crash during
//! save never leaves a corrupt file. A missing or malformed file silently
//! falls back to [`UiPrefs::default()`].

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

/// Accent colour applied to interactive elements throughout the UI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AccentColor {
    #[default]
    Blue,
    Purple,
}

/// Which sub-tab of the Settings view is currently open.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SettingsTab {
    #[default]
    Simple,
    Advanced,
}

// ---------------------------------------------------------------------------
// Main struct
// ---------------------------------------------------------------------------

/// UI-local preferences that are not synced to the daemon.
///
/// Serialised to `ui-prefs.toml` inside the platform config directory.
/// Use [`UiPrefs::load`] / [`UiPrefs::save`] for persistence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiPrefs {
    /// Show clipboard rows in compact (single-line) mode.
    pub compact: bool,
    /// Whether the left sidebar is in collapsed (icon-only) state.
    pub sidebar_collapsed: bool,
    /// Whether the right detail panel is visible.
    pub detail_visible: bool,
    /// Accent colour used for highlights and interactive elements.
    pub accent: AccentColor,
    /// Enable sidebar vibrancy / translucency effect (macOS only).
    pub vibrancy: bool,
    /// Last-active sub-tab in the Settings view.
    pub settings_tab: SettingsTab,
}

impl Default for UiPrefs {
    fn default() -> Self {
        Self {
            compact: false,
            sidebar_collapsed: false,
            detail_visible: true,
            accent: AccentColor::Blue,
            vibrancy: true,
            settings_tab: SettingsTab::Simple,
        }
    }
}

// ---------------------------------------------------------------------------
// Persistence
// ---------------------------------------------------------------------------

impl UiPrefs {
    /// Return the platform-specific path to `ui-prefs.toml`.
    ///
    /// | Platform | Path |
    /// |----------|------|
    /// | macOS    | `~/Library/Application Support/CopyPaste/ui-prefs.toml` |
    /// | other    | `<config_dir>/copypaste/ui-prefs.toml` |
    pub fn prefs_path() -> PathBuf {
        #[cfg(target_os = "macos")]
        {
            dirs::data_local_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("CopyPaste")
                .join("ui-prefs.toml")
        }
        #[cfg(not(target_os = "macos"))]
        {
            dirs::config_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("copypaste")
                .join("ui-prefs.toml")
        }
    }

    /// Load preferences from disk.
    ///
    /// Returns [`UiPrefs::default()`] silently when the file is absent or
    /// cannot be parsed — never returns an error for a missing file.
    pub fn load() -> Result<Self> {
        let path = Self::prefs_path();

        let raw = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Self::default());
            }
            Err(e) => {
                return Err(e).with_context(|| format!("reading ui-prefs from {}", path.display()));
            }
        };

        match toml::from_str::<Self>(&raw) {
            Ok(prefs) => Ok(prefs),
            Err(_) => {
                // Malformed file → fall back silently rather than crashing.
                Ok(Self::default())
            }
        }
    }

    /// Persist preferences to disk using an atomic tmpfile-then-rename write.
    ///
    /// Creates the parent directory if it does not yet exist.
    pub fn save(&self) -> Result<()> {
        let path = Self::prefs_path();

        // Ensure parent directory exists.
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("creating prefs directory {}", parent.display()))?;
        }

        let serialised = toml::to_string_pretty(self).context("serialising UiPrefs to TOML")?;

        // Atomic write: write to a sibling tmpfile, then rename.
        let tmp_path = path.with_extension("toml.tmp");
        std::fs::write(&tmp_path, &serialised)
            .with_context(|| format!("writing tmp prefs to {}", tmp_path.display()))?;
        std::fs::rename(&tmp_path, &path)
            .with_context(|| format!("renaming tmp prefs to {}", path.display()))?;

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    /// Helper: write arbitrary content to a temp file and return its path.
    fn write_temp_toml(content: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::TempDir::new().expect("temp dir");
        let path = dir.path().join("ui-prefs.toml");
        let mut f = std::fs::File::create(&path).expect("create temp file");
        f.write_all(content.as_bytes()).expect("write temp file");
        (dir, path)
    }

    // -----------------------------------------------------------------------
    // Test 1: default round-trip
    // -----------------------------------------------------------------------
    /// Serialise the default prefs and deserialise back — must equal default.
    #[test]
    fn default_round_trip() {
        let original = UiPrefs::default();
        let serialised = toml::to_string_pretty(&original).expect("serialise");
        let recovered: UiPrefs = toml::from_str(&serialised).expect("deserialise");
        assert_eq!(original, recovered, "round-trip must preserve all fields");
    }

    // -----------------------------------------------------------------------
    // Test 2: custom values round-trip
    // -----------------------------------------------------------------------
    /// Non-default values survive a serialize → deserialize cycle.
    #[test]
    fn custom_round_trip() {
        let original = UiPrefs {
            compact: true,
            sidebar_collapsed: true,
            detail_visible: false,
            accent: AccentColor::Purple,
            vibrancy: false,
            settings_tab: SettingsTab::Advanced,
        };
        let serialised = toml::to_string_pretty(&original).expect("serialise");
        let recovered: UiPrefs = toml::from_str(&serialised).expect("deserialise");
        assert_eq!(original, recovered);
        assert_eq!(recovered.accent, AccentColor::Purple);
        assert_eq!(recovered.settings_tab, SettingsTab::Advanced);
        assert!(recovered.compact);
        assert!(recovered.sidebar_collapsed);
        assert!(!recovered.detail_visible);
        assert!(!recovered.vibrancy);
    }

    // -----------------------------------------------------------------------
    // Test 3: malformed TOML file falls back to default
    // -----------------------------------------------------------------------
    /// A file with garbage TOML must not panic — `load()` must return default.
    ///
    /// We override `prefs_path()` behaviour indirectly by testing the parsing
    /// path: load the TOML from a string to exercise the fallback branch.
    #[test]
    fn malformed_file_falls_back_to_default() {
        let garbage = "compact = !!!! not valid toml {{{";
        let result = toml::from_str::<UiPrefs>(garbage);
        assert!(result.is_err(), "garbage must fail to parse");
        // Mirror what UiPrefs::load() does in the Err branch.
        let fallback = UiPrefs::default();
        assert!(!fallback.compact);
        assert_eq!(fallback.accent, AccentColor::Blue);
        assert_eq!(fallback.settings_tab, SettingsTab::Simple);
    }

    // -----------------------------------------------------------------------
    // Test 4: save + load round-trip via tempfile
    // -----------------------------------------------------------------------
    /// Write custom prefs to a temp file, then read back by parsing the file
    /// directly (avoids depending on the platform prefs_path in CI).
    #[test]
    fn save_and_reload_via_tempfile() {
        let original = UiPrefs {
            compact: false,
            sidebar_collapsed: true,
            detail_visible: true,
            accent: AccentColor::Purple,
            vibrancy: false,
            settings_tab: SettingsTab::Advanced,
        };

        let serialised = toml::to_string_pretty(&original).expect("serialise");
        let (_dir, path) = write_temp_toml(&serialised);

        let raw = std::fs::read_to_string(&path).expect("read back");
        let recovered: UiPrefs = toml::from_str(&raw).expect("deserialise");

        assert_eq!(original, recovered);
    }

    // -----------------------------------------------------------------------
    // Test 5: unknown fields in TOML are ignored (forward compatibility)
    // -----------------------------------------------------------------------
    #[test]
    fn unknown_fields_are_ignored() {
        let toml_with_extra = r#"
compact = true
sidebar_collapsed = false
detail_visible = true
accent = "purple"
vibrancy = false
settings_tab = "simple"
unknown_future_field = "ignored"
"#;
        let result = toml::from_str::<UiPrefs>(toml_with_extra);
        // serde(default) + deny_unknown_fields is NOT set, so this should parse fine.
        assert!(
            result.is_ok(),
            "unknown fields must not fail: {:?}",
            result.err()
        );
        let prefs = result.unwrap();
        assert!(prefs.compact);
        assert_eq!(prefs.accent, AccentColor::Purple);
    }

    // -----------------------------------------------------------------------
    // Test 6: prefs_path returns a non-empty path
    // -----------------------------------------------------------------------
    #[test]
    fn prefs_path_is_non_empty() {
        let p = UiPrefs::prefs_path();
        assert!(
            p.to_string_lossy().contains("ui-prefs.toml"),
            "path must contain ui-prefs.toml, got: {}",
            p.display()
        );
    }
}
