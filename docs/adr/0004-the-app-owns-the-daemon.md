# ADR-0004 — The app owns the daemon's lifetime

**Status:** accepted · 2026-07-30
**Scope:** who starts, stops and supervises `copypaste-daemon` on macOS.
**Supersedes nothing.** This decision applies to the distribution in ADR-0001.

## Decision

**On macOS the CopyPaste app is authoritative for the daemon's lifetime.**
Opening the app starts the daemon; quitting the app stops it. The app installs
no launchd agent and does not use `brew services`.

The daemon it starts is the one inside its own bundle —
`CopyPaste.app/Contents/MacOS/copypaste-daemon`, which
`scripts/release/build-macos-app.sh` already injects and signs. The app
therefore never depends on the CLI formula being installed.

`brew services start copypaste-cli` stays as the supervisor for people who
install only the formula. The two are alternatives, not layers, and the
formula's caveats already say to run one or the other.

## Why one owner and not two

Two supervisors over one Unix socket is the failure this decision exists to
avoid. launchd's `KeepAlive` restarts a daemon the app has just stopped, and the
app then finds a daemon it does not own holding the socket it wants — which is
the "stale daemon after an upgrade" symptom the parity audit records (finding 2),
arriving by a second route.

`tauri-plugin-autostart` is a different question and is unaffected. It decides
whether the **app** launches at login. Because the app owns the daemon, turning
it on is also what makes the daemon run at login — one switch, one owner.

## Ownership is per-process, and adoption is read-only

The app stops **only a daemon it started itself**, tracked as a child process
handle for the lifetime of this app process. A daemon it merely found running is
adopted read-only: used, reported, never killed.

That asymmetry is deliberate. A daemon the app did not start belongs to
something else — `brew services`, a terminal, another copy of the app — and an
app that kills processes it did not start is a worse failure than a duplicate
one. It also means force-quitting the app orphans its daemon, which the next
launch detects by version rather than by guessing.

## The four states, and what each does

| Situation | Detected by | What happens |
|---|---|---|
| Not installed | no daemon binary beside our own executable | "This build doesn't include the background service." No start button; nothing to start. |
| Installed, not running | `status` is unreachable, binary present | Start it, wait for `status` to answer, then refresh. |
| Running, different version | `status.version` ≠ the app's version | Reported as its own state with a Restart offer. See "what is still missing" below. |
| Running from a previous install | same as above | Same path. The version is the signal: an orphan from an older bundle answers with an older version. |

A daemon answering with the same version is adopted silently, whoever started
it. That is the case after `brew services` started it, and it is correct: it is
the same code.

## Stopping through the IPC contract

`Method::Shutdown` acknowledges the request before it signals the daemon's
shared shutdown channel. The app uses it for an owned daemon on exit and for an
adopted, mismatched daemon before restart. It adds no authority: the socket is
`0600`, so any client that can call it can already delete the entire history.

`Child::kill` remains the bounded fallback when an owned daemon cannot answer
or does not finish stopping. The app never applies that fallback to an adopted
process because it has no child handle for one.

## Rejected alternatives

**A launchd agent installed by the app.** It survives the app being force-quit
and restarts on crash — real benefits. It also means the app must write a plist
into the user's `LaunchAgents`, keep it in step with the bundle's location
across `brew upgrade`, and remove it on uninstall. The cask's `zap` cannot be
relied on for the last of those (`brew uninstall` without `--zap` leaves it),
so the failure mode is an agent pointing at a bundle that no longer exists,
respawning nothing, forever. The formula delegates service ownership to
Homebrew's `service` DSL, which is where that job belongs.

**`tauri-plugin-shell`'s sidecar mechanism.** It is the maintained way to ship a
companion binary, and it was the first thing checked (AGENTS.md rule 1). Two
reasons against: it expects the binary to be named with a target triple suffix
and registered as `externalBin`, which conflicts with the injection the release
script already does and with the per-binary `--identifier` signing that goes
with it; and it grants the WebView a general command-execution capability
through its ACL. Spawning one known binary from Rust needs neither. The frontend
gets a `start_service` command, not a shell.

**Leaving the daemon running after the app quits.** Tempting for a clipboard
manager — history would keep recording. But quitting is already an explicit
gesture and the only one: the window close button hides (INV-36), so the tray's
"Quit CopyPaste" is the sole exit. A Quit that leaves a background process
recording the clipboard is a Quit that did not quit.

## Consequences

- The offline screen's terminal instruction is gone. It stays only as the
  fallback for a build with no bundled daemon, where it is true.
- `BackendError::Unreachable` no longer tells the user to run a command. The
  screen has a button.
- Nothing here has been run on macOS. The spawn path, the readiness wait and
  the stop-on-exit hook are exercised by tests on Linux against a fake binary;
  whether `Contents/MacOS/copypaste-daemon` starts correctly from inside a
  quarantined, ad-hoc-signed bundle is unverified and is the second thing to
  check on a Mac after ADR-0002's hotkey question.
