# Security

> Foundation status: the React/Tauri application and its release artifacts have
> been removed. Rust security controls and the local Supabase harness remain;
> no Flutter product, Rust bridge, platform host, updater or release is
> qualified. The sections below record the retained backend security model.

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

This document describes the current CopyPaste security model.

## At rest

- **Content** is sealed with XChaCha20-Poly1305, with the item id bound as
  associated data: a row's ciphertext cannot be moved to another row, it fails
  authentication instead.
- **The database** is SQLCipher keyed with a raw 32-byte key — the page key is
  supplied directly, so there is no passphrase KDF pass and no cipher parameter
  is set.
- **Key derivation** is HKDF-SHA256 from a device secret, with separate labels
  for SQLCipher, item content and the LAN pairing store (`copypaste/v2/sqlcipher-db-key`,
  `copypaste/v2/item-content-key`, `copypaste/v2/peer-store-key`). None is the stored secret.
- **Crypto fails closed.** A wrong key, a wrong AAD or a tampered ciphertext
  gives an authentication error with no detail and no fallback read.
- Key material is zeroized on drop; secret comparisons are constant-time.

### Where the device secret lives

`crypto/keystore/` selects the production backend by platform. Debug macOS
deliberately retains a development-only `0600` plaintext device-secret file;
its database and pairing encryption do not protect against theft of that file.

| Platform | Store | State |
|---|---|---|
| macOS | Keychain, via `security-framework` | CI runs isolated-Keychain Rust tests; execution remains unverified until that job has passed on this foundation |
| Android | Android Keystore. It holds keys, not blobs, so an AES-GCM key that never leaves it wraps the secret, and the wrapped blob sits in app-private storage | **Never compiled** — no NDK on any host here |
| Windows | Device secret sealed with DPAPI under the user's login | Same-device and same-user protection |
| Linux | `0600` file under the data directory | Development fallback, **not a shipping posture** |

Minting a fresh secret needs both an unambiguous "no entry" *and* a data
directory with no history database in it. Any other keystore error is surfaced
rather than minted over, and a database sitting next to a missing secret means
we looked in the wrong place — either way, replacing the secret would orphan the
history.

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
- The pairing token is 256 bits from the OS CSPRNG. Manual entry uses its
  Crockford base32 code; the automatically displayed QR wraps the same token and
  LAN address in the versioned `copypaste://pair/v1` URI. Possession is the
  authentication — there is no password, so there is no dictionary to attack.
  Treat either form like a password. The invite stays redeemable for two minutes,
  and the first session that completes burns it.
- A wrong key fails the handshake on the first message. There is no
  unauthenticated mode to fall back to.
- A session poisons itself after any authentication failure rather than
  continuing with a desynchronised nonce.
- Peer keys and revocations live in an XChaCha20-Poly1305 encrypted envelope in
  `peers.json`, under the device-derived pairing-store key. Every replacement
  uses a fresh nonce and retains owner-only atomic writes. Existing plaintext
  state is converted once, without a plaintext backup, before startup completes.
  A marker in SQLCipher then disables plaintext import; a wrong key or damaged
  envelope refuses to open instead of resetting trust or retrying as JSON.
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
(`scripts/cloud-stub.py`) imitating GoTrue and PostgREST, asserting convergence
and that only ciphertext reaches the backend. It cannot tell you a real
deployment accepts any of it.

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

The backend sees an account email, device ids, content types, payload sizes and
timestamps. Content stays end-to-end encrypted.

## Unverified

Every security control on a shipping platform is written, reviewed, and never
observed working. `README.md`'s Unverified table is the full list; the three
that decide whether anything above holds:

- The **macOS Keychain** store and the **NSPasteboard** capture path. CI is
  configured to execute isolated-Keychain and pasteboard Rust tests; they remain
  unverified until those jobs have passed on this foundation.
- The **Android Keystore** store and capture behavior. Their Flutter host and
  Rust bridge are not implemented yet, so no product claim is qualified.
- **Cloud sync against a live Supabase project.** No deployment has ever had
  `supabase/`'s schema and RLS policies applied to it, so the second layer under
  the row encryption is unproven.

## Controls that are weaker than their names suggest

Two limitations change what the rest is worth:

- **`unpair` is local and unilateral.** It removes this device's half; the other
  device keeps its half until it also unpairs. Nothing revokes a lost device
  from here, and there is no sync-key rotation.
- **Application attribution is necessarily best effort.** The macOS capture
  path caches the frontmost bundle id briefly, applies the persisted exclusion
  list and always skips known password managers before reading a representation.
  If exclusions are configured but attribution is unavailable, it fails closed.
  Private mode is persisted and gates capture before any representation read.

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

- Android restricts clipboard reads to foreground apps. The new product must
  define and qualify its capture behavior before release.
- Linux desktop is a test surface, not a shipping target.

## Dependency auditing

`cargo deny check` and `cargo audit`, both run in CI by
`.github/workflows/supply-chain.yml` on every push and weekly on a schedule.

Rust advisory exceptions remain versioned and checked by
`scripts/check_rustsec_policy.py`. Flutter, Gradle and platform-host
dependency policy is required before an application release.

## Secret scanning

`gitleaks`, run in CI by `.github/workflows/secret-scan.yml` from
`supply-chain.yml` on every push and pull request and from `release.yml` before
a release publishes. Push and pull request scan the tree; the weekly cron scans
every commit, because a secret that was committed and then deleted is still in
every clone.

With `gitleaks` on `PATH`, the same checks run locally:

```sh
./scripts/check-secret-scan.sh --self-test   # prove the scan can still fail
./scripts/check-secret-scan.sh --scan        # tree; add --history for commits
```

`.gitleaks.toml` extends the upstream default ruleset and allowlists reviewed
synthetic fixtures by rule id and literal, never by path — a path entry makes
gitleaks skip the whole file, which would stop it scanning a fixture's
neighbours. `.gitleaksignore` pins reviewed pre-v2 history findings by commit.
This repository scan is separate from the application and has no runtime effect
on captured clipboard items.
