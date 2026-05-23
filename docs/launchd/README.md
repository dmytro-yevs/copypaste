# CopyPaste — macOS LaunchAgent

Run `copypaste-daemon` automatically at user login via `launchd`.

## What this installs

- **Plist:** `~/Library/LaunchAgents/com.copypaste.daemon.plist`
- **Label:** `com.copypaste.daemon`
- **Scope:** per-user (`gui/<uid>`), not system-wide
- **Binary:** `/Applications/CopyPaste.app/Contents/MacOS/copypaste-daemon`
- **Logs:** `~/Library/Logs/CopyPaste/daemon.out.log` and `daemon.err.log`

The agent is configured with:

- `RunAtLoad=true` — starts on login
- `KeepAlive=true` — restarts on crash
- `ThrottleInterval=10` — minimum 10s between restarts (avoids tight crash loops)
- `ProcessType=Interactive` — full UI session access (pasteboard, notifications)

## Install

Prerequisite: `CopyPaste.app` must be installed in `/Applications/`.

```bash
bash scripts/launchd/install-agent.sh
```

The script will:

1. Create `~/Library/Logs/CopyPaste/` and `~/Library/LaunchAgents/`
2. Copy the plist (substituting `$HOME` for the log paths)
3. Run `plutil -lint` to validate
4. `launchctl bootout` any existing instance
5. `launchctl bootstrap gui/<uid> <plist>`
6. `launchctl enable gui/<uid>/com.copypaste.daemon`
7. `launchctl kickstart -k gui/<uid>/com.copypaste.daemon`

## Uninstall

```bash
bash scripts/launchd/uninstall-agent.sh
```

Boots out the agent and removes the plist. Log files in `~/Library/Logs/CopyPaste/` are preserved.

## Verify

Check it's loaded:

```bash
launchctl list | grep com.copypaste
```

Get detailed status:

```bash
launchctl print gui/$(id -u)/com.copypaste.daemon
```

Tail logs:

```bash
tail -f ~/Library/Logs/CopyPaste/daemon.out.log
tail -f ~/Library/Logs/CopyPaste/daemon.err.log
```

## Troubleshooting

### Agent fails to load

Symptom: `launchctl bootstrap` returns `Bootstrap failed: 5: Input/output error`.

Causes:

1. The plist already loaded — `bootout` first, then `bootstrap`
2. The binary at `ProgramArguments[0]` does not exist or is not executable
3. Plist is malformed — run `plutil -lint ~/Library/LaunchAgents/com.copypaste.daemon.plist`

### Agent loaded but does not run

Symptom: `launchctl list | grep copypaste` shows status `-` (never run) or non-zero exit code.

Check:

```bash
launchctl print gui/$(id -u)/com.copypaste.daemon | grep -E "(state|last exit)"
cat ~/Library/Logs/CopyPaste/daemon.err.log
```

Common causes:

- Binary missing — install `CopyPaste.app` to `/Applications/`
- Code-signing or Gatekeeper rejection — first launch via Finder once to approve
- Required permissions (Accessibility, Input Monitoring) not granted

### Restart after binary update

```bash
launchctl kickstart -k gui/$(id -u)/com.copypaste.daemon
```

### Disable temporarily without uninstalling

```bash
launchctl disable gui/$(id -u)/com.copypaste.daemon
launchctl bootout gui/$(id -u)/com.copypaste.daemon
```

Re-enable:

```bash
launchctl enable gui/$(id -u)/com.copypaste.daemon
launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/com.copypaste.daemon.plist
```

## Notes

- This is a **LaunchAgent**, not a **LaunchDaemon**: it runs in the user's GUI session, which is required for clipboard / pasteboard access. A LaunchDaemon (system-wide, runs as root, no UI session) cannot read `NSPasteboard`.
- The plist template in `packaging/macos/com.copypaste.daemon.plist` uses the literal placeholder `/Users/USERNAME` for log paths; the install script substitutes the actual `$HOME` at install time.
- The Rust side of LaunchAgent management lives in `crates/copypaste-daemon/src/launchd.rs` — this packaging is for manual / installer-driven setups.
