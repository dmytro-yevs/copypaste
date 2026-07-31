//! The menu-bar item's user-facing strings.
//!
//! Authored in `src/i18n/en/tray.ts` like every other string in the app. Rust
//! cannot import the catalogue, so these are a copy — and [`tests`] reads the
//! catalogue and fails if a copy has drifted, which is what stops "the strings
//! live in i18n" from being a claim nothing checks.

pub const SHOW: &str = "Show CopyPaste";
pub const OPEN_AT_LOGIN: &str = "Open at Login";
pub const QUIT: &str = "Quit CopyPaste";

pub const RECENT: &str = "Recent clippings";
pub const RECENT_EMPTY: &str = "Nothing copied yet";
pub const RECENT_OFFLINE: &str = "Can't reach the background service";

pub const STATUS_CAPTURING: &str = "Capturing what you copy";
pub const STATUS_STOPPED: &str = "Not capturing";
pub const STATUS_OFFLINE: &str = "Background service not running";

/// Every string above, with the catalogue key it must equal.
#[cfg(test)]
const CATALOGUE: &[(&str, &str)] = &[
    ("show", SHOW),
    ("openAtLogin", OPEN_AT_LOGIN),
    ("quit", QUIT),
    ("recent", RECENT),
    ("recentEmpty", RECENT_EMPTY),
    ("recentOffline", RECENT_OFFLINE),
    ("statusCapturing", STATUS_CAPTURING),
    ("statusStopped", STATUS_STOPPED),
    ("statusOffline", STATUS_OFFLINE),
];

#[cfg(test)]
mod tests {
    use super::*;

    fn catalogue_source() -> String {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../src/i18n/en/tray.ts")
            .canonicalize()
            .expect("the tray catalogue is a sibling of this crate");
        std::fs::read_to_string(path).expect("the tray catalogue is readable")
    }

    /// A tray label that has drifted from the catalogue is a string that
    /// escaped every rule the catalogue tests apply — including that no
    /// user-facing string may name a path (INV-12).
    #[test]
    fn every_label_is_the_one_the_catalogue_authored() {
        let source = catalogue_source();
        for (key, value) in CATALOGUE {
            let entry = format!("{key}: {value:?}");
            assert!(
                source.contains(&entry),
                "src/i18n/en/tray.ts has no `{entry}`"
            );
        }
    }

    /// The other direction: an entry added to the catalogue and never wired up
    /// here would be a label the app does not show.
    #[test]
    fn the_catalogue_holds_nothing_this_menu_does_not_render() {
        let source = catalogue_source();
        let keys: Vec<&str> = source
            .lines()
            .filter_map(|line| line.trim().split_once(": \""))
            .map(|(key, _)| key)
            .collect();
        assert!(!keys.is_empty(), "no entries were parsed");
        for key in keys {
            assert!(
                CATALOGUE.iter().any(|(known, _)| *known == key),
                "src/i18n/en/tray.ts declares `{key}`, which no menu item uses"
            );
        }
    }
}
