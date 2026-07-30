# Security

## Reporting a vulnerability

**Do not open a public GitHub issue for anything exploitable.** Open a private
security advisory on GitHub instead.

Useful in a report: the affected component (`core`, `daemon`, `cli`, `p2p`,
`cloud`), what an attacker must already have, and what they gain.

---

## Status

**v2 is alpha and has not been audited.** Parts of it have never executed on a
platform we ship to — [Unverified](#unverified) is the section to read before
trusting anything else here.

An adversarial source review of this branch is at
[`docs/rewrite/security-review.md`](docs/rewrite/security-review.md): fourteen
findings, two High, several still open. It is dated 2026-07-30; each finding
carries its own status.

This document describes v2 only. v0.4.1 survives on
`archive/v0.4.1-pre-rewrite` and differs in ways that matter — most sharply, it
synced sensitive items and v2 does not.

## At rest

- **Content** is sealed with XChaCha20-Poly1305, with the item id bound as
  associated data: a row's ciphertext cannot be moved to another row, it fails
  authentication instead.
- **The database** is SQLCipher keyed with a raw 32-byte key — the page key is
  supplied directly, so there is no passphrase KDF pass and no cipher parameter
  is set.
- **Key derivation** is HKDF-SHA256 from a device secret: one extract, two
  expands (`copypaste/v2/sqlcipher-db-key`, `copypaste/v2/item-content-key`).
  Neither key is the stored secret.
- **Crypto fails closed.** A wrong key, a wrong AAD or a tampered ciphertext
  gives an authentication error with no detail and no fallback read.
- Key material is zeroized on drop; secret comparisons are constant-time.

### Where the device secret lives

| Platform | Store | State |
|---|---|---|
| macOS | Keychain, behind the `macos-keychain` cargo feature | Compiled and lint-clean on `macos-14` in CI; **never executed** |
| Android | Android Keystore | Not built |
| Linux | `0600` file under the data directory | Development fallback, **not a shipping posture** |

The keystore fails closed: only an unambiguous "no entry" mints a new secret.
Any other error is surfaced rather than silently replacing a secret, which would
orphan the existing database.

## Sensitive content

A detector flags clipboard content that looks like a credential — the ruleset in
`crates/copypaste-core/src/sensitive/rules.rs`, taken from gitleaks where an
equivalent exists, with NFKC normalisation, Luhn validation for card numbers, and
entropy and allowlist gates against false positives.

What follows from a match:

- **It is never written to the search index.** Enforced at write time and again
  at read time.
- **It never leaves the device.** Peer sync does not list it and cloud sync
  refuses to upload it. This inverts v1, which synced sensitive items encrypted
  and left the receiver to re-evaluate.
- **It is not deleted automatically.** Only rules above a confidence floor are
  eligible for a destructive action, and no automatic deletion is implemented. A
  false positive destroying unrecoverable user data is the worse outcome.

Detection is best-effort and will miss things — the security review found two
whole classes it was missing (F-1, F-2). Do not rely on it as the only control
over what reaches your clipboard history.

## Local IPC

The daemon listens on a Unix domain socket at mode `0600`, owned by the running
user. There is no network listener for IPC and no auth token: the filesystem
permission is the boundary, so any process running as the same user can read the
whole history. That is the trust boundary the system clipboard already has.

**No user-facing error may contain a filesystem path**, because the socket path
discloses the local username. Enforced in the daemon and again by a redaction
pass shared by every client, with tests asserting it.

## Peer-to-peer sync

- The channel is Noise `NNpsk0` (`snow`): mutual authentication and forward
  secrecy from the pairing key alone.
- The pairing token is 256 bits from the OS CSPRNG, shown as a Crockford base32
  code. Possession is the authentication — there is no password, so there is no
  dictionary to attack. Treat a code like a password. It is shown once, stays
  redeemable for five minutes, and the first session that completes burns it.
- A wrong key fails the handshake on the first message. There is no
  unauthenticated mode to fall back to.
- A session poisons itself after any authentication failure rather than
  continuing with a desynchronised nonce.
- Peer keys live in a `0600` file, written atomically.
- **Content crosses the wire as plaintext inside the Noise channel**, and the
  receiver re-encrypts under its own key. Confidentiality comes from the
  transport, not a second envelope — the sender's ciphertext is bound to a key
  the receiver does not have.
- mDNS advertises a `pairing_id`: a domain-separated BLAKE2s of the token,
  truncated to 128 bits. It is one-way and not usable as a credential, but it is
  derived from the token rather than independent of it, and it is a stable
  identifier broadcast on every network the device joins (F-7).

A peer's item stamped more than 24 hours in the future is skipped, so a device
with a broken clock cannot win every comparison for an item and censor it
everywhere.

## Cloud sync

**Wired into the daemon and the CLI, and never once spoken to a real Supabase
project.** `scripts/demo-cloud.sh` drives two real daemons against a local stub
(`scripts/cloud-stub.py`) imitating GoTrue and PostgREST; it asserts
convergence, that a sensitive item never leaves its device, and that only
ciphertext reaches the backend. It cannot tell you a real deployment accepts any
of it.

Rows are sealed client-side under an Argon2id key derived from a passphrase that
never leaves the device, so the server holds ciphertext and metadata only.
Row-level security is the second layer — a misconfigured policy would expose
rows that remain unreadable.

The daemon stores the access token, the rotated refresh token and the derived
sync key in the SQLCipher database, under the device key from the OS keystore.
Never the account password and never the passphrase, so a stolen database yields
a session that expires and a key for one account — not the means to re-derive
it. Sign-out clears all three and keeps the deployment URL and anon key, which
are configuration rather than credentials.

Sign-in carries three secrets over the `0600` IPC socket, which is the only
authentication boundary. The CLI takes the password and passphrase from the
environment or stdin and has no flag for either: process arguments are readable
by every process running as the same user. The passphrase is zeroized once the
key is derived; the request frame it arrived in is not, so it is "not persisted"
rather than "not in memory".

What leaves the device is gated twice — the outbound query never lists a
sensitive row, and the driver re-checks each item against the same detector
before sealing it (manifest 05 AT-56: v1 had one enforcement point and it had a
hole).

Metadata the backend sees that v1's account-less relay did not: account email,
`origin_device_id`, content types, payload sizes, timestamps. Content stays
end-to-end encrypted; the metadata surface is a real regression, and sync now
requires an account.

## Unverified

Written, never observed working on a platform we ship to:

- The **macOS Keychain** store and the **NSPasteboard** capture path. CI's
  `macos-check` job compiles and lints them on `macos-14` with `--all-features`
  and runs the portable half of the suite there, so "never compiled" is no
  longer true. Nothing drives a real pasteboard or a real keychain entry.
- **Cloud sync against a live Supabase project.** Unit tests use in-process
  fakes; the demo uses a local stub. No deployment has ever had `supabase/`'s
  schema and RLS policies applied to it.
- **mDNS discovery** — the development container has no multicast; pairing is
  exercised only over explicit addresses.
- **The app on either shipping target.** The `e2e/` suite drives it through a
  real WebKitGTK WebView on Linux, which is neither WKWebView nor Android's
  WebView, and no macOS `.app` or Android APK has been produced.

## Not implemented

Sensitive-item auto-wipe, private mode, an application exclusion list, device
revocation and sync-key rotation reachable from any client, rate limiting on any
surface, and telemetry.

What is outstanding, re-checked against the tree, is
[`docs/backlog.md`](docs/backlog.md).

## macOS permissions

**The app requests no TCC permission** — not Accessibility, not Input
Monitoring. Reading `NSPasteboard` needs none, and selecting an item puts it on
the clipboard instead of synthesising Cmd+V. The global hotkey goes through
Carbon `RegisterEventHotKey`; `shell::hotkey::is_permission_free` refuses the
media keys, which `global-hotkey` binds with an event tap instead and which
would therefore cost an Accessibility grant. That no prompt appears is inferred
from documentation, not observed.

This is a security property and a distribution constraint at once: the app is
ad-hoc signed, so macOS would tie any grant to a code hash that changes on every
build and revoke it on every update. See
[ADR-0001](docs/adr/0001-macos-distribution-without-a-developer-id.md).

## Known limitations

- Android restricts clipboard reads to foreground apps; see
  [`docs/rewrite/android-clipboard-access.md`](docs/rewrite/android-clipboard-access.md).
- Windows and Linux desktop are out of scope.
- v2 reads nothing written by v0.4.x, and uses a distinct database filename so
  an older file is never opened, modified, or reported as corrupt.

## Dependency auditing

`cargo deny check` and `cargo audit`, both run in CI by
`.github/workflows/supply-chain.yml` on every push and weekly on a schedule.
