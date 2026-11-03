//! Sound feedback for successful captures and clipboard writes.
//!
//! The daemon owns copy sound because it knows whether capture succeeded or
//! an explicit or received item reached the clipboard. The bundled app owns
//! notifications because macOS notification centre
//! requires an app bundle and Windows toast identity belongs to the installed
//! executable. The two settings and permission paths remain independent.

use crate::AppState;

/// The prefix every fake clipboard backend's name carries (`fake-memory`,
/// `fake-file`).
///
/// Manifest 01 §3.23 asks for the sound to be suppressed in test environments.
/// The fake clipboard is exactly the set of runs that are not a real user copying
/// something — every `cargo test` and every demo script.
const FAKE_BACKEND_PREFIX: &str = "fake";

/// A capture or clipboard write succeeded. Play the requested sound, including recopy.
///
/// Never fails and never blocks the caller: this runs on the capture path, and
/// a capture that has already been stored must not be jeopardised by an audio
/// device.
pub fn on_copy(state: &AppState) {
    play_if(state.settings.get().sound_on_copy, state.backend_name());
}

/// [`on_copy`] for a caller holding the setting and backend name directly.
pub fn play_if(sound_on_copy: bool, backend_name: &str) {
    #[cfg(test)]
    COPY_REQUESTS.with(|count| count.set(count.get().map(|count| count + 1)));
    if should_play(
        sound_on_copy,
        backend_name,
        cfg!(any(target_os = "macos", target_os = "windows")),
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
fn should_play(enabled: bool, backend_name: &str, supported: bool) -> bool {
    enabled && supported && !backend_name.starts_with(FAKE_BACKEND_PREFIX)
}

#[cfg(test)]
thread_local! {
    static COPY_REQUESTS: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
}

/// Observe feedback requests without playing audio from a fake clipboard.
#[cfg(test)]
pub(crate) fn count_requests(action: impl FnOnce()) -> usize {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            COPY_REQUESTS.with(|count| count.set(None));
        }
    }
    COPY_REQUESTS.with(|count| assert!(count.replace(Some(0)).is_none()));
    let _reset = Reset;
    action();
    COPY_REQUESTS.with(|count| count.get().unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_plays_unless_the_user_asked() {
        assert!(!should_play(false, "nspasteboard", true));
        assert!(should_play(true, "nspasteboard", true));
    }

    /// Manifest 01 §3.23. Every `cargo test` and every demo script runs on the
    /// fake backend, and a player process per capture there is the "sound spam
    /// in CI" the rule was written about.
    #[test]
    fn the_fake_backend_is_silent_even_when_the_setting_is_on() {
        assert!(!should_play(true, "fake-memory", true));
        assert!(!should_play(true, "fake-file", true));
    }

    #[test]
    fn nothing_is_spawned_without_a_shipped_platform_adapter() {
        assert!(!should_play(true, "nspasteboard", false));
    }

    /// The gate must hold on this host, whatever the settings say, because
    /// `play` would otherwise try to execute a binary that is not here.
    #[test]
    fn the_live_gate_agrees_with_the_platform_this_is_built_for() {
        let supported = cfg!(any(target_os = "macos", target_os = "windows"));
        let allowed = should_play(true, "nspasteboard", supported);
        assert_eq!(allowed, supported);
    }
}
