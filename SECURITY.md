# Security

## Reporting a vulnerability

**Do not open a public GitHub issue for anything exploitable.** Open a private
security advisory on GitHub instead.

Useful in a report: the affected component (`core`, `daemon`, `cli`, `p2p`,
`cloud`), what an attacker must already have, and what they gain.

---

## Status

**v2 is alpha and has not been audited.** Several components have never been
executed on their target platform — see [Unverified](#unverified), which is the
section to read before trusting anything else on this page.

This document describes v2. The previous implementation is preserved on
`archive/v0.4.1-pre-rewrite` and differed in ways that matter here — most
sharply, it synced sensitive items and v2 does not. Do not read v1 documentation
as describing this code.

## At rest

- **Content** is sealed with XChaCha20-Poly1305. The item id is bound as
  associated data, so a row's ciphertext cannot be moved to another row — it
  fails authentication instead.
- **The database** is SQLCipher, keyed with a raw 32-byte key: the page key is
  supplied directly, so there is no passphrase KDF pass and no cipher parameter
  is set.
- **Key derivation** is HKDF-SHA256 from a device secret — one extract, two
  expands: `copypaste/v2/sqlcipher-db-key` and `copypaste/v2/item-content-key`.
  Neither key is the stored secret itself.
- **Crypto fails closed.** A wrong key, a wrong AAD or a tampered ciphertext
  gives an authentication error with no detail and no fallback read.
- Key material is zeroized on drop; secret comparisons are constant-time.

### Where the device secret lives

| Platform | Store | State |
|---|---|---|
| macOS | Keychain, behind the `macos-keychain` cargo feature | Written, **never executed** |
| Android | Android Keystore | Not built |
| Linux | `0600` file under the data directory | Development fallback, **not a shipping posture** |

The keystore fails closed: only an unambiguous "no entry" mints a new secret.
Any other error is surfaced rather than silently replacing a secret, which would
orphan the existing database.

## Sensitive content

A detector flags clipboard content that looks like a credential — around 42
rules, taken from gitleaks where an equivalent exists, with NFKC normalisation,
Luhn validation for card numbers, and entropy and allowlist gates against false
positives.

What follows from a match:

- **It is never written to the search index.** Enforced at write time and again
  at read time.
- **It never leaves the device.** Peer sync does not list it and cloud sync
  refuses to upload it. This inverts v1, which synced sensitive items encrypted
  and treated sensitivity as metadata for the receiver to re-evaluate.
- **It is not deleted automatically.** Only rules above a confidence floor are
  eligible for any destructive action, and no automatic deletion is implemented
  today. A false positive destroying unrecoverable user data is the worse
  outcome.

Detection is best-effort and will miss things. Do not rely on it as the only
control over what reaches your clipboard history.

## Local IPC

The daemon listens on a Unix domain socket at mode `0600`, owned by the running
user. There is no network listener for IPC and no auth token — the filesystem
permission is the boundary.

Any process running as the same user can therefore read the whole history. That
is the trust boundary the system clipboard already has.

**No user-facing error may contain a filesystem path**, because the socket path
discloses the local username. Enforced in the daemon and again by a redaction
pass shared by every client, with tests asserting it.

## Peer-to-peer sync

- The channel is Noise `NNpsk0` (`snow`): mutual authentication and forward
  secrecy from the pairing key alone.
- The pairing token is 256 bits from the OS CSPRNG, shown as a Crockford base32
  code. Possession is the authentication — there is no password, so there is no
  dictionary to attack. Treat a code like a password; it is shown once.
- A wrong key fails the handshake on the first message. There is no
  unauthenticated mode to fall back to.
- A session poisons itself after any authentication failure rather than
  continuing with a desynchronised nonce.
- Peer keys live in a `0600` file, written atomically.
- **Content crosses the wire as plaintext inside the Noise channel**, and the
  receiver re-encrypts under its own key. Confidentiality comes from the
  transport, not a second envelope — the sender's ciphertext is bound to a key
  the receiver does not have.
- mDNS advertises only a non-secret pairing id — never a token, and deliberately
  not a digest of one.

A peer's item stamped more than 24 hours in the future is skipped, so a device
with a broken clock cannot win every comparison for an item and censor it
everywhere.

## Cloud sync

**Not wired into the daemon.** The crate is built and tested against mocked
HTTP; nothing calls it yet.

By design: rows are sealed client-side under an Argon2id key derived from a
passphrase that never leaves the device, so the server holds ciphertext and
metadata only. Row-level security is the second layer — a misconfigured policy
would expose rows that remain unreadable.

## Unverified

Written, never observed working. Treat these claims as unproven:

- The **macOS Keychain** backend and the **NSPasteboard** capture path — never
  compiled, because development happens on Linux.
- **mDNS discovery** — the development container has no multicast; pairing is
  exercised only over explicit addresses.
- **Cloud sync against a live Supabase project** — every test uses in-process
  fakes.
- The **app on either target**. The Tauri shell builds and launches on Linux,
  but WebKitGTK executes no JavaScript under headless Xvfb, so what it renders
  is covered only by jsdom unit tests. It has never been built for macOS or
  Android.

## Not implemented

Age-based retention, private mode, an application exclusion list, rate limiting
on any surface, and telemetry.

## macOS permissions

**The app requests no TCC permission** — not Accessibility, not Input
Monitoring. Reading `NSPasteboard` needs none, the global hotkey goes through
Carbon `RegisterEventHotKey` rather than an event tap, and selecting an item
puts it on the clipboard instead of synthesising Cmd+V.

This is a security property and also a distribution constraint: the app is
ad-hoc signed, so macOS would tie any grant to a code hash that changes on
every build and revoke it on every update. See
[ADR-0001](docs/adr/0001-macos-distribution-without-a-developer-id.md).

An earlier version of this document claimed clipboard monitoring requires
accessibility permission on macOS. That was wrong.

## Known limitations

- Android restricts clipboard reads to foreground apps.
- Windows and Linux desktop are out of scope.

## Backward compatibility

v2 reads nothing written by v0.4.x, and uses a distinct database filename so an
older file is never opened, modified, or reported as corrupt.

## Dependency auditing

```bash
cargo deny check   # requires cargo-deny
cargo audit        # requires cargo-audit
```
