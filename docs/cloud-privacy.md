# What cloud sync discloses

Cloud sync is optional and off until an account is signed in. This page says
what the backend can see when it is on, what it cannot, and how that differs
from the v1 relay it replaces. It is the documentation manifest 05 §5.4 item 3
asks for, and its claims are the ones `supabase/tests/01_schema_audit.sql`
asserts against the schema.

Peer-to-peer sync and local history disclose nothing to any server.

## What the backend stores

One row per clipboard item, in one table:

| Column | Visible to the backend |
|---|---|
| `ciphertext`, `nonce` | **Opaque.** Sealed on the device with XChaCha20-Poly1305 under a key derived from a passphrase that never leaves it. |
| `item_id` | A random per-item identifier, stable across devices. |
| `user_id` | The account. |
| `content_type` | `text`, or the kind of item. |
| `origin_device_id` | Which of your devices produced this version. |
| `created_at` | When that version was written, to the millisecond. |
| `deleted` | Whether this version is a deletion. |
| `signature` | An authentication tag over all of the above. The backend cannot produce it or check it. |
| `inserted_at`, `updated_at` | Server clock, for retention. |

Plus, in the account service: **the email address** the account was created
with, and a session per signed-in device.

So the backend learns the **size** of each item, its type, which device wrote
it, when, how often you copy things, and when you delete them. It learns
nothing about content.

## What the backend cannot do

- **It cannot decrypt an item.** The sync key is derived from your passphrase
  with Argon2id, on the device. It is never sent, and the account password is
  not it.
- **It cannot search or compare content.** There is no content column, no
  content hash, no full-text index. `tests/01_schema_audit.sql` fails the build
  if one appears. The `signature` column is not an exception: it authenticates
  the *ciphertext* and the metadata, never the plaintext, so it says nothing
  about whether two items are the same.
- **It cannot forge a version.** The fields sync orders on are in the clear
  because the backend pages on them, but they are signed on the device under a
  key derived from the same passphrase. See below.
- **It cannot see items marked sensitive.** Anything the detector flags — a
  password, a key, a card number — is withheld from upload entirely, at two
  independent points. It never leaves the device that captured it.
- **It cannot read another account's rows**, and neither can you: row-level
  security pivots on `user_id` and is enforced *and* forced.

## One account is one trust circle

Row-level security is scoped to the account, not to a device. Every device
signed into an account reads every other device's rows. There is no per-device
partition and no way to share a subset.

## The server forgets within a day

Rows are deleted 24 hours after they arrive, and only the newest 500 per account
are kept. Both are enforced on the server clock, which no client can write.

The consequences are worth knowing:

- A device offline for more than a day misses whatever aged out. **Local history
  is the durable copy; the cloud table is a transit buffer.**
- A new device sees at most the last 24 hours, never the full history.
- An operator with a copy of the database has at most a day of ciphertext, not a
  corpus to attack offline later.

## What is worse than v1, and by how much

v1 synced through a relay that knew nothing but an opaque inbox identifier
derived from the sync key, and required no account. Supabase requires an account
and sees the metadata listed above. This is a **real regression in metadata
privacy**, taken deliberately: the relay was 12,000 lines of server we
maintained, and the alternative to running it was not running anything.

Content confidentiality is unchanged — end-to-end encrypted in both.

There is a second consequence, and it is the sharper one. In v1 the secret that
reached the data and the secret that decrypted it were the same secret. They are
now different: the **account password** gates access to the rows, and the **sync
passphrase** decrypts them.

Someone who compromises the account but not the passphrase therefore can:

- read all ciphertext and all metadata — but not decrypt anything;
- **write rows into the account**, which every device will fetch and refuse.

## The metadata is signed, and that is what makes "refuse" true

Encryption protects content. It does nothing for the fields sync *orders* on —
`item_id`, `created_at`, `deleted`, `origin_device_id`, `content_type` — because
the backend has to sort and page on them, so they travel in the clear. Left
unauthenticated, whoever can write to the table decides which version of an item
every device keeps: stamp a competing version a minute into the future and the
real one loses; stamp a tombstone and the item disappears everywhere.

So every row carries an HMAC over all of those fields plus the ciphertext and
the nonce, under a key derived from the sync passphrase with HKDF — a separate
key from the one that encrypts, from the same secret. A device verifies before
the row reaches the merge and refuses what does not verify: it is not applied,
not counted as a delete, and never allowed to displace a local version.

Each round counts its refusals, and a non-zero count means something wrote a row
into your account that does not hold your passphrase. **That count does not
reach the app yet** — the sync engine reports it, the daemon's status message
does not carry it, so today it is only in the log. Sync itself is not affected;
what is missing is the ability to be told.

Because the ciphertext is under the signature too, a real version cannot be
spliced onto another version's stamp — an attacker cannot roll an item back to
older content while keeping the newer metadata.

What this does **not** stop: someone with the account password can still read
all the metadata, and can fill the table with rows every device throws away.
Treat the account password as a secret that matters, and turn on whatever second
factor the deployment offers.

The rest of what bounds an injected row is unchanged: the AEAD binds each item's
ciphertext to its `item_id`, so a row cannot be moved onto another item; a
version stamped more than a day into the future is refused rather than allowed
to outrank real versions; the server checks the shape of every row it accepts,
and refuses one with no signature at all; and retention runs on a column no
client can forge.

## Turning it off

Signing out clears the account, the tokens and the sync key from the device and
leaves local history untouched. It does not delete rows already uploaded; those
age out within 24 hours.
