# Port Manifest 02 — Crypto

This manifest specifies the current v2 security contract. The implementation
has one device secret, one derivation tree and one authenticated content format.
There is no alternate key generation, version dispatch, trial decryption,
format repair or decode-only path.

The authoring sources are `copypaste-core::crypto`, `copypaste-cloud::crypto`
and the P2P handshake modules. Generated bindings and stored ciphertext are
consumers of those sources, not independent specifications.

## 1. Responsibilities

The crypto boundary owns:

- loading or creating the device secret through the platform keystore;
- deriving domain-separated SQLCipher and item-content keys;
- sealing item content with AAD that binds the logical item id;
- authenticated streaming for binary payloads;
- cloud row encryption and signing;
- pairing and transport secrets;
- zeroization, secret-safe comparison and path-redacted errors.

Storage owns transactions and rows. Sync owns merge order. IPC owns user-facing
error codes. Those layers may classify a crypto failure, but may not add a
fallback decrypt path.

## 2. Required security properties

### 2.1 One key path

1. A 32-byte device secret is created only after the keystore reports
   unambiguously that no entry exists.
2. Database and item keys are separate HKDF outputs with distinct `info`
   values. Reusing the same output for two purposes is forbidden.
3. The database key is passed to SQLCipher in raw-key form before any other
   statement on every connection.
4. Item encryption accepts one `ItemKey` type. There is no numeric key version,
   older-key parameter, loop over candidate keys or environment-selected
   production backend.
5. Runtime environment may select an ephemeral key only in a build carrying an
   explicit development feature. Shipped macOS, Android and Windows builds do
   not compile that path.

### 2.2 Authenticated encryption

1. Item content uses a maintained XChaCha20-Poly1305 implementation.
2. Every envelope receives a fresh nonce from the OS CSPRNG. Nonces are not
   counters and are never reused deliberately.
3. AAD binds the cross-device logical item id. Moving ciphertext to a different
   id must fail authentication.
4. Binary streaming authenticates segment order, stream identity and finality.
   Reorder, splice, gap, duplicate, missing tail and premature-final cases all
   fail.
5. Wrong key, wrong AAD and modified nonce/ciphertext/tag collapse to the same
   authentication error. Callers do not learn which check failed.
6. Authentication failure is never a partial success. The content is skipped
   and counted or the operation fails; unauthenticated bytes are never returned.
7. Caller-controlled input cannot trigger a panic. Invalid nonce length,
   truncated framing, an empty stream and an oversized input are typed errors.

### 2.3 Secret lifetime and comparison

Values containing device secrets, derived keys, shared secrets, passphrases,
pairing codes, session keys and decrypted plaintext are zeroized when their
last owner is dropped. Public APIs return zeroizing wrappers where ownership
crosses a module boundary.

Secret equality uses constant-time comparison. Secret-bearing types do not
derive `Copy`, do not expose bytewise `==`, and redact `Debug` and `Display`.
Logs never contain keys, passphrases, pairing codes, cloud inbox identifiers or
complete fingerprints.

### 2.4 Keystore fail-closed behaviour

The platform owner is selected by the target:

| Platform | Required owner |
|---|---|
| macOS | Keychain generic-password entry with the v2 service/account identifiers |
| Android | AES-GCM wrapping key held by Android Keystore plus an app-private sealed blob |
| Windows | DPAPI-protected blob scoped to the signed-in user |

The current v2 service/account identifiers are frozen because changing them
would orphan data written by v2 itself. This is an identifier-stability rule,
not a second lookup path.

Only “not found” authorizes creation. Locked, denied, timed out, malformed,
wrong-length and I/O outcomes leave the existing database untouched. A
temporarily inaccessible store maps to a retryable locked state; an entry that
exists but cannot be used maps to a non-retryable unusable state. Neither state
authorizes minting a replacement secret over existing history.

Potentially interactive keystore reads are time-bounded. A timeout may detach
the platform call, but a late result must not open the database after startup
has already degraded or failed.

### 2.5 Cloud and pairing separation

- Cloud content encryption is end-to-end: the backend never receives the
  passphrase or a derivable content key.
- Account identity participates in cloud key derivation so equal passphrases in
  different accounts do not yield equal keys.
- Empty account identity and a passphrase below the configured floor are
  rejected before expensive derivation.
- Pairing proof, transport key, confirmation tag and displayed SAS are separate
  domains. A value from one role cannot validate as another.
- Pairing persistence happens only after both peers confirm the handshake-bound
  SAS. Invitations and unconfirmed session secrets remain memory-only.
- Revocation removes or bars the relevant long-term key material; it is never
  simulated by accepting a weaker handshake.

### 2.6 Stable property IDs

Source comments use these identifiers as compact links to the current rule.
They remain stable even when this document is reorganized.

| ID | Current v2 property |
|---|---|
| I-8 | Cloud key derivation uses one canonical, non-empty account identity. |
| I-10 | The v2 keystore service/account identifiers are frozen and have one lookup path. |
| I-11 | Every envelope nonce comes from the OS CSPRNG and is never deliberately reused. |
| I-12 | Secret material crosses ownership boundaries in zeroizing types. |
| I-13 | Secret comparison is constant-time. |
| I-14 | Caller-controlled input returns typed errors and cannot panic. |
| I-15 | Wrong key, wrong AAD and tampering are one indistinguishable authentication failure. |
| I-16 | Derivations and confirmation roles are domain-separated. |
| I-17 | Argon2id parameters are a security floor and must be cross-platform reproducible. |
| I-18 | The passphrase floor counts Unicode scalar values, not bytes. |
| I-20 | Only an unambiguous keystore “not found” result authorizes secret creation. |
| I-22 | Potentially interactive keystore reads are time-bounded. |
| I-23 | Runtime environment cannot select a production key backend. |
| I-27 | A security constant duplicated across a necessary boundary has a compile-time equality guard. |

Ambiguity-free length-prefixing of caller-controlled fields is the rule formerly
cited as §3.2.2 in source comments. Platform keystore handling is the rule cited
as §3.8, and the Argon2id floor is the rule cited as §4.2. These are references
to current properties, not alternate format definitions.

## 3. Error contract

| Condition | Required classification | Required caller behaviour |
|---|---|---|
| wrong key, wrong AAD or tamper | authentication failed | fail closed; never retry another format |
| malformed nonce or stream structure | invalid input/corrupt record | reject without panic |
| keystore state unknown | key locked/unavailable | retry may be offered; do not mint |
| stored secret unusable | key unusable | explain that retry will not repair it; do not mint |
| input exceeds primitive limit | bounded internal/input error | reject before allocation growth |
| invalid pairing proof or tag | pairing/authentication failure | abort the ceremony without persistence |

Every rendered error is a fixed, path-free message. Underlying platform or I/O
errors may be logged only through the repository's redaction boundary and must
not be forwarded to a user.

## 4. Acceptance tests

### 4.1 Key derivation and keystore

- The same device secret derives stable database and item keys; the two outputs
  differ.
- Changing the device secret changes every derived key.
- Type-level tests prove key-returning APIs use zeroizing wrappers.
- Key-bearing types have redacted `Debug` and no cloning/copying surface that
  extends secret lifetime accidentally.
- Only platform “not found” creates a secret. Locked, denied, timeout,
  malformed-entry and generic I/O cases do not.
- A release build ignores the ephemeral-key environment variable.
- A development build samples the environment choice once per process.
- Every platform reopens data with the same keystore entry and refuses it with
  a different entry.
- Keystore and crypto error strings contain no absolute path or username.

### 4.2 Item AEAD

- Empty and non-empty plaintext round-trip.
- A single-bit change in nonce, ciphertext body or every tag byte fails.
- Truncated and sub-tag-length ciphertext fail without panic.
- Wrong key fails with the same public classification as tampering.
- Swapping ciphertext between two item ids fails in both directions.
- Two encryptions of the same input produce different nonces and ciphertexts.
- A large nonce-uniqueness sample contains neither duplicates nor an all-zero
  nonce.
- Decrypted buffers zeroize on drop.

### 4.3 Streaming

- Single- and multi-segment payloads round-trip.
- Empty input has one authenticated final segment or another maintained
  representation that cannot be confused with truncation.
- Reordered, missing, duplicated, cross-stream and premature-final segments
  fail.
- Zero segment size and malformed framing return errors, never panics.
- Peak memory stays bounded by the configured segment window rather than total
  payload size.

### 4.4 At rest

- The database, WAL and shared-memory sidecars do not contain known plaintext
  payloads after a checkpointed write.
- The database does not expose the plaintext SQLite header.
- The correct key reopens the database; a wrong key fails closed and does not
  trigger an unkeyed read, repair or replacement.
- Item ciphertext copied under another logical id cannot be read even when the
  SQLCipher database itself opens.

### 4.5 Cloud and pairing

- Same account and passphrase derive the same cloud key; a different account or
  passphrase cannot decrypt the row.
- Empty account identity and boundary-short passphrases are rejected.
- Cloud nonce/ciphertext tampering and item-id substitution fail.
- Pairing confirmation tags are role-bound and compared in constant time.
- A mismatched SAS decision persists no peer.
- Cancellation and timeout zeroize ephemeral session material.
- No secret appears in diagnostics, events or ordinary logs.

## 5. Module and dependency rules

Use maintained crypto crates for HKDF, AEAD, STREAM framing, password hashing,
constant-time comparison and zeroization. A custom primitive or format requires
an ADR under the dependency exemptions in `AGENTS.md`.

One module owns each stable boundary: derivation, envelope, stream and platform
keystore. The facade exposes typed keys and closed errors, not raw internals.
Tests may independently reproduce a property, but production must not carry a
second parser or second decryption route for the sake of a fixture.
