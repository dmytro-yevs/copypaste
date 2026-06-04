# CopyPaste — Privacy-Persona Product Critique

**Persona:** Security-conscious user. Stores passwords, 2FA codes, recovery
phrases, financial data, and API keys in the clipboard. Chose CopyPaste
specifically because it is E2E-encrypted, self-hostable (relay), and does not
depend on iCloud or Google. Distrusts centralised cloud. Prefers P2P or
self-hosted relay. Wants zero plaintext leaving the device unencrypted, fine
control over what is captured, and confidence that sensitive items do not
linger or sync anywhere I have not explicitly approved.

---

## 1. What Earns My Trust

These are the features that made me install and keep the app. I have read the
source, and these claims check out.

**XChaCha20-Poly1305 with 192-bit random nonces.** Every clipboard item is
encrypted before it hits SQLite. Two encryptions of the same plaintext produce
different ciphertexts because each nonce is freshly drawn from the OS CSPRNG.
The algorithm choice is correct: 192-bit nonces make birthday-bound collision
irrelevant at any realistic item count. I compared this to the competition —
every iCloud-backed manager (Paste, Pastebot, PastePal) hands Apple the keys.
Raycast does not sync at all. Only ClipCascade comes close, and its Android
client is less polished.

**AEAD AAD binding `(item_id, schema_version, key_version)`.** A stolen row
cannot be replayed into a different row: the auth tag will fail. The v1
empty-AAD path was permanently removed in v0.3. Cloud items use a distinct
schema version (5) so they cannot be silently decoded as local items. This is
correct design and I verified it in `encrypt.rs`.

**SQLCipher at rest.** The database is AES-256-CBC encrypted with the key held
in the macOS Keychain or an Android KeyStore-backed file, not on disk in
cleartext. Even if someone pulls the `.db` file off a stolen machine, they
cannot read it without the device key. The key is stored with
`kSecAttrAccessibleWhenUnlockedThisDeviceOnly` and `kSecAttrSynchronizable =
false`, which means it does not go into iCloud Keychain backup. That matters
to me.

**Sensitive item TTL (default 30 seconds).** Passwords and API keys that match
the detector are timestamped with `expires_at` and purged on the next daemon
tick. 37 patterns cover AWS keys, GitHub PATs, OpenAI keys, Anthropic keys,
SSH private keys, JWTs, credit card numbers (Luhn-validated), IBANs, database
connection strings, and more. Having the item disappear automatically is the
correct default. Every other macOS clipboard manager relies on the password
manager clearing the system clipboard on first paste — which only helps for
the original copy, not subsequent re-pastes from clipboard history.

**PAKE device pairing (OPAQUE-KE) with SAS verification.** LAN pairing runs a
3-message OPAQUE-KE handshake over a bootstrap TLS channel. The 6-digit Short
Authentication String is derived from the post-PAKE, post-TLS channel-binding
key. If a MITM intercepts and relays the PAKE messages over two separate TLS
sessions, the channel binders differ, the SAS digits differ, and the human
sees a mismatch. This is the right construction. QR pairing uses 256-bit
entropy tokens, so there is no human-visible code to leak at all.

**Device revocation and key rotation.** The `revoke_and_rotate` path blocks
the evicted device from P2P and rotates the shared sync key, making the old
relay inbox HKDF-diverge and become unreachable. If my laptop is stolen, I
can lock it out of sync in one action. No iCloud-backed competitor offers
per-device key isolation.

**Self-hostable relay.** I can run the relay Axum server on my own hardware.
The relay never sees plaintext — items are re-encrypted under the shared sync
key client-side before upload. The relay only stores opaque AEAD blobs with
per-device auth tokens. This is the architecture I need.

**Sensitive-app detection.** A hardcoded allowlist of 17 password-manager
bundle IDs (1Password, Bitwarden, KeePass, Dashlane, LastPass, Enpass,
NordPass, RoboForm) triggers sensitive treatment regardless of content. If I
accidentally copy from one of these apps without the manager clearing the
clipboard, CopyPaste treats the item as sensitive and auto-wipes it. Good
defence-in-depth.

**Telemetry off by default.** The telemetry crate returns a `NoopReporter`
unless I explicitly pass an `Enabled*` consent value and a DSN. Even when
opted in, the wire payload is a single string:
`copypaste-daemon@0.6.1 [macos] keychain.read_failed`. No clipboard contents,
no paths, no IPs, no user IDs.

**FLAG_SECURE on Android.** The app sets `WindowManager.LayoutParams.FLAG_SECURE`
before `setContent` in `MainActivity`, blocking screenshots and recents
thumbnails for the entire app. My clipboard history will not appear in the
Android task switcher.

---

## 2. What Worries Me or Is Inconvenient

These are real gaps I found by reading the code. Not speculation.

**Sensitive detection is text-only.** Images and files are never
pattern-scanned (`features-core-security.md` §8.9: "Sensitive detection is
text-only"). A screenshot of my 1Password recovery phrase, a PDF containing
an SSH private key, or a photo of a credit card is stored without the
`is_sensitive` flag. It will not receive the 30-second TTL, will not be masked
in the history view, and will sync to other devices without any warning. This
is a significant gap. A clipboard manager that promises to protect sensitive
data must extend that promise to images.

**The `detect()` vs `is_sensitive_for_autowipe()` confidence bug.** The code
ships a documented bug (marked `# FIXWAVE` at `detector.rs:197`): the daemon
calls `detect(&text).is_some()` where it should call `is_sensitive_for_autowipe`.
The consequence is that email addresses (confidence 0.60), phone numbers
(0.55), and passport-like codes (0.55) — which the design explicitly intends
to retain — may currently receive a 30-second `expires_at` and be silently
deleted. I might copy an email address or a phone number for a legitimate
reason and have it vanish without warning. This erodes trust in the auto-wipe
feature because I cannot predict what will disappear.

**No manual "mark sensitive".** Once an item is captured, there is no IPC
method or UI affordance to manually set `is_sensitive = 1` on it. If the
detector misses an item — a novel API key format, an OTP URL, a recovery
phrase in an unusual casing — it stays in cleartext preview in the history
list forever. I need to be able to say "this item is sensitive, mask it and
wipe it in 30 seconds" after the fact.

**Private mode is all-or-nothing.** The only way to stop capture from a
specific app is to put the entire daemon in private mode (all capture off) or
use the `excluded_app_bundle_ids` list to block that app entirely. There is no
per-app sensitivity override, no per-app TTL, and no per-app sync exclusion.
If I want to capture from my browser but never capture from my banking app or
my SSH key manager, I must use the exclusion list. That is reasonable on
macOS, but on Android the analogous per-package exclusion exists only in
Settings and is not surfaced in the onboarding as a security recommendation.

**PAKE password sent in-clear inside the bootstrap TLS channel (discovery path).**
`features-sync.md` §5.1 documents this explicitly: "on the discovery path,
the PAKE password is an ephemeral random string sent in-clear inside the
bootstrap TLS channel — authentication is NOT provided by the PAKE password
here. Authentication is entirely provided by the human SAS comparison." This
is the correct design — OPAQUE-KE over a random password provides no
meaningful pre-auth on this path, and the SAS comparison is the real
protection. But it means that if I skip the SAS check or the SAS modal does
not open on the responder side, there is no fallback authentication. I need
the SAS to always fire.

**Relayed-PAKE channel-binding TODO.** The ADR-008 reference and threat model
note that the relay channel binding for the bootstrap path is a known open
item. On the LAN discovery path the TLS channel binder is in-play. On the QR
path the 256-bit token provides authentication. But if a relay-proxied pairing
were ever added, this would need explicit re-review.

**Plaintext briefly in daemon RAM and in FTS.** Decrypted item text is indexed
in the `clipboard_fts` FTS5 virtual table — in plaintext — for search. This
is a design necessity (you cannot search encrypted text) but it means the
SQLite WAL may briefly contain or persist plaintext FTS entries. The Keychain
key protects the DB file, but the WAL and shared-memory files (`-wal`, `-shm`)
are in the same directory and protected only by filesystem permissions. A
forensic acquisition of the disk could recover recently-written WAL pages
before they are checkpointed and overwritten.

**Pinned items are exempt from ALL pruning, including sensitive TTL.** From
`features-core-security.md` §4.4: "Pinned items are exempt even if marked
sensitive." If I pin an item that contains a password, the 30-second TTL is
silently skipped. I might expect pin to mean "keep this in history" without
realising it also disables the security wipe. The UI gives no warning.
Similarly, there is no ceiling on total pinned bytes — I could inadvertently
pin large sensitive images that will never be cleaned up.

**FLAG_SECURE absent on Android DevicesActivity.** The own-QR section on the
Devices screen is blurred by default, but the window lacks `FLAG_SECURE`, so a
screen-recording app with the correct permissions could capture the QR code
after the user taps to reveal it. `features-android.md` §10 calls this out
explicitly. Pairing QR codes are 256-bit tokens — if captured and replayed
before expiry, they grant full device pairing.

**Sync key rotation orphans the old relay inbox.** From `features-sync.md`
§9.8: "after `rotate_sync_key` IPC, the HKDF-derived inbox ID changes. Items
in the old inbox that have not yet been polled are permanently lost." If I
rotate the key to revoke a compromised device but another legitimate device has
not polled in the last 5 seconds, it will miss items that were in the
in-flight queue. This is a correctness gap during a security-critical
operation.

**Clear-all does not propagate.** `delete_all` is a local SQL DELETE with no
broadcast. If I want to wipe my history after a security incident, I must
manually clear each device. The "panic wipe" use case — a single action that
removes all history from all devices simultaneously — is not implemented.

**Android SharedPreferences backend.** On Android, clipboard history is stored
in `SharedPreferences` rather than SQLCipher. Encryption goes through the
UniFFI `encryptText`/`decryptText` calls (XChaCha20-Poly1305 via Rust), but
the storage medium is a XML-backed key-value store, not a proper encrypted
database. This is functionally equivalent if the encryption is correct, but
the `AES-256-GCM Android KeyStore fallback` path that activates when the
native `.so` is absent is weaker (GCM vs ChaCha20) and the KeyStore-backed
key is in a different trust domain than the Rust HKDF path. I need to know
which path I am on.

---

## 3. What Is Missing That I Need

These are features I would require before trusting this app with recovery
phrases and financial data, ranked by urgency.

**P0 — Panic wipe (propagated clear-all).** A single action that broadcasts
soft-delete tombstones for every non-pinned item to all connected devices and
purges the local DB. This is the "security incident" button. Without it, a
breach on one device leaves history intact on all others until I manually
touch each device.

**P0 — Manual "mark sensitive".** A right-click or long-press action that sets
`is_sensitive = true` on any item and starts the TTL clock immediately. The
auto-detector misses novel patterns. I need to be able to override it.

**P1 — Per-app capture blocklist on Android (UI-visible, recommended in
onboarding).** The `excluded_app_bundle_ids` list exists on both platforms, but
it is buried in Settings and not surfaced during onboarding as a security
best-practice. The first-run experience should prompt: "Do you have any
banking or password-manager apps you want to exclude?"

**P1 — Sensitive detection for images (OCR or heuristic).** At minimum, images
from password-manager apps should be flagged sensitive by the app-bundle-ID
heuristic already used for text. Full OCR (e.g. Apple Vision on macOS) would
be better, but even app-aware image flagging would close the biggest gap.

**P1 — Fix the `detect()` vs `is_sensitive_for_autowipe()` bug.** This is
documented as a `# FIXWAVE` TODO. It is not a missing feature — it is a
shipped bug that silently deletes items I do not intend to delete while
simultaneously under-protecting items I do want to wipe.

**P2 — Audit log for sensitive-item lifecycle.** I want to know: what items
were detected as sensitive, when they were created, when they were wiped, and
whether they were synced before wiping. Right now this information is in daemon
logs that rotate and are not user-accessible in a structured form.

**P2 — Local-only mode (no sync, not even P2P).** A mode where a device
captures locally and never sends items anywhere, regardless of pairing
configuration. Some items — say, a one-time recovery phrase — should never
leave the device, even to my own trusted paired devices. An item-level "local
only" flag would serve this need.

**P2 — `FLAG_SECURE` on Android DevicesActivity.** Simple to add; prevents
screen-recording apps from capturing an exposed pairing QR.

**P2 — Warning when a pinned item is also sensitive.** A pinned sensitive item
bypasses TTL silently. At minimum, show a warning in the UI: "This item is
sensitive and pinned. Pin exempts it from auto-wipe. Unpin to allow
auto-wipe."

**P3 — Screenshot protection / `FLAG_SECURE` on macOS (optional setting).** On
macOS there is no `FLAG_SECURE` equivalent, but Tauri can use the
`NSWindowCollectionBehavior` to exclude the window from screen capture. I
would like a setting to prevent the history window from appearing in
screenshots or screen recordings.

**P3 — Configurable sync exclusion per item or per content type.** Let me mark
specific items or entire content types (e.g., "never sync images") as
local-only without requiring me to disable sync globally.

---

## 4. Honest Verdict — Do I Trust It Enough?

**Provisional yes, with three dealbreakers.**

The cryptographic foundation is genuinely strong. XChaCha20-Poly1305,
HKDF-SHA512, AEAD AAD binding, SQLCipher at rest, PAKE pairing with SAS, key
rotation — this is better engineering than every commercial competitor. The
dev has clearly read the relevant papers. The self-hosted relay architecture
means I do not have to trust anyone but myself.

But I would not store recovery phrases or financial data in it today. My three
hard dealbreakers:

1. **No panic wipe.** A security incident on any device means my entire
   clipboard history on every device is exposed until I manually touch each
   one. This is a hard no for someone handling recovery phrases.

2. **Sensitive detection misses images.** Screenshots of private keys, QR
   codes for TOTP, and photos of credit cards are stored without any sensitive
   flag. They sync to all paired devices with no TTL, no masking, and no
   warning. The guarantee that sensitive data auto-wipes does not hold for
   images.

3. **The `detect()` / `is_sensitive_for_autowipe()` bug is shipped.** An
   unintentional `expires_at` on email addresses undermines trust in the
   auto-wipe mechanism. If the detector behaves unexpectedly in one direction,
   I cannot rely on it in the other direction either.

Fix those three and this becomes the most privacy-correct clipboard manager on
any platform. The underlying crypto is already there. The security UX just
needs to catch up.

---

## 5. Top 10 Wishlist (Ranked by Threat Model Impact)

**1. Propagated panic wipe**
_Why:_ Breach recovery requires a single action that zeroes history on all
devices atomically. The current `delete_all` is local-only. This is the most
important operational security tool I am missing.

**2. Sensitive detection for images (OCR or app-bundle heuristic)**
_Why:_ Password screenshots, TOTP QR codes, bank statements, private-key
photos are stored without any protection. The gap between "text is protected"
and "images are not" is too large for an app that advertises sensitive-data
handling.

**3. Fix `detect()` / `is_sensitive_for_autowipe()` bug**
_Why:_ A shipped, documented bug that causes unexpected auto-deletes and
incorrect TTL assignment undermines confidence in the detection layer entirely.
This is a correctness fix, not a feature.

**4. Manual "mark sensitive" with immediate TTL start**
_Why:_ The 37-pattern detector is good but not complete. Novel API key
formats, unusual passphrase encodings, and anything the detector misses will
sit in cleartext history indefinitely. A per-item override closes the gap.

**5. Per-item local-only flag**
_Why:_ Some items — recovery phrases, one-time OTPs, temporary credentials —
should never leave the device regardless of sync configuration. Item-level
isolation is finer-grained and more useful than a global sync toggle.

**6. Audit log for sensitive-item lifecycle**
_Why:_ I need to know what was captured, when it was classified as sensitive,
whether it synced before wiping, and when it was deleted. Without a structured
audit trail I cannot reason about whether a breach window existed.

**7. `FLAG_SECURE` on Android DevicesActivity**
_Why:_ The pairing QR is a 256-bit authentication token. After tap-to-reveal,
a screen-recording app with appropriate permissions can capture and replay it.
A one-line fix closes this window.

**8. Warning when a pinned item is also sensitive**
_Why:_ Pin silently disables the sensitive TTL. This is a footgun — users pin
things to keep them around and do not realise they are opting out of
auto-wipe. The UI should make this trade-off visible.

**9. Prominent per-app blocklist during onboarding**
_Why:_ The `excluded_app_bundle_ids` feature exists but is buried. A banking
app or SSH key manager that the detector misses will have all its clipboard
output captured and synced. Surfacing the blocklist during onboarding turns a
buried setting into a first-class security control.

**10. macOS optional screen-capture exclusion**
_Why:_ History containing passwords, API keys, and recovery phrases should not
appear in Cmd+Shift+3 screenshots or screen-share sessions. An opt-in window
exclusion flag reduces accidental exposure in remote-work and recording
contexts.

---

_Document path: `docs/product/persona-privacy.md`_  
_Audit basis: `features-core-security.md`, `features-macos.md`,
`features-android.md`, `features-sync.md`, `competitive-gap-analysis.md`,
`ux-ui-review.md`, `THREAT-MODEL.md`, `telemetry-policy.md`_  
_Branch reviewed: `feat/android-parity-v0.5.3` / `v0.6.1-integration` (same codebase)_  
_Date: 2026-06-04_
