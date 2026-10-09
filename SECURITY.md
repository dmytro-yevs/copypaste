# Security

This document describes the current Flutter/Rust product for macOS, Android,
and Windows. Security claims are bounded by the platform and artifact actually
exercised. The isolated macOS assessment of release v1.0.14 on 8–9 October 2026
confirmed clipboard privacy-marker handling as a vulnerability. It did not
establish that every other surface is vulnerability-free.

## Reporting a vulnerability

Do not publish exploitable details in a public GitHub issue. Use the repository's
private security advisory channel. Include the affected version and platform,
prerequisites, a synthetic reproduction, and the observed impact.

## Encrypted storage and keys

- SQLCipher encrypts the history database with a raw 32-byte device-derived key.
  The key is supplied before schema access. SQLite temporary storage is in memory.
- Item content uses XChaCha20-Poly1305 with the item ID as associated data.
  Moving ciphertext to another row fails authentication.
- HKDF-SHA256 derives separate database, item, and LAN peer-store keys from the
  device secret. Derived keys do not replace the stored device secret.
- Production macOS uses Keychain; Windows uses same-user DPAPI; Android uses an
  Android Keystore AES-GCM key to wrap the device secret in app-private storage.
  Debug macOS deliberately uses a development-only owner-readable secret file.
- Key material is zeroized where its ownership permits. Wrong keys, damaged
  ciphertext, and ambiguous keystore failures fail closed. An inaccessible key
  must not silently create a new identity or orphan existing history.
- Desktop data directories and files use owner-only access. Backup and restore
  validate the candidate before replacing history and refuse overwriting an
  existing backup destination.

## Capture privacy

Private mode gates capture before reading clipboard content. Application
exclusions apply when the platform has sufficient source evidence. Attribution
is best effort: a foreground application is not proof of the producer of every
background copy. The product does not universally exclude password managers by
name, and an empty exclusion list is not a password-manager protection policy.

Settings exposes two independent live options, both enabled by default:

- **Skip confidential clipboard:** reject producer-marked confidential content.
- **Skip temporary clipboard:** reject producer-marked temporary/history-excluded
  content.

Older settings records acquire these safe defaults on upgrade. Unreadable
privacy settings also fail closed. Native adapters inspect producer hints before
materializing a payload and preserve accepted classification through storage and
synchronization. macOS uses ConcealedType and TransientType; Android uses the
clipboard description's IS_SENSITIVE hint; Windows uses monitor-exclusion and
clipboard-history opt-out formats. Android has no equivalent general transient
marker in this contract. These hints depend on the producer publishing them;
ordinary-looking text cannot reliably be identified as a password.

If the user allows confidential captures, the content stays encrypted and its
classification remains attached. History, the inspector, and Quick Paste present
a dust spoiler until explicitly revealed. Hidden spoilers expose no content to
accessibility. Confidential content does not enter the text or semantic search
index, notification previews, or automatic received-clipboard writes. Explicit
copy, full-item IPC reads, and deliberately initiated exports are still content
access; a spoiler is a display control, not a separate authentication boundary.
Existing unclassified history cannot retrospectively recover missing producer
hints and is not automatically deleted.

## Local process boundary

macOS desktop IPC uses a private Unix socket; Windows uses a user-restricted
named pipe. Same-user clients are intentionally trusted to read history. Android
hosts the runtime in the application process and exposes platform integration
through typed adapters. There is no privileged CopyPaste helper.

Requests, frames, content, cursors, and watcher counts are bounded. HTML clipboard
content is rendered as content, not executed as a webpage. File materialization
validates paths and refuses traversal and unintended overwrites. User-facing
errors must not disclose filesystem paths or secrets.

## Pairing and synchronization

LAN sessions use an authenticated Noise NNpsk0 channel. Invitations use random
256-bit tokens, expire after two minutes, and display their versioned QR URI
immediately. Scanning alone does not pair: both devices must confirm the common
handshake-bound SAS before peer persistence. Failed authentication is terminal.

Peer keys and revocations are stored in an authenticated encrypted envelope with
owner-only atomic replacement. A wrong key or damaged envelope refuses startup
instead of resetting trust. Revoking a peer locally refuses its subsequent sync;
it does not erase history already delivered to another device or rotate all keys.
The sender opens item content inside the authenticated transport, and the
receiver encrypts it with its own device key. Source metadata, including privacy
classification, is carried with the synchronized version.

Cloud synchronization is provided by an optional signed Supabase module. Content
is encrypted client-side under a passphrase-derived key. Metadata used for merge
ordering is authenticated; the backend observes account, traffic, and synchronized
metadata, including producer privacy classification. Credentials and derived sync keys use application-owned encrypted
state, not plaintext preference files. Never infer deployed row-level security
from repository SQL or local stubs: the actual deployment requires separate
qualification. See [cloud privacy](docs/cloud-privacy.md).

## Modules and updates

Modules are trusted first-party native code loaded in the runtime process.
Publisher signatures, strict package inventories, path checks, and file hashes
protect admission. They do not sandbox publisher-authorized native code or limit
it to declared application services. Process and permission isolation are not
implemented and must not be advertised.

Windows and Android updater assets use the embedded publisher key and independent
artifact signatures, in addition to integrity checks and platform signer checks.
macOS distribution uses DMG and the project's Homebrew tap. Homebrew checks the
required cask digest; the current macOS DMG has no independent publisher artifact
signature. HTTPS, release/tap control, and the cask checksum remain its trust
chain. The project has no Apple Developer ID and does not claim notarization.
Local re-signing supports installation and stable local permissions; it does not
authenticate the publisher.

## Screenshots and native permissions

**Block screenshots** is a device-local option, off by default. It applies to the
main application, Quick Paste, pairing QR/code/SAS, and application-owned windows.
With the option off, those surfaces can be captured. Pairing cannot override the
user's saved choice. Windows uses WDA_EXCLUDEFROMCAPTURE, Android uses FLAG_SECURE,
and macOS presents protected Flutter surfaces through a capture-protected native
layer. OS permission dialogs and file pickers are separate system surfaces.

The macOS app requests Accessibility when automatic pasting needs it. Ordinary
capture, history, pairing, and copying do not require that grant. Clipboard-read
and screen-recording consent remain subject to the OS. A passing screenshot test
does not qualify every recording API, transition frame, external camera, or GPU.

## Qualification limits

The v1.0.14 assessment exercised the released application in an isolated macOS
VM with synthetic data. Android and Windows source review is not physical Android
or installed Windows runtime evidence. Production Supabase policies, unresolved
native dependencies, every recording path, and unbounded resource-exhaustion
attacks require additional evidence. Each subsequent fix must be tested against
its actual source snapshot and shipping artifacts before being claimed as shipped.
