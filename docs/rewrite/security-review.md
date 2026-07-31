# Security review — v2, branch `v2-main`

**Date:** 2026-07-30 · **Method:** adversarial source review, read-only · **Scope:**
`copypaste-core`, `copypaste-ipc`, `copypaste-p2p`, `copypaste-cloud`,
`copypaste-daemon`, `copypaste-cli`, `copypaste-ui/src-tauri`.

This tests the claims in `SECURITY.md` against the code. It is not an audit: no
fuzzing, no dynamic analysis, nothing executed on a target platform. Everything
below was read; the one thing that was *run* is the secret-detector regex
extraction behind F-1 and F-2.

### Three caveats on the state of the tree

1. **The tree moved substantially during the review, including on the cloud
   path.** At the start, `cargo check --workspace --all-targets` failed with 17
   errors in `copypaste-daemon` (`Method::Cloud*` added to `copypaste-ipc` with
   no daemon-side `cloud` module). By the end, `crates/copypaste-daemon/src/cloud/`
   had landed and `cargo check --workspace` passes. No test in this document was
   executed; the crypto and detector conclusions were reached by reading, and the
   regex conclusions by extracting the patterns and running them standalone.
2. **`SECURITY.md`'s cloud paragraph is now stale.** It says *"**Not wired into
   the daemon.** The crate is built and tested against mocked HTTP; nothing calls
   it yet."* As of this review's final pass that is false:
   `daemon/src/cloud/{mod,handlers,poll,source}.rs` exist and
   `cloud/source.rs:107` calls `apply_remote_version`. Every cloud finding below
   should therefore be read as live rather than latent. **`SECURITY.md` has since
   been corrected.** (The cloud module landed too late in the review for me to give it
   the same depth as the peer path — see "Suspected, not confirmed".)
3. **Files moved during the review.** `daemon/src/p2p/meta/` became
   `daemon/src/meta/`, and `p2p/source.rs`'s duplicated merge logic was hoisted
   into the new `daemon/src/merge.rs` *while this review was in progress* — the
   first read of `source.rs` and the second returned different files. Line
   numbers were re-pinned immediately before writing, but treat them as
   approximate and anchor on the quoted text.

---

## Summary

| # | Severity | Finding |
|---|---|---|
| F-1 | High | Quoted credential values are systematically invisible to the detector — **closed** |
| F-2 | High | `aws_secret_access_key = …` — the literal `~/.aws/credentials` line — matches no rule — **closed** |
| F-3 | Medium | Peer-supplied `content_hash` is never checked against the content it arrived with — **closed** |
| F-4 | Medium | A remote tombstone clears `pinned`; a local `delete_all` deliberately does not — **decided the other way** |
| F-5 | Medium | The clock-skew ceiling is enforced at one point per transport, not at the shared merge |
| F-6 | Medium | A pairing token is minted, persisted and valid forever, with no expiry and no cap — **closed** |
| F-7 | Low | `SECURITY.md`'s mDNS claim is false: the advertised pairing id *is* a digest of the token — **closed** |
| F-8 | Low | Every inbound TCP connection copies all PSKs into a heap buffer that is never zeroized — **closed** |
| F-9 | Low | TOCTOU window between `bind()` and `chmod 0600` on the daemon socket |
| F-10 | Low | Undecryptable rows vanish from `list`/`search` with no user-visible signal — **closed** |
| F-11 | Low | `--data-dir` does not relocate the device secret — **closed** |
| F-12 | Low | The "purge pass" three comments promise does not exist, and nothing ever re-evaluates `is_sensitive` — **closed for the index; the flag is deliberately still never rewritten** |
| F-13 | Low | `accept_any` does one X25519 per stored pairing per unauthenticated connection; pairings are uncapped — **closed** |
| F-14 | Low | Reassembly buffer reallocates inside `Zeroizing`, leaving peer plaintext in freed heap — **closed** |
| F-15 | Medium | The window is capturable by any screen recorder, so the reveal gesture's ten-second exposure is recordable — **closed** |

Claims verified **sound**: fail-closed crypto and AAD binding (V-1), sensitive
items and the search index (V-2), sensitive items and both sync transports
(V-3), pairing as the only authentication and session poisoning (V-4), one merge
comparator reached by both transports (V-5), no filesystem path in a
user-facing error (V-6). Details in "What held up" below — including the
divergence F-5/V-5 did *not* find, since you asked specifically.

---

## Findings

### F-1 — High — Quoted credential values are systematically not detected · **closed** (`validators.rs` `unquote()`, and a quoted key in `generic_password_kv`)

**Files:** `crates/copypaste-core/src/sensitive/validators.rs:15,71` ·
`crates/copypaste-core/src/sensitive/rules.rs:340-346`

`value_is_strong` rejects any captured value containing `( ) ' " \` < >`
(`CODE_SHAPED_CHARS`, line 15; test at line 71). That rejection is a **v2
addition** — manifest 07 §5.3 specifies three criteria and no code-shape gate —
introduced to kill one benign-corpus entry (`const password =
prompt('enter password:');`). It also rejects every credential that was pasted
with its quotes, which is the ordinary shape of a config file.

Separately, `generic_password_kv` requires `\s*[:=]\s*` **immediately** after
the keyword, so the JSON/YAML form never matches at all: in `"password":` the
character after `password` is `"`, not `:` or whitespace.

I extracted the two regexes and the gate verbatim and ran them (the patterns use
no Rust-specific syntax):

| input | result |
|---|---|
| `"password": "hunter2xyz"` | **no rule matches** |
| `'password': 'hunter2xyz'` | **no rule matches** |
| `password="S3cr3tValue"` | matched → value `"S3cr3tValue"` → code-shaped → **not sensitive** |
| `password = 'S3cr3tValue'` | matched → value `'S3cr3tValue'` → code-shaped → **not sensitive** |
| `export OPENAI_API_KEY="sk-abc123def456ghi789"` | both rules match → quoted value → **not sensitive** |
| `password: hunter2xyz` | detected (control) |

**What follows from a miss.** `capture::ingest` writes `search_text:
Some(content)` for anything not flagged, so the credential lands in
`clipboard_fts` as plaintext — the one table in the schema not covered by the
per-item AEAD. `meta::summaries` advertises it, `meta::fetch` serves it, and
(once wired) `CloudSync::push` uploads it. The blast radius is bounded — FTS
sits inside SQLCipher, the wire is Noise, the cloud row is sealed under
Argon2id — but "never written to the search index" and "never leaves the
device" are exactly the claims this defeats, for the most common paste shape
there is.

**Fix.** Strip one balanced pair of surrounding quotes from group 1 before the
code-shape test (`"S3cr3tValue"` → `S3cr3tValue`), keep the rejection for
*interior* parens/backticks/angle brackets, and add a quoted-key alternative to
the keyword pattern so `"password"\s*:\s*"…"` matches. Add all six rows above to
the true-positive table in `engine.rs`.

### F-2 — High — `aws_secret_access_key = …` matches no rule · **closed** (its own rule at 0.99, `sensitive/rules.rs`)

**File:** `crates/copypaste-core/src/sensitive/rules.rs:340-346` (keyword list)

`aws_access_key` fires at confidence 0.99 on `AKIA…` — which is the *key id*, a
public identifier, not a secret. The 40-character secret access key has no rule
of its own, and the generic keyword rule does not reach it: the alternation does
match the substring `secret`, but the next character is `_`, not `[:=]`, so
`\s*[:=]\s*` fails.

Confirmed by execution:

```
aws_secret_access_key = wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY   ->  NO MATCH
AWS_SECRET_ACCESS_KEY=wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY     ->  dotenv_secret (detected)
```

The lowercase form is the literal content of `~/.aws/credentials`, and copying a
block out of that file is a routine act. The uppercase environment form is
caught only because it happens to end in `_KEY`.

**Fix.** Let the keyword alternation absorb a suffix before the separator —
`(?:…|secret|…)[a-z0-9_-]*\s*[:=]` — or add `secret_access_key`,
`aws_secret_access_key`, `private_token`, `session_token` as explicit keywords.
While there: `glpat-` (GitLab PAT) and `Authorization: Basic <base64>` also match
nothing today; `generic_bearer` covers only `Bearer`.

### F-3 — Medium — Peer-supplied `content_hash` is stored and ordered on, unverified · **closed** (`sync/session.rs`'s `receive_items` recomputes it and drops a live item that disagrees; a tombstone stays exempt)

**Files:** `crates/copypaste-daemon/src/p2p/source.rs:113` ·
`crates/copypaste-daemon/src/merge.rs:121-127,138-146`

`SyncSource::apply` passes the peer's hash straight through:

```rust
// The peer protocol carries the sender's hash, and a tombstone's hash is the
// deleted item's — so it is passed through rather than recomputed…
content_hash: Some(&item.content_hash),
```

and `apply_remote_version` uses it as key 2 of the comparator and writes it into
the row. Nothing ever checks that it is `compute_content_hash(content)`. The
session's `item.summary() != *promised` check (`p2p/src/sync/session.rs:232`)
only proves the item matches the summary *the same peer* sent a moment earlier.

The cloud path, on the same function, **recomputes** the hash (`merge.rs:125-127`,
the `None =>` arm) because a cloud row deliberately carries none. So the two
transports agree only as long as the peer is honest — which is the precise
shape of the INV-C2 class the module exists to prevent.

What a paired-but-hostile peer gets:

* **Targeted refusal.** Pick a hash equal to that of an existing local row in the
  same 60 s dedup bucket. `meta::apply`'s `INSERT` violates `idx_items_dedup`,
  the constraint arm returns `Ok(false)` (`meta/write.rs:72-78`), and the item is
  counted as "skipped". A specific item never lands, and the log line says
  nothing about who caused it.
* **Control of key 2**, letting it win exact-timestamp ties it should lose.
* **Poisoned metadata onward.** The receiver re-advertises the attacker's hash to
  a third device, so two devices holding byte-identical content disagree about
  its hash and the ordering separates versions that are in fact the same.

**Fix.** In `apply_remote_version`, for `!incoming.deleted`, compute the hash
from the content and either reject a mismatch or ignore the supplied value. That
also removes the difference between the two transports, which is the point of the
module.

### F-4 — Medium — A remote tombstone clears `pinned`; a local `delete_all` does not · **decided the other way** (`daemon/src/merge.rs` refuses a remote delete of a pinned row; manifest 05 §3.6 amended to match)

**Files:** `crates/copypaste-daemon/src/meta/write.rs:56-57` ·
`crates/copypaste-core/src/storage/items.rs:149-159`

`Store::delete_all` protects pins deliberately, with a comment and a regression
test: *"Pinning is the one gesture by which a user says 'keep this' — clearing
history must not be the thing that discards it."*

The sync write path takes the opposite decision, silently:

```sql
pinned    = CASE WHEN excluded.deleted = 1 THEN 0 ELSE clipboard_items.pinned END,
pin_order = CASE WHEN excluded.deleted = 1 THEN NULL ELSE clipboard_items.pin_order END
```

So a pin is honoured against the local user and ignored against the network. A
paired device — hostile, compromised, or simply running a build with a bug — can
enumerate the ids it saw in the summary exchange and tombstone every one of
them, pinned included, on every device it can reach. Under `CLAUDE.md` rule 4
that is the worst outcome in the document, reachable from one compromised peer.

The two rules being written in two files with two answers is also exactly the
duplication `CLAUDE.md` rule 1 is about.

**Fix.** Decide it once. Either a remote tombstone preserves the pin (matching
`delete_all`) and only clears the payload, or `delete_all` stops preserving it —
but the same sentence has to be true on both paths, and a test should assert
the pair.

### F-5 — Medium — The skew ceiling is enforced per-transport, not at the shared merge

**Files:** `crates/copypaste-p2p/src/sync/plan.rs:22,41-52` ·
`crates/copypaste-cloud/src/sync/pull.rs:56,98-110` ·
`crates/copypaste-daemon/src/merge.rs:105-146`

**The claim itself holds.** Both transports have the guard, both use
`24 * 60 * 60 * 1000`, both skip a single version rather than failing the round
or deleting anything, and `cloud/src/sync/mod.rs`'s test pins the two constants
equal. Verified.

Two things to record anyway.

**(a) What a bad clock still buys.** The ceiling stops the "stamp `i64::MAX` and
win for ever" case. It does not stop a peer stamping `now + 24 h − ε`, which wins
every comparison for the following day. Within that window a peer can overwrite
the content of, or tombstone, **any item id it knows**, on every device it syncs
with, and the change propagates onward through honest devices because they see a
legitimately-newer version. Combined with F-4 that is a whole-history wipe from
one peer with a wrong clock. This is the accepted trade in manifest 05 R-CLK-2,
but "a device with a broken clock cannot censor an item everywhere" (SECURITY.md)
overstates it: it cannot censor *permanently*; for 24 hours it can.

**(b) The shared merge has no ceiling of its own, and now has two callers.**
`apply_remote_version` performs no skew check. On the peer path the only
enforcement is in `plan`, and it holds today solely because `receive_items` drops
anything whose summary does not equal the one `plan` accepted
(`session.rs:227-238`) — a three-hop argument. The cloud daemon wiring landed
during this review (`daemon/src/cloud/source.rs:107`), so
`apply_remote_version` is now reached from two directions; it is guarded on that
side only because `CloudSync::pull` re-checks before handing the row down. Two
callers, a guard at neither convergence point, and the next caller inherits
nothing.

**Fix.** Move the ceiling into `apply_remote_version`, where both transports
already meet, and leave the `plan`/`pull` checks as the cheap early-out. Same
argument as the comparator: the guard belongs where the two paths converge.

### F-6 — Medium — A pairing token never expires and is never single-use · **closed** (`PAIRING_CODE_TTL` 300 s, burnt by the first completed session; the list is still uncapped — F-13)

**File:** `crates/copypaste-daemon/src/p2p/handlers.rs:59-87`

`pair_create` mints the PSK, writes it to the peer file, and returns the code —
all before the other device has been heard from, which the comment explains is
necessary so the listener can recognise the dial-in. What is missing is the other
half: nothing ever removes it. There is no `created_ms`, no pending flag cleared
on first successful session, no sweep, and `PeerStore` has no cap (`grep` for
`max_peers` in `peers/` returns nothing).

So a code shown on a screen, photographed, pasted into a chat, or simply
abandoned remains a permanent credential for that device — it is not merely a
pairing invitation, it *is* the long-term Noise PSK. `SECURITY.md`'s "it is shown
once" reads as ephemeral; nothing in the code makes it so. The only remedy is
`unpair` by an id the user would have to know they should look for.

**Fix.** Stamp a `created_ms` on a pairing that has never completed a session and
expire it (minutes, not days); drop the entry once a session succeeds, or mark it
established. Cap the pairing list. Surface unused pairings in `peers` so the UI
can show "waiting to pair — expires in N minutes", which is also rule 6.

### F-7 — Low — The mDNS claim is false as written · **closed** (`SECURITY.md` and `discovery/record.rs`'s module doc both now say the id is a one-way digest of the token; `advertisement_carries_the_pairing_id_and_nothing_else_of_the_token` builds its record from a real `PairingToken`, so it can fail on the claim it pins)

**Files:** `SECURITY.md:103` · `crates/copypaste-p2p/src/discovery/record.rs:14-19` ·
`crates/copypaste-p2p/src/transport/token.rs:129-133`

> mDNS advertises only a non-secret pairing id — never a token, and deliberately
> not a digest of one. — `SECURITY.md`

> The pairing token / PSK is never advertised, in any form — not hashed, not
> truncated, not as a "fingerprint". — `record.rs`

But `pairing_id` is exactly that:

```rust
pub fn pairing_id(&self) -> String {
    let digest = blake2s(&[PAIRING_ID_DOMAIN, &self.0[..]]);
    hex::encode(&digest[..PAIRING_ID_LEN])   // truncated to 128 bits
}
```

and the pairing id is what goes in the TXT record as `p0…pN`. The test that
"pins" the property (`record.rs`, `advertisement_has_no_secret_material`) builds
its record from fabricated ids like `"pair-one"`, never from a real
`PairingToken`, so it cannot fail on this.

Practical impact is small — inverting a domain-separated truncated BLAKE2s over a
256-bit CSPRNG input is not a thing — but two consequences are real: someone who
comes into possession of a candidate code can confirm offline which device on the
LAN it belongs to without touching the network, and the pairing ids are stable
public identifiers broadcast on every network a device joins, which links a
device across networks.

**Fix.** Correct both texts to say what is true — a one-way, domain-separated,
truncated digest, chosen so the id cannot be used as a credential — and make the
test derive its ids from a real token so the claim it pins is the claim the code
makes.

### F-8 — Low — PSKs are copied unzeroized on every inbound connection · **closed** (`PeerStore::psks` returns `Zeroizing<Vec<PskCandidate>>`, so the hazard cannot be re-created by the next caller; `psks_are_handed_out_wiped_on_drop`)

**Files:** `crates/copypaste-p2p/src/peers/store.rs:143` ·
`crates/copypaste-daemon/src/p2p/mod.rs:205`

`psks()` returns `Vec<(String, [u8; TOKEN_LEN])>` and documents that the copies
are "**not** zeroized for you". `serve_peer` takes it as a plain local:

```rust
let candidates = state.p2p.peers().psks();
```

The `Vec` is dropped at the end of the function without wiping. This runs for
**every** inbound TCP connection, before any authentication, so an unauthenticated
attacker can force the whole PSK set to be copied into and abandoned in freed heap
as fast as they can open sockets. `Peer` itself is careful (`Drop` + `zeroize`,
constant-time compare, redacted `Debug`); this one call site undoes it.

**Fix.** `let candidates = Zeroizing::new(state.p2p.peers().psks());` — or better,
have `psks()` return `Zeroizing<Vec<…>>` so the hazard cannot be re-created by the
next caller.

### F-9 — Low — TOCTOU between `bind()` and `chmod` on the daemon socket

**File:** `crates/copypaste-daemon/src/server/listener.rs:47-49`

```rust
let listener = UnixListener::bind(path).context("bind the daemon socket")?;
std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
```

Between the two the socket exists at `0777 & ~umask`. `connect(2)` on a Unix
socket needs write permission, so the usual `umask 022` (mode 0755) is not
exploitable — but `umask 002`, which is the default on several distributions and
inside a lot of container images, gives 0775 and lets any process in the user's
group connect during the window and read the entire clipboard history. The
parent-directory `0700` that would otherwise cover this is applied warn-only
(line 41-43), so it is not a guarantee.

**Fix.** Set `umask(0o177)` around the bind and restore it, or bind to a
temporary name inside the (already `0700`) directory and `rename` it into place.

### F-10 — Low — Undecryptable rows disappear silently from every read path · **closed** (`ItemPage::skipped_undecryptable`, shown by `history/SkippedNotice.tsx`)

**Files:** `crates/copypaste-daemon/src/server/items.rs:185-195` ·
`crates/copypaste-ui/src-tauri/src/backend/embedded.rs:196-210`

`decrypt_rows` / `to_wire_page` drop any row that fails to open, with a `warn!`
and nothing else. Failing closed is right, and not blanking a whole page for one
bad row is right. The problem is that the user is never told anything: the item
is simply not in the list.

Two consequences. An attacker with write access to the database file — the same
uid, which is the stated trust boundary, but also anything that can touch a
backup — can make chosen items vanish from the UI by flipping one ciphertext bit,
and the row survives on disk so nothing reports corruption. And a genuine
key-derivation or storage problem presents to the user as "my history is
quietly shrinking" rather than as an error.

**Fix.** Return a placeholder item flagged unreadable (the wire `Item` already
has room), or at minimum surface a count in `status` so a UI can say "3 items
could not be read".

### F-11 — Low — `--data-dir` does not relocate the device secret · **closed** (`Keyring::load_or_create` takes the data directory; `crypto/keystore/mod.rs` additionally refuses a directory holding a history database but no secret, rather than minting into it — `a_database_without_its_secret_is_refused_rather_than_re_keyed`)

**Files:** `crates/copypaste-core/src/crypto/keystore.rs` (`secret_path`) ·
`crates/copypaste-ipc/src/lib.rs:284-288` · `crates/copypaste-daemon/src/main.rs:224-236`

`--data-dir` moves the database, the socket and the peer file. It does not move
the device secret: `secret_path()` resolves
`ProjectDirs::from("com", "copypaste", "copypaste")` unconditionally, and takes no
argument. Note also that the qualifier differs in case from
`copypaste_ipc::data_dir`'s `("com", "copypaste", "CopyPaste")`, so on macOS the
secret and the database live in two different `Application Support` directories.

`main.rs` documents `--data-dir` as running an instance "fully isolated from the
user's real history". It is isolated in contents; it is not isolated in key
material — a demo or test daemon reads and, on first run, *creates* the real
user's device secret, and its database is keyed identically to the real one.

**Fix.** Thread the data directory into `Keyring::load_or_create`, and settle on
one `ProjectDirs` qualifier triple in one place.

### F-12 — Low — The promised "purge pass" does not exist, and `is_sensitive` is never re-evaluated · **closed** (`core/src/sensitive/purge.rs`, run from `daemon/src/main.rs` before the socket binds)

The pass removes from the index and nothing else. It deliberately does **not**
rewrite `is_sensitive`, which is the half of this finding that stays open by
decision rather than by omission: `sweep_sensitive` selects on that flag, so a
re-derived flag would hand a changed ruleset a hard delete over data the user
never reviewed. The consequence to accept is that a row the current ruleset
would flag stays listable and stays syncable; only its plaintext leaves the
index.

**Files:** `crates/copypaste-ipc/src/lib.rs:250` ·
`crates/copypaste-daemon/src/server/items.rs:225` · `CLAUDE.md` rule 4

Three texts promise a third enforcement layer:

> excluded from the search index at write time, at read time, and by a purge
> pass. — `copypaste-ipc`
>
> Sensitive items must never reach the search index. Enforced at write time, at
> read time, and by a purge migration for databases predating the rule. —
> `CLAUDE.md` rule 4

`grep -rni "purge|reindex|rescan"` across the workspace returns those two
comments and nothing else. `schema.rs` has one migration and no purge step.

Under rule 3 this is defensible — there is no v1 data to purge and v2's schema is
one version old — and `SECURITY.md` has already quietly dropped the claim to two
layers. But the substantive gap remains: `is_sensitive` is computed once, at
capture, and nothing ever revisits it. When the ruleset gains a rule (or F-1/F-2
are fixed), every already-captured item stays unflagged and its plaintext stays in
`clipboard_fts` and keeps syncing, with no mechanism to correct it.

**Fix.** Either add the rescan-and-purge pass and keep the sentence, or delete the
sentence from all three places and record the consequence as a known limitation.

### F-13 — Low — Unauthenticated CPU amplification in `accept_any` · **closed** (`MAX_PAIRINGS`, enforced in `PeerStore::upsert` by refusing a *new* pairing rather than evicting an old one — `the_pairing_list_is_capped_by_refusing_not_by_evicting`. The mDNS-hinted ordering was not built; the cap alone bounds the multiplier.)

**File:** `crates/copypaste-p2p/src/transport/handshake.rs:141-163`

The responder replays the first handshake frame against every stored PSK in
turn, building a Noise responder and performing the `psk, e` read per candidate.
That is one X25519 operation per stored pairing, per inbound connection, before
anything is authenticated. Pairings are uncapped (F-6), so the multiplier is
whatever has accumulated. `MAX_CONCURRENT_PEER_SESSIONS = 4` and the 10 s
handshake timeout bound it, so this is a nuisance rather than a real DoS — but
the cost of an anonymous TCP connect should not scale with a local list an
attacker can grow by social means.

**Fix.** Cap stored pairings; consider an mDNS-hinted candidate ordering so the
common case is one trial. (Do **not** add a wire hint that identifies which
pairing is being attempted — that would undo the "reveals nothing about which
pairings this device holds" property, which is currently correct.)

### F-14 — Low — Reassembly leaves peer plaintext in freed heap · **closed** (`Reassembly` starts at `MAX_NOISE_PLAINTEXT` and grows by moving into a fresh `Zeroizing<Vec>` and dropping the old one, which wipes it; the ceiling is checked before anything is copied or reserved, so the size comes from our limit and not from what the peer declares)

**File:** `crates/copypaste-p2p/src/transport/session.rs:150,202`

```rust
let mut message: Zeroizing<Vec<u8>> = Zeroizing::new(Vec::new());
…
message.extend_from_slice(body);
```

`Zeroizing` wipes the buffer it finally owns. It does not wipe the buffers
`Vec`'s growth left behind: each reallocation copies the plaintext accumulated so
far into a new allocation and frees the old one intact. This is precisely the
hazard the same file avoids deliberately for the staging buffer — *"a growing
`Vec` leaves copies of old plaintext in freed heap that nothing will ever
zeroize"* (line ~88). A multi-record message can leave ~2× its size in
un-wiped copies, up to the 32 MiB cap.

**Fix.** Allocate once (`Vec::with_capacity(MAX_NOISE_PLAINTEXT)` up front, grown
to `MAX_MESSAGE_BYTES` on the first `RECORD_MORE`), matching what `plain` and
`cipher` already do.

---

### F-15 — Medium — Nothing kept the window out of a screen recording · **closed** (`contentProtected` in `tauri.conf.json`, `FLAG_SECURE` in `MainActivity.onCreate`, and a user-reachable opt-out behind both)

**Files:** `crates/copypaste-ui/src-tauri/tauri.conf.json`,
`src-tauri/src/shell/protection.rs`,
`gen/android/app/src/main/java/com/copypaste/app/{MainActivity,ScreenProtectionPlugin}.kt`.

Not found by this review — parity S11 and ui-parity §2.4 both had it, against
INV-35's "on by default". Recorded here because of what it defeats rather than
what it discloses: sensitive items are withheld from the list and revealed for
ten seconds behind a confirmation (INV-10, INV-11), and a recorder makes that
whole mechanism decorative. The history itself is the more ordinary half.

Two mechanisms, because there is only one API and it does not reach both
platforms. `WebviewWindow::set_content_protected` dispatches to tao's
`Window::set_content_protection`, which is
`#[cfg(any(target_os = "macos", target_os = "windows"))]` — `NSWindowSharingNone`
on macOS, and **nothing at all** on Android. Android's half is therefore
`FLAG_SECURE`, set in Kotlin, which additionally blanks the recents thumbnail.

**Why on-by-default with an opt-out, and not conditional on a reveal.** Only the
opt-out model needs no ordering guarantee: protection is applied at window
creation on both platforms, so it is already on before any plaintext can render,
and every failure path — a plugin that did not register, a window that will not
answer, a preference that would not load — leaves it on. A model that protected
only while something sensitive was on screen would have to win a race against a
React render on every reveal, and would still leave the list unprotected, which
is a clipboard history and therefore the secret.

**What is not established.** Neither line has executed. `NSWindow.sharingType`
and `FLAG_SECURE` are API readings against the tao and AOSP contracts, not
observations.

---

## What held up

Recorded because a claim checked and found true is a result.

### V-1 — Fail-closed crypto and AAD binding — sound

* One seal and one open (`core/src/crypto/aead.rs`). Every attacker-influenceable
  failure collapses to `CryptoError::AuthFailed`; only a structurally wrong nonce
  length is distinguishable, and that is not an oracle over key or content.
  There is no retry, no second key, no "try without AAD", and no degraded read
  anywhere in the tree.
* The AAD is length-prefixed (`…|<len>:<item_id>`), so `"item"` cannot
  authenticate a ciphertext bound to `"item-2"` — tested, including the
  delimiter-abuse injectivity case.
* **Every production call site binds the row's own logical item id.** Five of
  them: `capture.rs` seals with an id chosen *before* the insert precisely so the
  written and read AAD cannot differ; `server/items.rs::to_wire` opens with
  `row.id`; `merge.rs::open_version` and `merge.rs::apply_remote_version` use
  `row.item_id` / `incoming.item_id`; `ui/backend/embedded.rs::to_wire` uses
  `row.id`. `StoredItem::id` and `StoredVersion::item_id` are the same column
  (`meta/read.rs` selects `ci.id`). **There is no path that seals under one id
  and opens under another.**
* Nonces are 24 bytes from `OsRng` on every call, never a counter; tested for
  uniqueness and against all-zero.
* SQLCipher is keyed with the raw `x'…'` form before any other statement, on both
  the pool's `with_init` and the second `Meta` connection, and `validate_key`
  proves the key before anything else touches the file — a wrong key is
  `InvalidKey`, never an unkeyed retry (`storage/connection.rs:98-106`,
  `meta/open.rs:131-139`).
* Keys: one HKDF extract, two expands with distinct `info`, neither key is the
  stored secret. `Keyring`/`ItemKey`/`SyncKey`/`PairingToken` all have redacted
  `Debug`, `Zeroizing` storage and constant-time comparison, with `PartialEq`
  routed through `subtle` so no caller can reach a short-circuiting compare.

### V-2 — Sensitive items and the search index — sound

Three layers confirmed, all present and all reachable:

1. write guard — `storage/items.rs:27-36` drops `search_text` on a sensitive item
   *whatever the caller passed*, and logs that it did;
2. in-transaction re-read — `storage/search.rs:54-64` re-queries `is_sensitive`
   inside the transaction that writes the index row;
3. read predicate — `storage/search.rs:39` joins `ci.is_sensitive = 0` in the SQL
   itself, so a row planted directly in `clipboard_fts` can never surface. Both
   clients then filter again on read (`server/items.rs:67`,
   `ui/backend/embedded.rs:270`).

**I looked for the third path you asked about and found a fourth writer, which
also enforces:** `meta/write.rs:85-96` is a second, independent `INSERT INTO
clipboard_fts` on the sync-apply path. It does an *unconditional* `DELETE`
first and gates the `INSERT` on `!deleted && !is_sensitive`, so it cannot leave a
stale row or index a flagged one. There is no bulk-insert, repair or reindex
routine that bypasses any of this — because no such routine exists at all (F-12).

### V-3 — Sensitive items and both transports — sound, including the planning half

**Peer path, four layers:** `meta::summaries` filters `is_sensitive = 0` in SQL
(`meta/read.rs:30`); `meta::fetch` filters again (`meta/read.rs:127`);
`serve_items` refuses any requested id outside the advertised set
(`p2p/src/sync/session.rs:283-292`); and it refuses again anything the *source*
hands back outside that set (`session.rs:301-306`), with a test that smuggles an
item into the source to prove it.

**The planning half is covered structurally.** A sensitive item never enters
`advertised`, so it cannot be named in a `Request`, cannot be fetched, and cannot
be batched — the peer is not told it exists, and a peer that learned the id from
an earlier session (before the item was flagged) gets nothing, which
`an_item_that_was_never_advertised_cannot_be_requested_out` exercises directly.

**Cloud path, two layers:** `meta::versions_since` applies the same SQL filter,
and `SensitiveGuard` runs before anything is sealed or counted
(`cloud/src/sync/push.rs:44-51`). The guard is a required argument to
`CloudSync::new`, so a driver that uploads unchecked cannot be constructed —
that is the AT-56 hole made unrepresentable rather than remembered.

Sensitivity is re-decided by the *receiver's* detector on apply
(`merge.rs:150-155`), and a tombstone inherits the flag from the row it deletes
so that deleting a flagged item does not publish its hash. Both correct.

### V-4 — Pairing is the only authentication — sound

* `Noise_NNpsk0_25519_ChaChaPoly_BLAKE2s`, with the PSK mixed into the chaining
  key before message one's payload, so a wrong code is an AEAD failure on the
  very first message. There is no unauthenticated mode in the file to fall back
  to, and no branch that could offer one. Tested from both sides.
* `accept_any` tries every candidate, continues past failures, and returns an
  identical `TransportError::Handshake` whether zero candidates matched or the
  list was empty — nothing about which pairings the device holds leaks to the
  dialler, and the daemon's logging is equally silent.
* **Poisoning holds on every failure path** (`transport/session.rs:161-226`):
  short frame, oversized frame, AEAD failure, `len < RECORD_HEADER_LEN`, unknown
  record marker, and EOF mid-message all set `poisoned` before returning, and
  both `send` and `recv` check it first. A desynchronised nonce cannot be
  continued past. A *parse* failure on authenticated bytes correctly does **not**
  poison — that is a payload problem, not a channel problem.
* Peer key file: `0600` set on the temp file before any bytes are written,
  `write_all` → `sync_all` → `rename` → directory fsync, with the in-memory map
  rolled back if the write fails, so a caller is never told a pairing was saved
  when it was not. A corrupt or legacy file is reported and **never** overwritten.
  Modes are asserted after both a first write and a rewrite.
* Constant-time PSK comparison, all-zero PSK refused at validate, `Peer` zeroizes
  on drop and cannot have its key moved out. (Except at the one call site in F-8.)

### V-5 — One merge comparator, both transports reach it — sound, no code divergence

You asked specifically whether any *code* diverges alongside the comment. It does
not.

* `merge_decision` (`p2p/src/sync/merge.rs`) is the only comparator.
  `merge_decision_by_summary` is defined as *that function with key 4 held equal*
  — not a second implementation — and a test asserts the equivalence across the
  entire decision space (3 timestamps × 3 hashes × 2 delete states × 3 origins,
  both directions). Symmetry and the strict four-key total order are also pinned
  against a tuple comparison, so transitivity and order-independence follow.
* In the daemon there is now **exactly one call site**
  (`daemon/src/merge.rs:138`), and both `p2p::source::apply` and the cloud source
  reach it through `apply_remote_version`. This is the refactor that landed
  during the review; before it, `p2p/source.rs` carried a second copy of the same
  logic (still calling the same comparator, so still not a divergence).
* The prose is wrong in **two** places, not one. Besides
  `cloud/src/sync/mod.rs:21-26`, the contract paragraph on
  `CloudSource::apply_remote` (`cloud/src/sync/source.rs:69-71`) also tells the
  implementor the order is "`created_at`, then `content_hash`, then
  `origin_device_id`" — omitting `deleted`. That is the paragraph someone writing
  a `CloudSource` would actually follow, and following it reintroduces the
  delete-resurrection bug (INV-N2 / `CopyPaste-ojhe`). Worth fixing in the same
  pass as the header comment.

The one substantive gap in this area is F-3: same comparator, but the *inputs*
to key 2 are computed differently by the two transports, and one of the two
trusts the peer.

### V-6 — No filesystem path in a user-facing error — sound

* The daemon's entire client-visible error vocabulary is ten `&'static str`s
  gathered in one file specifically so the completeness test is provable
  (`server/messages.rs`), plus eight more in `p2p/handlers.rs` with the same
  treatment. `storage_error` and `decrypt_error` log the real error and return a
  fixed sentence.
* `copypaste-p2p`'s `ProtocolError` and `SyncError` carry only field names, byte
  counts and `&'static str`s. `copypaste-cloud`'s `SyncError` has no `String`
  field at all, which makes the property structural.
* Every client surface routes daemon text through the shared `scrub_paths`
  (`cli/src/error.rs:93-95`, `cli/src/render.rs:196`,
  `ui/src-tauri/src/backend/error.rs:84,92`), and the CLI never formats
  `socket_path()` into anything.
* **I looked specifically for the case you named** — an error built inside
  `copypaste-core` or `copypaste-p2p` and surfaced through IPC, or a
  `std::io::Error` interpolated into a message. I found none in production code:
  the only `.display()` / `to_string_lossy()` uses in the tree are in tests and
  in `main.rs`'s hostname read. (Rust's `std::fs` deliberately does not attach
  paths to `io::Error`, so the usual accident is not available here either.)

Two properties of `scrub_paths` worth writing down rather than fixing:

* It is whitespace-token based, so a macOS path is redacted only because the
  username sits in the first token:
  `/Users/alice/Library/Application Support/CopyPaste/daemon.sock` →
  `<path> Support/CopyPaste/daemon.sock`. The username is gone, which is the
  property that matters, but the redaction is partial and a path whose sensitive
  component fell after a space would survive.
* It does not recognise a bare relative path (`copypaste-v2.db`,
  `Library/Application Support/…` with no leading separator).

---

## Suspected, not confirmed

* **F-5(b) reachability.** I argued from the source that a future-stamped item
  cannot reach `apply` on the peer path today; I could not execute the test suite
  to demonstrate it, because the workspace does not build.
* **The daemon-side cloud module.** `daemon/src/cloud/{mod,handlers,poll,source}.rs`
  appeared in the last minutes of the review. I confirmed only that
  `cloud/source.rs` routes through the shared `apply_remote_version` and
  `open_version` (so V-5 still holds) and that it reads through `Meta`, whose
  queries filter `is_sensitive = 0` (so V-3 still holds). **The rest of it —
  `handlers.rs` (which takes an email, a password and a passphrase over IPC),
  `poll.rs`, and wherever the derived `SyncKey` is persisted — is unreviewed.**
  Given `Method::CloudSignIn` carries three secrets in one request, that module
  should get its own pass.
* **`copypaste-cloud` itself.** The Argon2id parameters, the per-account salt
  derivation and the row AEAD all read as correct; none was exercised against a
  live backend. One note: the Argon2id salt is *derived deterministically from
  the account id* rather than random-and-stored, so it is predictable to anyone
  who knows the account — acceptable at 19 MiB / t=2 with a 12-character
  minimum, but it does make precomputation against a *specific* account possible
  in a way a random salt would not.
* **macOS Keychain and NSPasteboard.** Never compiled, per `SECURITY.md`. Read
  only. The Keychain backend's fail-closed logic (only `errSecItemNotFound`
  mints) reads correctly; `-25308`/`-25293`/`-34018` are all surfaced. **Since
  amended:** the `macos-keychain` cargo feature that guarded it — and that no
  release script ever passed — is gone; `security-framework` is a plain
  target-gated dependency and the backend is selected by `target_os` alone.
* **Android Keystore.** *Was* "not implemented; the `0600` file backend is what
  would ship today". **Since built** —
  `core/src/crypto/keystore/android.rs`, target-gated the same way: an AES-GCM
  key in the Keystore wraps the device secret and the blob sits in app-private
  storage, because the Keystore holds keys and not blobs. Still never compiled —
  no NDK here — so it moves from "not implemented" to the unverified pile rather
  than out of this section.
* **DoS accounting.** Per-session memory looks bounded (≤ 8 items × 4 MiB per
  message, 32 MiB reassembly cap, 4 concurrent sessions ⇒ order 128 MiB), and
  the mDNS peer table is capped at 256 with LRU eviction and a TTL. I did not
  measure any of it.
* **`is_sensitive` on very large items.** The detector runs ~42 regexes over the
  full content of every captured *and every received* item, with no size cap. The
  `regex` crate is linear so this is not catastrophic backtracking, but a session
  can hand a peer up to 1 000 items of up to 4 MiB each to scan inside
  `block_in_place`. Bounded by `SESSION_TIMEOUT` (120 s) rather than by anything
  in the detector. Not measured.
