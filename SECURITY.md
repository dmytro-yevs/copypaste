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
[`docs/rewrite/security-review.md`](docs/rewrite/security-review.md). It is a
dated document, not a live one: each finding records its own status in place.
What is still outstanding is [`docs/backlog.md`](docs/backlog.md), which is
maintained.

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

`crypto/keystore/` has three `#[path]` modules and no feature gate: the backend
is chosen by `target_os` alone, so no build can be configured into the weaker
one.

| Platform | Store | State |
|---|---|---|
| macOS | Keychain, via `security-framework` | Compiled and lint-clean on `macos-14` in CI; **never executed** |
| Android | Android Keystore. It holds keys, not blobs, so an AES-GCM key that never leaves it wraps the secret, and the wrapped blob sits in app-private storage | **Never compiled** — no NDK on any host here |
| Linux | `0600` file under the data directory | Development fallback, **not a shipping posture** |

Minting a fresh secret needs both an unambiguous "no entry" *and* a data
directory with no history database in it. Any other keystore error is surfaced
rather than minted over, and a database sitting next to a missing secret means
we looked in the wrong place — either way, replacing the secret would orphan the
history.

## Sensitive content

A detector flags clipboard content that looks like a credential — the ruleset in
`crates/copypaste-core/src/sensitive/rules.rs`, taken from gitleaks where an
equivalent exists, with NFKC normalisation, Luhn validation for card numbers, and
entropy and allowlist gates against false positives.

What follows from a match:

- **It is never written to the search index.** A write guard drops the index
  text whatever the caller passed, the flag is re-read inside the writing
  transaction, and `is_sensitive = 0` is joined into the search SQL itself. The
  sync-apply path is a fourth writer and enforces the same rule.
- **A rule added later still reaches rows captured before it.**
  `sensitive::purge_indexed_secrets` runs at daemon start and drops from the
  index anything the *current* ruleset calls sensitive. Index only: it never
  deletes a row and never writes `is_sensitive`, because that flag is what
  authorises the wipe below, and a re-derived one would be a deletion nobody
  reviewed.
- **It never leaves the device.** Peer sync does not list it and cloud sync
  refuses to upload it. This inverts v1, which synced sensitive items encrypted
  and left the receiver to re-evaluate.
- **Automatic deletion exists and is off** — `sensitive_ttl_secs` defaults to
  `0`. Switched on, a flagged item is hard-deleted once its TTL elapses, but
  only if it was flagged at capture *and* its plaintext still scans as high
  confidence when the sweep reads it.

Detection is best-effort. One adversarial pass over the ruleset found two whole
classes it did not see — quoted values, and `aws_secret_access_key` — and both
are closed; assume a third exists. Do not rely on this as the only control over
what reaches your clipboard history.

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
  truncated to 128 bits. One-way, so not a credential — but derived from the
  token rather than independent of it, which buys an attacker two things we
  accept: a candidate code can be matched to a device on the LAN offline, and
  the ids are stable identifiers broadcast on every network the device joins.
- The pairing list is capped, and the cap refuses a *new* pairing rather than
  evicting an old one. Nobody can push a real pairing out by making more.

A peer's item stamped more than 24 hours in the future is skipped, so a broken
clock cannot censor an item *permanently*. It can for a day: `now + 24 h − ε`
still wins every comparison until real time catches up. That is the accepted
trade, and the ceiling lives in each transport rather than at the merge they
share.

## Cloud sync

**Wired into the daemon and the CLI, and never once spoken to a real Supabase
project.** `scripts/demo-cloud.sh` drives two real daemons against a local stub
(`scripts/cloud-stub.py`) imitating GoTrue and PostgREST, asserting convergence,
that a sensitive item never leaves its device, and that only ciphertext reaches
the backend. It cannot tell you a real deployment accepts any of it.

Rows are sealed client-side under an Argon2id key derived from a passphrase that
never leaves the device, so the server holds ciphertext and metadata only.
Row-level security is the second layer — a misconfigured policy would expose
rows that remain unreadable.

The fields sync *orders* on travel in the clear, because the backend pages on
them, so each row carries an HMAC over them plus the ciphertext and the nonce,
under a second key from the same passphrase. A device verifies before the row
reaches the merge. Without that, someone holding the account password but not
the passphrase could stamp a competing version that outranks the real one, or a
tombstone that makes an item disappear everywhere.
[`docs/cloud-privacy.md`](docs/cloud-privacy.md) is the full disclosure page.

The daemon stores the access token, the rotated refresh token and the derived
sync key in the SQLCipher database, under the device key from the OS keystore —
never the account password and never the passphrase. A stolen database yields a
session that expires and a key for one account, not the means to re-derive it.
Sign-out clears all three and keeps the deployment URL and anon key, which are
configuration rather than credentials.

Sign-in carries three secrets over the `0600` IPC socket, which is the only
authentication boundary. The CLI takes the password and passphrase from the
environment or stdin and has no flag for either, because process arguments are
readable by every process running as the same user. The passphrase is zeroized
once the key is derived; the request frame it arrived in is not, so it is "not
persisted" rather than "not in memory".

What leaves the device is gated twice — the outbound query never lists a
sensitive row, and the driver re-checks each item against the same detector
before sealing it (manifest 05 AT-56: v1 had one enforcement point and it had a
hole).

Against v1's account-less relay this is a real regression in metadata privacy,
taken deliberately: the backend now sees an account email, device ids, content
types, payload sizes and timestamps. Content stays end-to-end encrypted.

## Unverified

Every security control on a shipping platform is written, reviewed, and never
observed working. `README.md`'s Unverified table is the full list; the three
that decide whether anything above holds:

- The **macOS Keychain** store and the **NSPasteboard** capture path. CI
  compiles and lints them on `macos-14`. Nothing drives a real pasteboard or a
  real keychain entry.
- The **Android Keystore** store and the capture ladder. No host here has an
  NDK, so neither has been compiled; every claim about what Android permits is
  read out of AOSP source
  ([`docs/rewrite/android-spike.md`](docs/rewrite/android-spike.md)).
- **Cloud sync against a live Supabase project.** No deployment has ever had
  `supabase/`'s schema and RLS policies applied to it, so the second layer under
  the row encryption is unproven.

## Controls that are weaker than their names suggest

A list of absences is the most perishable thing this page can carry, so it lives
in [`docs/backlog.md`](docs/backlog.md), whose §2 is the security tier. Two
belong here because they change what the rest is worth:

- **`unpair` is local and unilateral.** It removes this device's half; the other
  device keeps its half until it also unpairs. Nothing revokes a lost device
  from here, and there is no sync-key rotation.
- **The application exclusion list is stored and enforced by nothing** — the
  capture path has no frontmost-app attribution to match it against. What *is*
  enforced is the `org.nspasteboard.*` opt-out, which is how password managers
  ask to be skipped. Private mode does not exist.

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

- Android restricts clipboard reads to foreground apps. The four-rung ladder and
  what each rung costs a user are in
  [`docs/rewrite/android-clipboard-access.md`](docs/rewrite/android-clipboard-access.md).
- Windows and Linux desktop are out of scope.
- v2 reads nothing written by v0.4.x, and the distinct filename means an older
  file is never opened or modified. It does **not** yet tell the user it found
  one: `is_v1_database` is only ever handed `copypaste-v2.db`, and
  `v1_database_in`, which asks the question that matters, has no caller. An
  upgrading user sees an empty history and no explanation.

## Dependency auditing

`cargo deny check` and `cargo audit`, both run in CI by
`.github/workflows/supply-chain.yml` on every push and weekly on a schedule.
