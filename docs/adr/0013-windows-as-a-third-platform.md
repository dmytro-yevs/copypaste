# ADR-0013 — Windows as a third platform

**Status:** accepted · 2026-08-07 · **amends CLAUDE.md rule 7**
**Scope:** that Windows is a shipped target, and what the tree already says it
will cost. No Windows code exists yet.

## Decision

Windows ships, on the same footing as macOS and Android. CLAUDE.md rule 7 is
amended in the same commit: a dependency must now work on all three, or sit
behind a platform cfg with **every** other side implemented.

**Linux desktop is still not a shipped target.** It stays a test surface —
`browser-webkitgtk.yml` drives the app through WebKitGTK, and that is the whole
of its purpose. [ADR-0014](0014-accept-the-glib-advisory-as-unshipped.md)'s diagnosis of the `glib 0.18.5` advisory rests on
that distinction: the GTK stack is unshipped, so the alert is not a shipped
exposure. Windows becoming shippable does not promote Linux.

This lands before any Windows code because rule 2 forbids building something
that contradicts the specification.

## Consequences

Each is in the tree today, named so the work is not rediscovered.

**IPC transport.** `crates/copypaste-daemon/src/server/listener.rs`,
`crates/copypaste-cli/src/client.rs` and
`crates/copypaste-ui/src-tauri/src/backend/daemon.rs` are
`UnixListener`/`UnixStream`, and there is no `cfg(windows)` anywhere in the
tree. Windows needs a named-pipe transport behind a seam — one declaration both
sides answer to, for the reason ADR-0003 gives: two `cfg` modules with matching
names compile while disagreeing.

**The socket's security properties do not port.** `bind_owner_only` reaches
`0600` by binding inside a `0700` staging directory and renaming into place
(security review F-9), and `BindLock` holds an exclusive `flock(2)` across
probe → remove → bind (`CopyPaste-ah1m`). A named pipe has neither: "only this
user" has to be an explicit ACL set at creation, and single-instance has to be
something other than `flock`. The socket is the only authentication boundary
(manifest 04 I14), so a transport that gets this wrong is an open daemon. Rule
4 carries too — a pipe name discloses the username exactly as the socket path
does, so it must not appear in a user-facing error.

**Secret storage.** `crypto::keystore` selects on the target: Keychain via
`security-framework` on macOS, the Android Keystore on Android, and a `0600`
file on everything else. That last arm is not a fallback Windows can take —
`file.rs` is written on `std::os::unix::fs` so it will not compile there, and it
is a development posture that ADR-0003 already refused to ship. Windows needs
its own backend on DPAPI or the Credential Manager. Fail-closed (rule 4) applies
unchanged, and so does I-20: no entry authorises a mint, an unusable entry does
not.

**Clipboard capture.** The `clipboard::ClipboardSource` trait is the seam and it
holds. `clipboard/macos.rs` exists because no crate exposes `changeCount`
together with the three `org.nspasteboard.*` opt-out markers. Windows needs its
own implementation behind that trait, with its own change detection and its own
self-write suppression. What stands in for the `org.nspasteboard.*` contract on
Windows is an open question, not a port.

**Build.** `bundled-sqlcipher` resolves to `-lcrypto` against the build
machine's OpenSSL on any non-Apple target, which on Windows means `OPENSSL_DIR`
(ADR-0007). Android answered the same problem by vendoring, through
`bundled-sqlcipher-vendored-openssl` in a target-specific dependency; that
mechanism is the precedent to weigh, against the costs ADR-0007 already
enumerates.

**Shell and packaging.** Tray, popover, global hotkey, launch-at-login and
notifications are written against Tauri plugins for macOS, and
`shell::hotkey::is_permission_free` encodes a macOS TCC constraint that means
nothing on Windows. Windows needs its own shell wiring, an installer, a signing
story, and a CI job — every runner in every workflow today is `ubuntu-*` or
`macos-14`. Until that job exists, Windows code is unverified in the sense the
README's middle column uses.

## Not decided

- Installer format — MSI, NSIS or MSIX.
- Signing: whether an Authenticode certificate is bought, and what ADR-0001's
  ad-hoc posture becomes against SmartScreen.
- Whether Windows ships in the same version stream as macOS or on its own until
  a runner has exercised it.
- The daemon's lifecycle — a Windows service, a run-key process, or spawned by
  the app, which ADR-0003 left open on macOS as well.

None of these blocks starting. Each belongs to the ADR that takes it.
