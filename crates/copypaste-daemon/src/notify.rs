//! Telling the user something was captured.
//!
//! Parity finding 18. A background capture is otherwise completely invisible:
//! the window is hidden, the tray icon does not move, and a user who cannot
//! tell whether the app is working has no way to find out. v1 fired a sound and
//! a rich notification, gated on `sound_on_copy` and `notify_on_copy`, and both
//! switches were lost with the config system.
//!
//! # The daemon plays the sound and does not post the notification
//!
//! Not a division of labour anyone chose — it is what macOS permits. Posting to
//! the notification centre needs `UNUserNotificationCenter`, which needs an
//! application bundle, and the daemon is a bare executable started by launchd.
//! The app has the bundle, so the app posts it: the daemon sets
//! [`copypaste_ipc::EventData::captured`] on the change event and reads
//! `notify_on_copy` back out through `get_config`. Manifest 06 §3.6 describes
//! that split already ("respecting the daemon's `notify_on_copy`").
//!
//! The sound has no such constraint, so it stays here, next to the capture that
//! caused it. Putting it in the app instead would mean a copy makes no sound
//! while the app is not running, which is exactly the case the feature is for.
//!
//! # Why a subprocess
//!
//! `afplay` is macOS's own player and the sounds are the system's own. The
//! alternative is an audio crate — `rodio` and friends — which decodes and
//! mixes audio and pulls a device backend in with it (ALSA on Linux, and a
//! question with no good answer on Android). That is a large dependency for one
//! ping, and it does not do the thing actually wanted, which is "play the
//! system's alert sound the way every other Mac app does". Manifest 01 §3.23
//! records that v1 also used a player process, and records the two ways it went
//! wrong: sound spam in test environments, and zombie processes. Both are
//! handled below.

#[cfg(target_os = "macos")]
use std::process::{Command, Stdio};

use crate::AppState;

/// macOS's player, and one of its stock alert sounds.
///
/// A fixed absolute path under `/System`, so it is neither user-supplied nor
/// user-identifying, and it never appears in a message that reaches a client
/// (CLAUDE.md rule 4).
#[cfg(target_os = "macos")]
const PLAYER: &str = "/usr/bin/afplay";
#[cfg(target_os = "macos")]
const SOUND: &str = "/System/Library/Sounds/Pop.aiff";

/// The prefix every fake clipboard backend's name carries (`fake-memory`,
/// `fake-file`).
///
/// Manifest 01 §3.23 asks for the sound to be suppressed in test environments.
/// v1's test was "is the key ephemeral"; v2 has a better one, because the fake
/// clipboard is *exactly* the set of runs that are not a real user copying
/// something — every `cargo test`, every demo script, and every non-macOS host.
const FAKE_BACKEND_PREFIX: &str = "fake";

/// Something was captured. Play the sound, if the user asked for one.
///
/// Never fails and never blocks the caller: this runs on the capture path, and
/// a capture that has already been stored must not be jeopardised by an audio
/// device.
pub fn on_capture(state: &AppState) {
    if should_play(
        state.settings.get().sound_on_copy,
        state.backend_name(),
        cfg!(target_os = "macos"),
    ) {
        play();
    }
}

/// The whole decision, as a pure function, so it is testable on any host.
///
/// Three conditions and each is a separate failure if it is dropped: without
/// the setting the app pings on every copy, without the backend check `cargo
/// test` spawns a player per test, and without the platform check a Linux
/// developer machine tries to run a binary that is not there.
fn should_play(enabled: bool, backend_name: &str, is_macos: bool) -> bool {
    enabled && is_macos && !backend_name.starts_with(FAKE_BACKEND_PREFIX)
}

#[cfg(target_os = "macos")]
fn play() {
    // Detached from this process's streams: `afplay` writes nothing useful and
    // inheriting the daemon's stderr would interleave with the log.
    let spawned = Command::new(PLAYER)
        .arg(SOUND)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn();

    match spawned {
        Ok(mut child) => {
            // Manifest 01 §3.23 and T-80: the player must be reaped. A child
            // that is never waited on stays in the process table as a zombie,
            // and at one copy per capture that is unbounded growth.
            //
            // Waited on a detached thread rather than here: the sound is
            // roughly 300 ms and the caller is the capture path, which is
            // holding nothing but should not be held up either.
            std::thread::spawn(move || {
                if let Err(e) = child.wait() {
                    // No path, no content — the failure is the wait, not the
                    // sound.
                    tracing::debug!(error = %e, "could not reap the capture sound player");
                }
            });
        }
        // Never fatal: a machine with no audio, or a stripped system, still
        // captures. Reported at `warn` once per capture is noisy, so `debug`.
        Err(e) => tracing::debug!(error = %e, "could not play the capture sound"),
    }
}

#[cfg(not(target_os = "macos"))]
fn play() {
    // Unreachable — `should_play` is false off macOS — but present so the
    // module compiles and is type-checked on the host it is developed on.
    tracing::warn!("the capture sound is macOS-only");
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
    fn nothing_is_spawned_off_macos() {
        assert!(!should_play(true, "nspasteboard", false));
    }

    /// The gate must hold on this host, whatever the settings say, because
    /// `play` would otherwise try to execute a binary that is not here.
    #[test]
    fn the_live_gate_agrees_with_the_platform_this_is_built_for() {
        let allowed = should_play(true, "nspasteboard", cfg!(target_os = "macos"));
        assert_eq!(allowed, cfg!(target_os = "macos"));
    }
}
