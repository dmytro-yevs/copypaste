//! Feedback for a newly stored desktop clipboard capture.
//!
//! The daemon owns capture sound because it knows whether ingest stored a new
//! item. The bundled app owns notifications because macOS notification centre
//! requires an app bundle and Windows toast identity belongs to the installed
//! executable. The two settings and permission paths remain independent.

use crate::AppState;

/// The prefix every fake clipboard backend's name carries (`fake-memory`,
/// `fake-file`).
///
/// Manifest 01 §3.23 asks for the sound to be suppressed in test environments.
/// v1's test was "is the key ephemeral"; v2 has a better one, because the fake
/// clipboard is *exactly* the set of runs that are not a real user copying
/// something — every `cargo test` and every demo script.
const FAKE_BACKEND_PREFIX: &str = "fake";

/// Something was newly stored. Play the sound, if the user asked for one.
///
/// Never fails and never blocks the caller: this runs on the capture path, and
/// a capture that has already been stored must not be jeopardised by an audio
/// device.
pub fn on_capture(state: &AppState, saved: bool) {
    if should_play(
        state.settings.get().sound_on_copy,
        state.backend_name(),
        cfg!(any(target_os = "macos", target_os = "windows")),
        saved,
    ) {
        copypaste_feedback::play();
    }
}

/// The whole decision, as a pure function, so it is testable on any host.
///
/// Three conditions and each is a separate failure if it is dropped: without
/// the setting the app pings on every copy, without the backend check `cargo
/// test` spawns a player per test, and without the platform check an unshipped
/// development host has no system acknowledgement adapter.
fn should_play(enabled: bool, backend_name: &str, supported: bool, saved: bool) -> bool {
    enabled && supported && saved && !backend_name.starts_with(FAKE_BACKEND_PREFIX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_plays_unless_the_user_asked() {
        assert!(!should_play(false, "nspasteboard", true, true));
        assert!(should_play(true, "nspasteboard", true, true));
        assert!(!should_play(true, "nspasteboard", true, false));
    }

    /// Manifest 01 §3.23. Every `cargo test` and every demo script runs on the
    /// fake backend, and a player process per capture there is the "sound spam
    /// in CI" the rule was written about.
    #[test]
    fn the_fake_backend_is_silent_even_when_the_setting_is_on() {
        assert!(!should_play(true, "fake-memory", true, true));
        assert!(!should_play(true, "fake-file", true, true));
    }

    #[test]
    fn nothing_is_spawned_without_a_shipped_platform_adapter() {
        assert!(!should_play(true, "nspasteboard", false, true));
    }

    /// The gate must hold on this host, whatever the settings say, because
    /// `play` would otherwise try to execute a binary that is not here.
    #[test]
    fn the_live_gate_agrees_with_the_platform_this_is_built_for() {
        let supported = cfg!(any(target_os = "macos", target_os = "windows"));
        let allowed = should_play(true, "nspasteboard", supported, true);
        assert_eq!(allowed, supported);
    }
}
