# Post-merge review — `fa514730..2808a8ab`

Adversarial read of the ~37 commits that landed on `main` on 2026-07-30, done
against a clean checkout of `2808a8ab` (`git archive HEAD` into a scratch tree,
so the concurrent edits in the working tree could not confuse a result).

**Baseline established first, so that "it fails" means something.** On that
clean tree: `cargo +1.96 test --workspace` passes (exit 0), `npm test` in
`crates/copypaste-ui` passes (24 files, 246 tests), `npm --prefix design run
check` passes (828 contrast pairs, 77 files through the usage gate). Everything
below is a defect that a green suite does not catch.

`docs/backlog.md` landed inside this range and already registers four of the
gaps found here (B-4, B-13, B-15, B-18, B-23). Those are marked **tracked**
below and ranked down accordingly — with two exceptions where the backlog's own
description of the gap is wrong, which is called out at the finding.

---

## 1. Data loss — an import can hard-delete the history · **reproduced**

`crates/copypaste-core/src/ingest.rs:170-175`

```rust
if settings.retention_days > 0 {
    let cutoff = created_at - i64::from(settings.retention_days) * 86_400_000;
    if let Err(e) = store.evict_older_than(cutoff) { … }
}
```

`created_at` is the **item's own** stamp, which on the import path is read
straight out of a user-supplied JSON file and validated nowhere:
`server/transfer.rs:130` passes `item.created_at` to `ingest_at`, and
`ipc/src/payload.rs:32` declares it as a bare `i64`. Nothing between the file
and this line bounds it.

So a single imported row stamped in the future drags the retention cutoff into
the future with it, and `Store::evict_older_than` is a **hard** delete —
`retention.rs:263-275` is `DELETE FROM clipboard_items`, not a tombstone. There
is nothing to restore from.

**Sequence that breaks it.** `retention_days = 30` (a setting the CLI offers:
`copypaste config set --retention-days 30`), then import a file containing one
item whose `created_at` is 60 days ahead. Cutoff becomes `now + 30 days`; every
unpinned live row is older than that; all of them go. Pinned rows survive
(`WHERE … pinned = 0`), which is the only thing limiting the blast radius. The
eviction runs after *each* imported item, so a bad row in the middle of a
10,000-item import also takes the rows imported before it.

**Reproduction** (added to the scratch copy of `ingest.rs`, not to the repo):

```
assertion `left == right` failed: the five real captures should still be here …
  left: 1
 right: 6
```

Five ordinary captures plus one future-stamped import leaves **one** row.

**Why it survived review.** Both sync transports guard exactly this — a stamp
past `now + 24 h` is refused at `p2p/src/sync/plan.rs:36` and
`cloud/src/sync/pull.rs:130`. The import path is the third writer and inherits
neither, which is the shape `docs/backlog.md` B-9 describes and then dismisses:

> "Both transports do check, so this is a latent hole rather than a live one."

That assessment is wrong. There is a third caller, it is reachable from the UI's
import button, and its failure mode is an unrecoverable delete rather than a
mis-ordered row. **B-9's severity should be raised, not its position.**

**Scope note, stated plainly:** this code predates the range — `git show
c427bfeb^:crates/copypaste-daemon/src/capture.rs:308` has the same three lines.
Commit `c427bfeb` moved it verbatim into `copypaste-core` and *exported it*
(`pub use ingest::{ingest, ingest_into, …}`), so today's change is that an
unvalidated-timestamp hard-delete is now a public API of the core crate, aimed
at an Android client that has no daemon-side validation in front of it. It is
first here because of consequence, not because of novelty.

**Suggested fix:** clamp in `ingest_into` (one ceiling, `copypaste-core`, which
is also where B-9 says `MAX_FUTURE_SKEW_MS` belongs), or compute the retention
cutoff from `now_ms()` rather than from the item's stamp. The second is a
one-word change and is almost certainly what was meant — retention is a
statement about wall-clock age, not about the age of whatever was last written.

---

## 2. The v0.4 refusal cannot fire in any shipping path · verified by call-site audit

`crates/copypaste-core/src/storage/legacy.rs`, `storage/store.rs:43`,
`storage/dbfile.rs:35`

Commit `153213d4` says:

> "Rule 3 obliges v2 to say plainly that it found a v0.4 history rather than
> report corruption. … Identify a v1 file positively … and refuse it before
> PRAGMA key."

The identification itself is careful and correct — read-only probe, positive
schema match, honest about the renamed-encrypted edge. The problem is that
**nothing in a shipping build ever asks it the question that matters.**

* `is_v1_database` is consulted from exactly two places: `Store::open` and
  `open_validated`.
* `Store::open` is called on one path in production —
  `daemon/src/main.rs:371`, with `db_path` from `main.rs:342-343`, which is
  always `<data_dir>/copypaste-v2.db`. A v0.4 file is called `clipboard.db`.
  The check therefore evaluates a file that by construction cannot be the old
  one, and returns `false` every time.
* `open_validated` has **no non-test caller at all** (see finding 6).
* `v1_database_in(dir)` — the function actually shaped for the real question,
  "is there an old history sitting beside the new one?" — has **zero callers**
  anywhere in the tree.
* `StoreError::LegacyDatabase` is referenced only inside `copypaste-core`. No
  `ErrorCode`, no `BackendError` variant, no catalogue string.

So the actual user experience on upgrade from v0.4.1 is unchanged by this
commit: the daemon creates a fresh `copypaste-v2.db`, says nothing, and the
user's history appears to have vanished.

`docs/backlog.md` B-4 describes this as half-closed — "recognises a v0.4 history
… Nothing carries it outward". That understates it. The recognition is not
merely un-surfaced; it is never *run* against a v0.4 file. Wiring a screen to
`StoreError::LegacyDatabase` would not fix this on its own, because the daemon
never produces that error. What is missing is a `v1_database_in(&data_dir)` call
at startup.

Not reproduced end-to-end — a live daemon run was not possible, see
"What I could not verify". The call-site audit is conclusive on its own: there
are only four references and none of them is on the path.

**In flight, and it will not close this.** While this review was being written
an agent added `ErrorKind::"legacy_database"` and a matching catalogue string to
`ui/src/lib/errors.ts`, plus (from the diff) an `ErrorCode::LegacyDatabase` on
the wire. That is B-4's surface half and it is the right work. It does not close
the gap: `v1_database_in` still has zero callers in the working tree, and
`Store::open` is still only ever handed `copypaste-v2.db`, so the daemon has no
way to *produce* the error the new screen is waiting for. Whoever owns that
change should add the startup probe in the same stretch of work, or the result
is a rendered state that nothing can enter.

---

## 3. Restore installs a search index that no ADR-015 layer has seen

`crates/copypaste-daemon/src/server/dbadmin.rs:247`, `main.rs:381`

`swap()` repopulates the live index with a raw
`INSERT INTO clipboard_fts (id, content_text) SELECT … FROM restore_src…`.
That bypasses layer 1 (`Store::insert`'s write guard) and layer 2
(`upsert_fts_in_tx`'s in-transaction re-read) — it is the only writer to
`clipboard_fts` that goes around both.

The new purge that would catch it (`purge_indexed_secrets`) runs **only at
daemon start**, `main.rs:381`. Restore is a live IPC operation and does not call
it, so between a restore and the next daemon restart the index holds whatever
the backup held.

Consequence is bounded and worth stating precisely: layer 3 survives — `search`
joins `ci.is_sensitive = 0` (`storage/search.rs:49`) — so a flagged item's text
cannot be *returned* by a search. What is left is plaintext at rest in
`clipboard_fts`, the one table not under the item AEAD, for rows the restore
brought in. That is a weaker property than the three-layer story `search.rs`
opens with, and it is a new gap because the purge is new.

Not reproduced (needs a running daemon). Verified by reading the two call sites;
the absence of any purge call on the restore path is a grep result, not an
inference.

The same argument applies, more weakly, to `import`: it goes through
`ingest_into`, so layers 1 and 2 do hold, and there is no gap.

---

## 4. Every peer-sync failure renders as the same generic sentence · **reproduced**

`crates/copypaste-ui/src/lib/errors.ts:37-58`, `backend/error.rs:99-108`,
`p2p/src/node/error.rs:9-32`

`copypaste-p2p` authors nine distinct sentences, several with a remedy in them.
The daemon maps them onto `ErrorCode` and passes the text through
(`daemon/src/p2p/handlers.rs:197-203`). `BackendError::from_code` keeps the text
as `Daemon(String)`. Then `classifyError` in the WebView matches it against a
pattern list — and the list was written against `BackendError`'s *own*
sentences, not the node's.

Running every `NodeError` message through the classifier (regexes transcribed
verbatim from `errors.ts`, script kept in scratch):

| `NodeError` | classified | what the user reads |
|---|---|---|
| `BadCode` | `unknown` | The background service returned an error. |
| `BadAddress` | `unknown` | The background service returned an error. |
| `Handshake` | `unknown` | The background service returned an error. |
| `NoAddress` | `unknown` | The background service returned an error. |
| `Session` | `unknown` | The background service returned an error. |
| `Timeout` | `unknown` | The background service returned an error. |
| `PeerStore` | `unknown` | The background service returned an error. |
| `TooManyPairings` | `unknown` | The background service returned an error. |

`PairCreateDialog.tsx:117` and `PairAcceptDialog` render `toFriendly(error)`, so
that table is what is on screen. Typing a wrong pairing code, typing a
malformed address, and pointing at a device that is switched off are
indistinguishable.

Two things make this a finding for *this* range rather than a standing gripe:

* `NodeError::TooManyPairings` landed today (`e471f92b`), and its commit message
  is "Refuse past 16 **with a reason naming the remedy**". The reason and the
  remedy are both discarded before the user sees anything. The doc comment on
  the variant — "reporting it as an internal error would hide the remedy" — is
  describing precisely what happens.
* `NoPeer` is worse than useless rather than merely unhelpful. It maps to
  `ErrorCode::NotFound`, which `from_code` (`backend/error.rs:103`) turns into
  the fixed string "That item is no longer there.", which the classifier reads
  as `not_found`, which renders **"That item is no longer in your clipboard
  history."** — for a *device*. Unpairing a peer that has gone reports a missing
  clipboard item.

The comment in `errors.ts:46-48` states the intended invariant — "the patterns
are matched against text this app does not author, so they track what
`BackendError` actually renders" — and that is exactly the invariant that broke
when the node's error vocabulary moved into `copypaste-p2p` (`6995f3e5`) and a
new variant was added on top of it (`e471f92b`) with no corresponding edit on
the TypeScript side. `errors.ts` was touched today (`ec22fe3a`) for the i18n
move and the pattern list was carried across unchanged.

The classification collapse predates the range for the seven older variants;
`TooManyPairings` and the `NoPeer` wording are in it. The correct fix is
structural rather than another regex: give the pairing failures their own
`ErrorCode`s, or carry a kind token instead of a sentence.

**In flight:** the same uncommitted `errors.ts` change adds three more pattern
rows (`legacy_database`, `key_locked`, `key_unusable`) and a retryability
record. It grows the pattern list rather than replacing the mechanism, so none
of the nine rows in the table above changes. Worth raising with that agent while
the file is open.

---

## 5. Reorder-pinned: five layers landed, the sixth refuses · **tracked (B-23/B-13)**, comments now false

`crates/copypaste-ui/src-tauri/src/backend/daemon.rs:236-243`,
`backend/embedded.rs:383-393`

The whole chain landed in this range — `Store::reorder_pinned`
(`c427bfeb`), `Method::ReorderPinned` (`6f91b76c`), the daemon dispatch
(`server/dispatch.rs:213`), the CLI verb (`cli.rs` `Reorder`), the Tauri command
(`commands/history.rs:120`), and the `useReorderPinned` hook. Both backend
implementations return `BackendError::Unsupported`, and no component calls the
hook, so nothing can reach it from the product surface.

This is registered as B-23/B-13, so it is not an undiscovered gap. What is new
and worth fixing today is that **both refusals now assert things that are
false**, which is the failure mode `CLAUDE.md` rule 8 singles out ("a comment
that is wrong is worse than one that is missing"):

* `daemon.rs:238` — "`copypaste_ipc::Method` has no reorder, so there is nothing
  to send." It has had one since `6f91b76c`, six hours before HEAD.
  `ipc/src/lib.rs:122` is `ReorderPinned { ids }`.
* `embedded.rs:383` — "Needs `Store::reorder_pinned`, which does not exist." It
  exists since `c427bfeb`, `storage/pinning.rs:72`.
* `ui/src/lib/ipc.ts:147` — "Not routed yet: `copypaste_ipc::Method` has no
  reorder verb". Same falsehood, third copy.

`docs/backlog.md:105` already notices one of the three ("`reorder_pinned`, whose
stated reason is false"). The other two are not flagged.

The daemon backend is a two-line change — `self.call(Method::ReorderPinned {
ids: ids.to_vec() }).await` — and the store method it needs on the embedded side
is right there. Whether to ship it is a product call; leaving three comments
that say the dependency is missing is not.

---

## 6. `open_validated` and `backup_to` have no consumers; the file they were written to delete is untouched

`crates/copypaste-core/src/storage/dbfile.rs:34,84`,
`crates/copypaste-daemon/src/dbfile.rs`

`c427bfeb`'s message: "Add `Store::backup_to`, `Store::reorder_pinned` and
`storage::open_validated`". The daemon's own `dbfile.rs:9-11` says what those
were for:

> **The clean fix is still a `copypaste-core` change** — a `Store::backup_to`
> and a `Store::validate` would delete this file.

Both landed. The file was not deleted, and nothing switched over:

* `open_validated` — no caller outside its own test module. `dbadmin.rs:188,225`
  and `meta/open.rs:48` still call the daemon's `dbfile::open`.
* `Store::backup_to` — no caller outside its own tests. `dbadmin::backup`
  (`dbadmin.rs:91`) still runs its own `VACUUM INTO` on a daemon-opened
  connection.

So the count of hand-written `PRAGMA key` paths is the same as before the
commit, and the tree now carries the duplicate *plus* the replacement. This is
rule 1's failure mode arriving from the other direction — not "someone wrote it
again", but "someone wrote the shared one and nobody imported it", which is
literally the case the rule's own preamble cites ("several of the duplications
above existed while the correct implementation was already present and merely
un-imported").

`docs/backlog.md:71` lists "Keyed-connection consolidation" as in flight, so the
intent is recorded; the consolidation is half-done and the half that landed is
dead code until the other half does. Worth a line in the backlog saying which
half.

A latent hazard rides on this, and it should be fixed before the first caller
appears rather than after. `open_validated` is documented as inspecting a
candidate file "as it stands" (`dbfile.rs:28-30`) and the module header calls it
"prove a *candidate* file — a backup being restored — before anything live is
touched". It then calls `apply_connection_pragmas`, which includes
`PRAGMA journal_mode = WAL` (`connection.rs:44`). That is a persistent header
write: validating a backup with this function modifies the backup and drops
`-wal`/`-shm` files beside it. The daemon's `dbfile::open` does not set
`journal_mode` and does not have the problem, which is presumably why nobody has
hit it. Not reproduced — there is no caller to exercise.

---

## 7. Smaller things, checked and clean, or checked and minor

Recorded because "I looked at it" is worth as much as a finding.

**Clean.**

* **Cloud row HMAC** (`20c73a01`). Sign and verify both build `RowMetadata` from
  the same `&self` (`rest/item.rs:171-183`), so the two cannot cover different
  fields. Length-prefixed injective encoding, HKDF-Expand off the Argon2 output
  with a distinct `info`, `verify_slice` for constant time, MAC over the
  ciphertext and never the plaintext. `validate()` refuses an unsigned row
  before upload. The realtime path is only a wake-up hint —
  `daemon/src/cloud/realtime.rs` forces a poll and never applies a row — so the
  unverified realtime payload never reaches the merge. I could not find a way in.
* **Cursor advance past a refusal** (`sync/pull.rs:255-266`). The ceiling is
  `created_at <= now`, and the reasoning in the module header holds up: a
  refused row at or behind `now` cannot skip an honest row because pages arrive
  in ascending keyset order. The `advanced == cursor` stall guard closes the
  spin case.
* **Sensitive purge cannot reach a clipboard item.** `clipboard_fts` is a plain
  fts5 table, not external-content (`schema.rs:68`), so `DELETE FROM
  clipboard_fts` cannot cascade. Both purge statements name only that table.
  `is_sensitive` is never rewritten, so the wipe sweep is never armed by a
  re-derived verdict. The module's own tests assert all of this and they are
  honest tests.
* **Transaction behaviour.** Every write path in `copypaste-core` and every
  read-then-write in `daemon/src/meta` goes through IMMEDIATE. I found no
  DEFERRED read-then-write. `reorder_pinned` reads the pinned set *inside* its
  IMMEDIATE transaction.
* **Pairing cap.** Enforced under `PeerStore`'s write lock
  (`peers/store.rs:150`), not by the pre-dial check, so the TOCTOU between
  `dial.rs:47`'s check and the write is harmless — and the pre-dial check is
  what stops a device from shipping its history to a peer it will then refuse.
  Refuses rather than evicts, and an update to a stored pairing is exempt.
* **PSK zeroization.** `psks()` allocates `with_capacity` from under the lock
  and never grows, returns `Zeroizing<Vec<PskCandidate>>`, and the handshake
  borrows rather than copies. The one residue is that `sort_by` memcpies
  `ZeroizeOnDrop` elements through temporaries; not worth changing.
* **Reassembly cap.** `Reassembly::push` checks `wanted > MAX_MESSAGE_BYTES`
  before reserving anything, and growth goes through a fresh `Zeroizing`
  allocation so the old buffer is wiped rather than freed intact.
* **`legacy::probe_unkeyed`** opens `SQLITE_OPEN_READ_ONLY`, so probing a v0.4
  file cannot journal it or replay a WAL into it — rule 3's "find their old data
  intact on disk" is upheld by the probe even though the probe never runs
  (finding 2).
* **Version agreement** for the release (`2808a8ab`). `tauri.conf.json`'s
  `"version": "../package.json"`, `crates/copypaste-ui/package.json` at
  `2.0.0-alpha.1`, workspace at `2.0.0-alpha.1`, and `scripts/release/check.sh`
  asserts all three. Consistent.

**Minor.**

* **`sensitive_ttl_secs` default moved to 0 today** (`6f91b76c`). Consistent
  everywhere — `ConfigData::default()`, the daemon's assertion at
  `settings.rs:231`, the CLI's rendered value, the wire fixture at
  `client.rs:527`. Nothing still assumes 30; `DEFAULT_SENSITIVE_TTL` is
  deliberately kept at 30 as "the value to use once it is switchable". No UI
  offers the switch (tracked, B-14/B-18).
* **`notify_on_copy`** is a config field with no reader. The daemon deliberately
  does not consult it (`ipc/src/lib.rs:297`) and the app is meant to; there is
  no notification plugin in `src-tauri` and no reference to `notify_on_copy` or
  to `EventData::captured` anywhere in the WebView. Tracked as B-18.
* **`Item::origin_device_name` / `too_large_to_sync`** reach `UiItem`
  (`src-tauri/src/model.rs:66`) and stop: `ui/src/lib/ipc.ts:22-31` declares six
  fields and neither is one of them. `model.rs:61` says of `origin_device_name`
  that "the whole point of it is to be shown". The CLI does show both. Tracked
  as B-15.
* **Restore leaves the cloud download watermark alone** (`sync_device_state` is
  not in `RESTORED_TABLES`). Deliberate, and the upload floor is re-armed via
  `note_version_written`, but rows already pulled before the restore and absent
  from the backup will not be re-fetched. Probably correct; noted because
  nothing says so.
* **`docs/backlog.md` is stale on B-3 and on all of §2.** B-3 says "the purge
  pass three texts promise does not exist" and "grep finds no rescan, reindex or
  purge anywhere" — `153213d4` and `5b912fc8` built it, in the same range. Every
  row of §2 "in flight — uncommitted in the working tree right now" has landed.

---

## What I did not look at

Stated so nobody reads this as coverage of twenty commits.

* **The design system and the frontend components in depth.** I ran both gates
  and read `errors.ts`, `ipc.ts`, `useHistory.ts` and the two pairing dialogs.
  `14c8f9cc` (accent hover fills), `5905c8c9` (component usage gate),
  `1e1e2984`'s ~1,500 lines of component change, and the 246 frontend tests were
  taken on trust.
* **The e2e suite.** Not run — it needs a built daemon and the container is at
  100% disk (see below). The five new e2e files (devices, settings, push,
  export-import, bulk-actions, daemon-config) were not read.
* **The release and packaging scripts.** `packaging/macos/selfsign.sh` (436 new
  lines), `scripts/release/check.sh` (450), `check-wiring.py` (215) and the
  Homebrew/cask generators were skimmed only for the version-agreement claim in
  `2808a8ab`.
* **The Supabase migrations, RLS policies and pgTAP tests** that came with
  `20c73a01`. I verified the client half of the row signature and did not audit
  the SQL.
* **The Android capture ladder** (`b9106c25`, `c28b2485`, `cd18e8c7`) — the
  Kotlin, the AIDL, the Shizuku path, the plugin wiring.
* **`ec22fe3a`'s catalogue move** beyond spot-checking `errors.ts` and
  `common.ts`.
* Commits `be2a339f`, `9fe8b2f7`, `7e0777e9`, `724e687b`, `1bc48302`,
  `82d54497`, `fa5e8e93`, `fff55313`, `bee10e12`, `0600e462`, `f69c53e8`,
  `cdd9d1ff` — formatting, hooks, CI wiring and doc audits, read only at the
  commit-message level.

## What I could not verify

* **Anything requiring a running daemon.** The container filesystem hit 100%
  (165 MB free) partway through, so `cargo build -p copypaste-daemon` could not
  link. Findings 2 and 3 are therefore call-site audits rather than live
  reproductions. Both are grep-conclusive — finding 2 rests on there being only
  four references to `is_v1_database`/`v1_database_in` in the tree, and finding
  3 on there being exactly one call to `purge_indexed_secrets`.
* **Everything Android.** No NDK, and `crypto/keystore/android.rs:1-3` and
  `src-tauri/src/android_context.rs` both say plainly that they have never been
  compiled. I read the JNI handover for the two things that are checkable by
  inspection and both look right: the symbol
  `Java_com_copypaste_app_KeystoreContext_initialize` matches a Kotlin `object`
  member (instance method, so the `_this: JObject` parameter is correct), and
  the `System.loadLibrary("copypaste_ui_lib")` name matches
  `[lib] name = "copypaste_ui_lib"` in `src-tauri/Cargo.toml`. Whether
  `applicationContext` is safe before `super.onCreate` (it should be — `attach()`
  runs first), whether Tauri's own `loadLibrary` conflicts, and whether the
  bundler accepts a pre-release `versionName` are all unverified.
* **The 9.7 ms purge measurement** in `sensitive/purge.rs`, and the 55×
  prefilter ratio in `sensitive/engine.rs`. Both have tests that assert a ratio
  rather than a wall clock, which is the right shape; I did not re-measure.
* **Whether finding 4's classification collapse is visible in the e2e suite.**
  `e2e/tests/error-strings.e2e.test.ts` changed in this range and I did not run
  it; it may already assert the generic sentence, in which case a test is
  pinning the defect.

## Method

Clean tree at `2808a8ab` extracted with `git archive` into scratch, built and
tested there with `CARGO_TARGET_DIR` outside the repo, so nothing here touched
the working tree — which was under heavy concurrent edit throughout (`meta/`
being lifted into `copypaste-core/src/sync/`, `embedded.rs` being split into a
directory, ~60 paths dirty at the time of writing). No file in the repository
was modified except this one.
