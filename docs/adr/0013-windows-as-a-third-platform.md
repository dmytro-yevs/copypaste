# ADR-0013 — Windows as a third platform

**Status:** accepted · 2026-08-07 · **codified in AGENTS.md rule 7**
**Scope:** that Windows is a shipped target, and the platform boundaries that
must be maintained and qualified.

## Decision

Windows ships, on the same footing as macOS and Android. AGENTS.md rule 7
requires a dependency to work on all three, or sit
behind a platform cfg with **every** other side implemented.

**Linux desktop is still not a shipped target.** It stays a test surface —
`browser-webkitgtk.yml` drives the app through WebKitGTK, and that is the whole
of its purpose. [ADR-0021](0021-accept-the-glib-advisory-as-unshipped.md) rests
its acceptance of the `glib 0.18.5` advisory on that distinction: the GTK
stack is unshipped, so the alert is not a shipped exposure. Windows becoming
shippable does not promote Linux.

## Consequences

The implementation is organized behind platform seams. These source references
describe current code, not native qualification of a release artifact.

**IPC transport.** `copypaste-ipc::transport` owns the shared stream/listener
surface: Unix uses a `0600` socket and Windows uses a named pipe. The daemon's
Windows binder supplies the pipe access list and singleton policy; clients use
the same transport surface rather than a parallel Windows protocol.

**The socket's security properties do not port.** `bind_owner_only` reaches
`0600` by binding inside a `0700` staging directory and renaming into place
(security review F-9), and `BindLock` holds an exclusive `flock(2)` across
probe → remove → bind (`CopyPaste-ah1m`). A named pipe has neither: "only this
user" has to be an explicit ACL set at creation, and single-instance has to be
something other than `flock`. The socket is the only authentication boundary
(manifest 04 I14), so a transport that gets this wrong is an open daemon. Rule
4 carries too — a pipe name discloses the username exactly as the socket path
does, so it must not appear in a user-facing error.

**Secret storage.** `crypto::keystore` selects a Windows DPAPI backend, keeping
the sealed blob in the data directory. The file-backed store is limited to
non-shipped targets. Fail-closed behaviour remains unchanged: no entry may mint
a secret, while an unusable entry must not.

**Clipboard capture.** `clipboard::ClipboardSource` is the seam. The Windows
implementation uses the Windows change cursor and Windows opt-out formats;
native capture evidence remains separately required.

**Shell, packaging and updates.** Windows-specific service, hotkey, updater,
and packaging code is maintained under the Tauri and release owners. The
installer, Authenticode, updater-signature, and publication decisions are
canonicalized by [ADR-0020](0020-windows-distribution-and-update-signing.md),
not duplicated here.

**The source-application icon resolves via App Paths registry on Windows.**
`get_source_app_icon` looks up the executable path transiently from the image
name using `App Paths` or `System32`, extracts the shell icon with
`SHGetFileInfoW`, and discards the path — the path never reaches storage or
logs (I-9). Applications not in the registry return no icon; rows keep their
semantic content-type icon in that case.

## Qualification

Windows remains a shipped target only when same-commit installed-product and
native evidence is available. CI and source tests are useful checks but do not
replace that evidence.
