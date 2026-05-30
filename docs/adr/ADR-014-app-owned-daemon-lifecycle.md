# ADR-014: Desktop app owns the daemon lifecycle

- Status: Accepted
- Date: 2026-05-30
- Supersedes: the prior "always-on background daemon" model (LaunchAgent with
  `RunAtLoad=true` + `KeepAlive`).

## Context

The original design ran `copypaste-daemon` as an always-on per-user
**LaunchAgent** (`packaging/macos/com.copypaste.daemon.plist`, installed into
`~/Library/LaunchAgents/`). The daemon was started by launchd at login
(`RunAtLoad=true`) and kept alive across crashes (`KeepAlive`).

This produced two product bugs the product owner flagged explicitly:

1. **Opening the app did NOT start the daemon.** The daemon only came up at
   login via `RunAtLoad`. If it was not running, launching the app left the UI
   in a "daemon offline" state.
2. **Quitting the app did NOT stop the daemon.** The daemon was fully decoupled
   from the app and survived after the app was killed/quit — the opposite of
   what the new model wants.

A secondary, related bug: after an in-place upgrade the OLD daemon binary kept
running (holding the socket), so the new app talked to a stale daemon until the
next reboot/login.

## Decision

**The desktop app owns the daemon process lifecycle.** Implemented in
`crates/copypaste-ui/src-tauri/src/daemon_lifecycle.rs` and wired into
`lib.rs`:

- **On launch** (`ensure_daemon_running`, called early in Tauri `.setup`):
  boot out any leftover LaunchAgent, stop any pre-existing daemon
  (`pkill -TERM copypaste-daemon`), then spawn the **bundled** daemon
  (`Contents/MacOS/copypaste-daemon`, resolved relative to the running UI exe)
  as a tracked child process. Wait for the IPC socket to come up; on failure,
  log loudly so the existing "daemon offline" UI surfaces it.
- **On full app quit** (`stop_daemon`, called from `RunEvent::Exit`): SIGTERM
  the tracked child for a graceful flush (then SIGKILL as a fallback) and boot
  out any leftover LaunchAgent so launchd cannot resurrect it.
- **Window close ≠ quit**: the main window's `CloseRequested` is intercepted
  (`setup_main_window`) and the window is hidden to the tray instead of closing.
  Only the tray "Quit" item (`app.exit(0)`) or process termination reaches
  `RunEvent::Exit`, so closing the window keeps the tray AND the daemon alive
  (standard macOS menu-bar pattern).

### LaunchAgent reconciliation (requirement C)

We chose **option (i): the app fully manages the daemon as a child process and
does NOT rely on the LaunchAgent for the default install.** An always-on agent
(`RunAtLoad`/`KeepAlive`) would fight "daemon dies when the app quits" — launchd
would relaunch the daemon the instant the app SIGTERMs it.

To stay robust against a leftover loaded agent from a prior install, the app
proactively `launchctl bootout`s `com.copypaste.daemon` on BOTH launch and quit.
`bootout` removes the job from launchd entirely, neutralising `KeepAlive`
regardless of its value.

The plist is **retained but demoted to opt-in/legacy**:
- `scripts/release/install.sh` no longer bootstraps it; it boots out any
  leftover one instead.
- `scripts/release/build-dmg-ci.sh` still ships the template in
  `Contents/Resources/` but its comment now reflects the legacy/opt-in status
  (no longer a hard build failure if absent).
- Power users who want a headless, CLI-managed daemon WITHOUT the desktop app
  can still install it via `scripts/launchd/install-agent.sh` /
  `copypaste daemon install`. They must not run it alongside the app — the app
  boots it out.

## How this fixes the stale-daemon-on-upgrade case

Because `ensure_daemon_running` always (a) boots out the LaunchAgent and
(b) `pkill -TERM`s any running `copypaste-daemon` before spawning the
freshly-installed `Contents/MacOS/copypaste-daemon`, a fresh app launch always
replaces whatever daemon was previously running with the new binary. The old
daemon can no longer linger holding the socket.

## Consequences

- The daemon's lifetime is now coupled to the app's: no app running ⇒ no daemon
  (by design). Headless/CLI-only use is still possible via the opt-in agent.
- `daemon_lifecycle` deliberately avoids the `libc` crate (uses a one-line
  `extern "C"` `getuid` shim and shells out to `kill`/`pkill`/`launchctl`).
- Compatible with in-flight work (Restart-daemon command, `status` build-version
  field, daemon socket-takeover): the hooks are small and localized to
  `lib.rs` setup/run and a new module; socket-takeover in the daemon makes the
  `pkill` step belt-and-suspenders rather than strictly required.
