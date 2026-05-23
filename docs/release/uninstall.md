# Uninstalling CopyPaste (macOS)

This page covers complete removal of CopyPaste — daemon, app bundle, data, and
Keychain entries — for both the **Homebrew cask** install path and the
**manual `install.sh` / DMG** install path.

## TL;DR

```bash
# Complete removal, interactive prompts for data:
curl -fsSL https://raw.githubusercontent.com/USER/CopyPaste/main/scripts/release/uninstall.sh | bash

# Or from a local checkout:
./scripts/release/uninstall.sh

# Keep clipboard history + settings, remove everything else:
./scripts/release/uninstall.sh --keep-data

# Preview what would happen without changing anything:
./scripts/release/uninstall.sh --dry-run

# Non-interactive (CI / scripted):
./scripts/release/uninstall.sh --force
```

## What gets removed

| Item | Path | Removed by default? |
| --- | --- | --- |
| LaunchAgent (autostart) | `~/Library/LaunchAgents/com.copypaste.daemon.plist` | Yes |
| Running daemon | `launchctl bootout gui/<uid>/com.copypaste.daemon` | Yes |
| App bundle (manual install) | `/Applications/CopyPaste.app` | Yes |
| App bundle (Homebrew) | `brew uninstall --cask copypaste` | Yes |
| Optional CLI symlinks | `/usr/local/bin/copypaste`, `/usr/local/bin/copypaste-daemon` | Yes (if they point into CopyPaste) |
| Clipboard DB + settings | `~/Library/Application Support/CopyPaste` | Yes, with prompt — skipped by `--keep-data` |
| Cache | `~/Library/Caches/CopyPaste` | Yes, with prompt — skipped by `--keep-data` |
| Logs | `~/Library/Logs/CopyPaste` | Yes, with prompt — skipped by `--keep-data` |
| Keychain master key (`copypaste-master-key`) | Login Keychain | **No — manual** (see below) |

## Flags

| Flag | Behaviour |
| --- | --- |
| `--keep-data` | Skip removal of `Application Support`, `Caches`, `Logs`. Binary + LaunchAgent still removed. |
| `--dry-run` | Print every command without executing. Combine with any other flag to preview. |
| `--force` | Suppress all confirmation prompts (implies "yes" to data removal). For CI and scripted uninstall. |
| `--help` | Print the embedded help text. |

`--dry-run` and `--force` can be combined (`--dry-run --force` shows the
full non-interactive plan).

## Install method detection

`uninstall.sh` detects how CopyPaste was installed:

- **Homebrew cask present** (`brew list --cask | grep copypaste`) →
  runs `brew uninstall --cask copypaste`. Brew owns the bundle; removing
  `/Applications/CopyPaste.app` manually while the cask record exists would
  leave brew in an inconsistent state.
- **Manual `install.sh` / DMG** → removes `/Applications/CopyPaste.app`
  directly.

Either way, the optional `/usr/local/bin/copypaste{,-daemon}` symlinks (created
by the post-install hint from `install.sh`) are removed if they point into a
CopyPaste path. `sudo` is only invoked when the parent directory is not
writable by your user (Intel `/usr/local/bin` typically requires it; Apple
Silicon `/opt/homebrew/bin` does not).

## LaunchAgent-only removal

If you just want to stop the daemon and prevent autostart but **keep the app
installed**, use the standalone helper:

```bash
./scripts/release/uninstall-launchd.sh
```

This runs `launchctl bootout` on the agent (modern API, falls back to legacy
`unload` for safety) and removes the plist. No app bundle, data, or Keychain
changes.

`uninstall.sh` delegates to this script as its first step — there is one
source of truth for the LaunchAgent removal logic.

## Keychain entries (manual)

CopyPaste stores a 32-byte master key in your **login Keychain** under the
service name `copypaste-master-key`. The daemon binary also stores a
device-secret key under the service `com.copypaste.daemon`.

We do **not** delete Keychain entries automatically, for two reasons:

1. **Recovery.** Deleting the master key permanently destroys access to any
   encrypted clipboard DB you may have kept (e.g. via `--keep-data`, or in a
   backup).
2. **Trust prompts.** macOS prompts for your login password before any process
   can delete Keychain entries; that prompt is jarring inside an uninstall
   script and easy to misread as suspicious.

To remove them yourself:

**GUI**

1. Open **Keychain Access.app** (Applications → Utilities).
2. Select the **login** keychain in the sidebar.
3. Search for `copypaste`.
4. Delete the entries `copypaste-master-key` and any under
   `com.copypaste.daemon`.

**CLI**

```bash
security delete-generic-password -s 'copypaste-master-key'
security delete-generic-password -s 'com.copypaste.daemon'
```

Each invocation deletes one matching entry; re-run until `security` reports
"could not be found" (idempotent).

## Idempotency

Every step in `uninstall.sh` and `uninstall-launchd.sh` is safe to re-run.
Already-removed paths, unloaded agents, and absent brew casks are reported
("already removed") rather than failed. You can interrupt the script at any
point and re-run it to pick up where you left off.

## What stays behind

After a default uninstall (no `--keep-data`), the following remains:

- Keychain entries (`copypaste-master-key`, `com.copypaste.daemon`) — see above.
- Empty parent directories (e.g. `~/Library/LaunchAgents` if you had no other
  agents) — left alone because they are shared with the OS.
- macOS Privacy & Security grants (e.g. Accessibility, Input Monitoring) for
  removed bundles — macOS clears these automatically when the bundle is gone
  and re-asks on next install. No manual cleanup required.
- Homebrew **tap** (`USER/homebrew-copypaste`) — if you installed via the tap,
  `brew uninstall --cask` removes the cask but keeps the tap registered.
  Remove it explicitly with `brew untap USER/copypaste` if you want to detach
  fully.

## Troubleshooting

**"launchctl: Bootout failed: 5: Input/output error"** — harmless; means the
daemon was already stopped. The script swallows this error.

**"Operation not permitted" deleting `~/Library/...`** — usually a stale
"Full Disk Access" grant blocking your shell. Either grant Terminal Full Disk
Access in System Settings → Privacy & Security, or `sudo rm -rf` the listed
paths.

**Brew uninstall fails with "Error: Cask 'copypaste' is not installed"** —
the cask record was already removed (perhaps a prior manual `rm -rf` of the
bundle confused brew). Run `brew cleanup` and the manual install path
fallback inside `uninstall.sh` will catch any leftover bundle.

**Daemon respawns after uninstall** — you almost certainly have a second
LaunchAgent under a different label. Inspect with:

```bash
launchctl list | grep -i copypaste
```

and bootout any matches with `launchctl bootout gui/$(id -u)/<label>`.

## See also

- [`install.sh`](../../scripts/release/install.sh) — the installer being undone.
- [`brew-tap-setup.md`](./brew-tap-setup.md) — Homebrew tap and cask details.
