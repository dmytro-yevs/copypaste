# Known Issues — CopyPaste

Current as of v0.3.0. Items marked **tracked** have an open beads issue.

---

## macOS — Intel (x86_64) runs via Rosetta 2 only

Release builds are `aarch64-apple-darwin` only; no universal (`lipo`) DMG is
published by CI. Intel Macs will run the arm64 binary under Rosetta 2
automatically. Performance is equivalent to native for typical clipboard
workloads.

**Workaround:** none required — Rosetta 2 handles the translation transparently.
To build a native x86_64 binary locally: `cargo build --target x86_64-apple-darwin -p copypaste-daemon`.

---

## macOS — minimum version is Sonoma (14.0)

The Homebrew Cask formula (`Casks/copypaste.rb`) declares `depends_on macos: :sonoma`.
macOS 13 (Ventura) and earlier are not supported.

---

## Linux — daemon only, no desktop UI

`copypaste-daemon` builds and runs on Linux (x86_64, arm64). The Tauri desktop
UI (`copypaste-ui`) does not build for Linux — use the CLI (`copypaste-cli`)
instead. A systemd user unit is provided in `contrib/systemd/`.

**Status:** no Linux UI is planned for the current roadmap.

---

## Windows — frozen as of 2026-05-23

Windows support is frozen. See
[`docs/adr/ADR-012-windows-frozen-homebrew-only.md`](adr/ADR-012-windows-frozen-homebrew-only.md)
for the rationale. Windows users can run the daemon under WSL2. No ETA for
unfreezing.

---

## Android — requires a paired macOS desktop

The Android app (`android/`) syncs clipboard items through the daemon running on
a paired macOS machine. P2P (LAN/mTLS), relay, and cloud sync all require an
active pairing. There is no standalone Android clipboard history when no desktop
is paired.

**Workaround:** pair the phone with a macOS desktop using the QR pairing flow,
or configure a self-hosted relay server so both devices poll independently.

---

## Relay TTL is ≥ 24 hours

Items pushed to the relay server persist in the receiver's inbox for at least
`sync_ttl_secs` (default 86 400 s = 24 h). Encrypted items accumulate until
the receiving daemon polls and acknowledges them. If a device is offline for
more than the TTL the oldest items are silently pruned (oldest-first once the
500-item quota is reached).

**Workaround:** keep `sync_ttl_secs` ≥ the expected longest offline window. For
highly sensitive deployments, prefer P2P (LAN, direct) which delivers items
immediately and leaves nothing on a third-party server.

---

## Sensitive items DO sync (encrypted)

Items detected as sensitive (API keys, tokens, SSH keys, credit card numbers,
etc.) are encrypted and synced via relay/cloud/P2P exactly like non-sensitive
items. Sensitivity is re-detected on the receiving device from the decrypted
plaintext. Items never leave any device in plaintext form.

The `docs/relay-api.md` note that "sensitive items are never uploaded" was
inaccurate and has been corrected. If you require sensitive items to stay on one
device, disable all sync transports for that device.

---

## mDNS broadcasts device name in cleartext on the LAN

mDNS-SD service discovery (`copypaste-p2p`) announces the device name and a
stable device UUID in cleartext TXT records — this is inherent to the mDNS
protocol (similar to Bonjour). Any device on the local network can see which
machines are running CopyPaste and their names.

**Mitigation:** clip content is never included in mDNS records. Pairing still
requires mutual PAKE + SAS confirmation; mDNS enumeration cannot bypass it.
A rotating discovery ID is tracked for a future release.

---

## Cloud/relay device revocation (sync-key rotation) is not yet implemented

Revoking a paired device from relay or cloud sync requires rotating the shared
sync key and re-registering all remaining devices. This flow is not yet
implemented. To revoke access, change your relay bearer token or Supabase
credentials and re-pair all legitimate devices.
