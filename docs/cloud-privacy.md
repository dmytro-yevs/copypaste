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
  if one appears.
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
- **write rows into the account**, which every device will fetch, try to
  decrypt, and correctly discard.

What bounds that today: the AEAD binds each item's ciphertext to its `item_id`,
so a row cannot be moved onto another item; a version stamped more than a day
into the future is refused rather than allowed to outrank real versions; the
server checks the shape of every row it accepts; and retention runs on a column
no client can forge.

What does **not** bound it yet: an injected row's *metadata* is not
authenticated, so a forged version stamped within the accepted window can still
outrank a real one for an item id it names, and effectively censor that item on
every device. Signing the merge metadata under the sync key is the fix, it is
manifest 05 §5.3's own recommendation, and **it is not implemented**. Until it
is, treat the account password as a secret that matters, and turn on whatever
second factor the deployment offers.

## Turning it off

Signing out clears the account, the tokens and the sync key from the device and
leaves local history untouched. It does not delete rows already uploaded; those
age out within 24 hours.
