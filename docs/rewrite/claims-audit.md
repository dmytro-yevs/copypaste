# Claims audit — every assertion in this repository, against the code

**Question asked:** *which statements in this repository does the code
contradict?*

**Answer:** 24. Four are security properties that a reader would act on and that
do not hold; two of those four fail in the dangerous direction — a document says
a control exists that does not, and a document says a destructive feature does
not exist while it runs by default.

Two of the 24 were falsified *during this audit* by other agents' commits, which
is the rate the problem occurs at and is why §0 pins a snapshot.

## 0. Anchor, method, and what would make this document wrong

| | |
|---|---|
| Anchor | `HEAD` = `fa514730`, **plus the working tree** at the time of reading. Nine agents are editing concurrently; `crates/copypaste-core/src/ingest.rs`, `storage/dbfile.rs`, `storage/pinning.rs`, `daemon/src/meta/devices.rs`, `ipc/src/content_type.rs`, `p2p/src/netif.rs`, `p2p/src/node/`, `ui/src-tauri/src/capture/` are untracked and are read as part of the tree. |
| Executed | `cargo test -p copypaste-core --lib` (123 pass), `./scripts/check-file-size.sh 500`, `cd design && npm run check`. Nothing else was run. |
| Not checked | macOS and Android runtime behaviour; the Supabase deployment; `docs/rewrite/port-manifest/**` internal consistency; `crates/copypaste-ui/src/**` TypeScript beyond four greps; `packaging/`, `Casks/`, `supabase/` SQL. |
| Landed mid-audit | `docs/backlog.md` appeared while this was being written and re-checks the same audits against the same tree. It independently reaches finding 3 (its **B-3**) and its §1 corroborates findings 5, 6 and 8. It is **not** audited here, with one exception: its §6 files the macOS Keychain store under "waiting on hardware", which finding 1 contradicts — no Mac is needed to observe that no build enables the feature. |
| Verdicts | **false** — wrong now, and there is no commit at which it was right in this tree. **stale** — was true, a later change falsified it. **true**. **unverifiable here**. |

Every "false" below names the file and line on both sides. Where I could not
distinguish false from stale I say stale, because the cheaper repair is the same.

---

## 1. Findings, ranked by consequence

### Tier 1 — a security property that does not hold as written

| # | Claim | Where the claim is | What the code does | Where | Verdict |
|---|---|---|---|---|---|
| **1** | The macOS device secret lives in the Keychain, "behind the `macos-keychain` cargo feature", and the `0600` file store is a *Linux* development fallback, "**not a shipping posture**"; the Keychain store is listed as work "waiting on hardware" | `SECURITY.md:45-49` · `crates/copypaste-core/Cargo.toml:39` ("Enabled by default on macOS builds") · `docs/backlog.md:182` | `macos-keychain` is **not** a default feature — `[features]` has no `default` list — and **no build in the repository enables it**. `copypaste-daemon`'s dependency on `copypaste-core` names no features. `scripts/release/build-macos-app.sh:110` and `build-cli-tarball.sh:44` are plain `cargo build --release --locked -p copypaste-daemon -p copypaste-cli`. The only place the feature is reached is CI's `--all-features` lint. A shipped macOS binary therefore takes the `#[cfg(not(all(target_os = "macos", feature = "macos-keychain")))]` branch and writes the device secret to a file. | `crates/copypaste-core/Cargo.toml:38-41` · `crates/copypaste-core/src/crypto/keystore.rs:41,92` · `crates/copypaste-daemon/Cargo.toml:10` · `scripts/release/build-macos-app.sh:110` | **false** |
| **2** | "It is **not** deleted automatically… no automatic deletion is implemented"; auto-wipe is listed under **Not implemented**; the sweep's own doc says `0` "is the default until a user asks for it" | `SECURITY.md:69-71` · `SECURITY.md:171` · `README.md:54-55` · `crates/copypaste-daemon/src/capture.rs:77-78` | Auto-wipe is built, wired to the poll loop, and **on by default at 30 s**. `ConfigData::default()` sets `sensitive_ttl_secs: 30`; a fresh install has no stored settings and `Settings::load` falls through to that default. The sweep **hard-deletes** (not tombstones) every row that is `is_sensitive = 1` and still scans `Severity::HighConfidence`. | `crates/copypaste-ipc/src/config.rs:106` · `crates/copypaste-daemon/src/settings.rs:48-62` · `crates/copypaste-daemon/src/capture.rs:79-110` · `crates/copypaste-core/src/sensitive/wipe.rs:65-101` | **false** |
| **3** | Sensitive items are kept out of the search index "at write time, at read time, **and by a purge migration for databases predating the rule**" / "and by a purge pass" | `CLAUDE.md:111` · `crates/copypaste-ipc/src/payload.rs:199` · `crates/copypaste-daemon/src/server/items.rs:349` | No purge, reindex or rescan exists. `schema.rs` is one migration (`Migrations::new(vec![M::up(SCHEMA_V1)])`) with no purge step. The three real layers are the write guard, the in-transaction re-read, and the read-time JOIN — none of them a purge. `is_sensitive` is computed once at capture and nothing revisits it, so a ruleset change never reflags an existing row. Already recorded as **F-12** and still open. | `crates/copypaste-core/src/storage/schema.rs:68-69` · `crates/copypaste-core/src/storage/search.rs:1-9` · `docs/rewrite/security-review.md:389-414` | **false** |
| **4** | Pairing codes: "nothing expires or burns it"; expiring or single-use pairing codes are **Not implemented** | `SECURITY.md:94-95` · `SECURITY.md:171-173` · `README.md:57` | Both exist. `PAIRING_CODE_TTL = 300 s`; an unredeemed pairing carries a deadline in `State::pending` and past it "its PSK is offered to no handshake"; the first successful session drops the deadline, giving exactly one redemption. Also present and undocumented: `PeerStore::revoke`, `revoke_all`, `revoked()` with a persisted `revoked` audit map. | `crates/copypaste-p2p/src/peers/mod.rs:46` · `crates/copypaste-p2p/src/peers/store.rs:3-21,133-142,227,260,289` · `crates/copypaste-p2p/src/peers/file.rs:42-48` | **stale** |

Findings 1 and 2 fail in the direction that costs a user something. Finding 4
fails in the safe direction but is still worth fixing: it invites someone to
build a control that is already there.

### Tier 2 — false statements that would send someone to do work already done, or to skip work still needed

| # | Claim | Where | What the code does | Where | Verdict |
|---|---|---|---|---|---|
| **5** | "the socket bind is TOCTOU-racy, and the IPC accept loop has no connection cap and no read or write timeouts" — presented as two live safety gaps, in both top-level documents | `SECURITY.md:176-178` · `README.md:60-62` | All three landed. `BindLock` takes an exclusive `flock(2)` on `<socket>.lock` around the whole probe→remove→bind sequence. `MAX_CONCURRENT_CONNECTIONS = 64`, acquired with `try_acquire_owned`. `READ_TIMEOUT = 30 s`, `WRITE_TIMEOUT = 10 s`, plus a `MAX_WATCHERS = 8` sub-cap. | `crates/copypaste-daemon/src/server/listener.rs:87,98-126,52,59,64,68,161` | **stale** |
| **6** | The README "Missing" list: "no sensitive-item auto-wipe, no export/import, no backup/restore, … dedup only inside a 60-second window (so re-copying an old item makes a second row), no daemon config or server-owned settings, pairing codes that never expire, no streaming updates, no discovery listing" — and "Pairing UI and the popup/hotkey shell have since landed; **the rest have not**" | `README.md:52-59` | Eight of the named gaps are closed. Export/import/backup/restore: CLI `Export`/`Import`/`Backup`/`Restore` and IPC `Method::{Export,Import,Backup,Restore}`. Config: `copypaste-ipc/src/config.rs` (11 fields, bounds, liveness), `daemon/src/settings.rs`, CLI `config show`/`config set`, IPC `GetConfig`/`SetConfig`. Streaming: CLI `Watch`, `Method::Watch`, `server/watch.rs`. Discovery listing: CLI `Discover`, `Method::{Discovered,Rescan}`. Auto-wipe and code expiry: findings 2 and 4. | `crates/copypaste-cli/src/cli.rs:125-197` · `crates/copypaste-ipc/src/lib.rs:152-230` · `crates/copypaste-daemon/src/server/watch.rs` | **stale** |
| **7** | "dedup only inside a 60-second window (so re-copying an old item makes a second row)" | `README.md:56-57` | Re-copying an old item does **not** make a second row. The bounded probe in `ingest_into` misses, but `Store::insert` delegates to `insert_or_bump`, whose probe is `newest_live_with_hash(&tx, hash, i64::MIN)` — unbounded across all live history — and bumps the surviving row. The ingest test asserts `count() == 1` after a repeat at `T0 + 120_000`. The only real consequence of the window is a mislabel: the outcome is reported as `Ingested::Stored` rather than `Ingested::Duplicate`. | `crates/copypaste-core/src/ingest.rs:120-130,144` · `crates/copypaste-core/src/storage/items.rs:19-23,62-66,122-124` · `crates/copypaste-core/src/storage/retention.rs:14-25` · test `ingest.rs:269-282` | **false** |
| **8** | "Two capabilities exist in `copypaste-core` with no caller: `retention::evict_older_than` (age-based retention) and `page::list_from`" — and, separately, age-based retention is **Not implemented** | `README.md:67-69` · `SECURITY.md:171` | `evict_older_than` has a caller: the ingest path runs it whenever `retention_days > 0`, and `retention_days` is a live setting with its own CLI flag and its own test. Only the `list_from` half is true — the server and the app both page by offset. | `crates/copypaste-core/src/ingest.rs:170-175` · `crates/copypaste-ipc/src/config.rs:33` · test `ingest.rs:316-322` · (`list_from` unreferenced outside `page.rs`; `daemon/src/server/items.rs:61-63` and `ui/src/lib/ipc.ts:114` use offset) | **false** (first half) |
| **9** | `sound_on_copy` — "Unlike the notification, the daemon *can* do this itself **and does** — see `daemon/src/notify.rs`"; and `notify_on_copy` — "parity finding 18 is built" | `crates/copypaste-ipc/src/config.rs:51-52,125` · `crates/copypaste-ipc/src/lib.rs:301` | **`crates/copypaste-daemon/src/notify.rs` does not exist** (`ls crates/copypaste-daemon/src/`). Nothing plays a sound anywhere in the workspace. `EventData.captured` is never set `true`: the only constructor is `AppState::publish`, which builds `EventData` without it. No frontend file references `notifyOnCopy`, `soundOnCopy` or `captured`. Both settings are stored, validated, round-tripped — and read by nothing. | `crates/copypaste-daemon/src/main.rs:276-282` · `crates/copypaste-daemon/src/` (no `notify.rs`) · `crates/copypaste-ui/src/**` (no match) | **false** |
| **10** | "Needs `Store::reorder_pinned`, **which does not exist**." — the recorded reason both backends refuse pin reordering | `crates/copypaste-ui/src-tauri/src/backend/embedded.rs:352` | `Store::reorder_pinned` exists, with six tests, and the daemon already dispatches `Method::ReorderPinned` to it. The refusal is now unjustified by its own stated reason. | `crates/copypaste-core/src/storage/pinning.rs:72` · `crates/copypaste-daemon/src/server/dispatch.rs:213` | **stale** |
| **11** | README's CLI inventory: `list search add copy **get** delete clear pin unpin status pair peers unpair sync cloud` | `README.md:34` | There is no `get` subcommand (`Get` is an IPC method, not a CLI verb), and seven that exist are missing from the list: `discover export import backup restore config watch`. | `crates/copypaste-cli/src/cli.rs:33-197` | **false** |
| **12** | Row mapping is done by "`serde_rusqlite`, by name" | `docs/rewrite/target-architecture.md:16` | `serde_rusqlite` is **not a dependency of this workspace** — it was removed from `Cargo.toml` mid-audit (finding 17), and `port-manifest/03-storage.md` was amended in the same stroke while this line was not. Row mapping is `rusqlite`'s `row.get("name")` behind the `item_columns!` / `item_columns_ci!` macros. The claim's *substance* — by name, not by position — is true; the named crate is not what does it and is no longer present. | `Cargo.toml` (no `serde_rusqlite`) · `crates/copypaste-core/src/storage/model.rs:8-27,140-150` | **false** |
| **12a** | "`npm run check` … **636 pairs**", and a worst-case table in which every category clears its floor — asserted as measurement, with "Do not replace a measurement here with a claim" beside it | `design/README.md:9,59-91` | **Ran it, twice.** On the first pass: `contrast: 636 pairs clear AA`. On the second, ninety minutes later and after `design/check-contrast.mjs` was edited by another agent: `contrast: **22 of 840** pairs below floor`, including `light/green bg-primary/90 — --on-accent on it, over --bg — 4.20:1, needs 4.5:1` and `dark/blue --color-ring (alias) on --elevated — 2.83:1, needs 3:1`. Two numbers in the document are now wrong (the pair count and the worst-case table) and the gate does not pass. This may be an in-flight edit rather than a regression — but the document is the thing that says 636, and the checker is the thing that says 840. | `design/check-contrast.mjs` (modified, uncommitted) · `design/README.md:63,73-85` | **stale** |
| **13** | "`docs/README.md` indexes **every** ADR, audit and study" | `README.md:122` · `docs/README.md:1-4` | `docs/README.md` lists ADR-0001/0002/0003 and omits **ADR-0004** (`0004-the-app-owns-the-daemon.md`, accepted 2026-07-30) and **`docs/rewrite/ui-parity-audit.md`** (added in `3d618932`). A reader following README's instruction to "Start there rather than here" never learns the app owns the daemon's lifetime. | `docs/README.md:8-25` · `docs/adr/0004-the-app-owns-the-daemon.md` · `docs/rewrite/ui-parity-audit.md` | **stale** |

### Tier 3 — documents that are cited as current and are not

| # | Claim | Where | Reality | Verdict |
|---|---|---|---|---|
| **14** | `SECURITY.md` sends the reader to the security review and says "**where the two disagree, believe it**" | `SECURITY.md:19-22` | The review is dated the same day and already partly overtaken: **F-3** (peer `content_hash` unverified) is closed by `sync/session.rs:232-245`, which recomputes the hash before applying; **F-6** (token never expires) is closed by `PAIRING_CODE_TTL`; **F-7** (the mDNS claim) is closed by the corrected `SECURITY.md:105-108`. The instruction now points at three findings that are no longer true. Still open and worth keeping: F-1, F-2, F-9 (bind→chmod, distinct from finding 5), F-10, F-12. | **stale** |
| **15** | `parity-audit.md` — cited as the live "Missing" list by both `README.md:52` and `SECURITY.md:175-178` | `docs/rewrite/parity-audit.md` | Anchored at `c53be35b` and honest about it (§0), but at least nine of its nineteen findings are now closed (3, 4, 5, 6, 9, 12, 13, 15, 16), and `ui-parity-audit.md:29-34` already records that six of its UI findings are dead. The document is not the problem; the two places that cite it as current are. | **stale** |
| **16** | `docs/README.md` describes the review as "Fourteen findings, two High"; `SECURITY.md` describes the same document as "fourteen findings, **none critical**" | `docs/README.md:24` vs `SECURITY.md:20-21` | Both count fourteen. The review's severity scale is High/Medium/Low with no Critical tier, so "none critical" is vacuously true and reads as reassurance the review does not offer. | **misleading, not false** |

### Tier 4 — declared-and-unused, stale narration, and counts

| # | Claim | Where | Reality | Verdict |
|---|---|---|---|---|
| **17** | "Library-first (CLAUDE.md rule 1). **Every entry here is a wheel we are NOT carving.**" | `Cargo.toml:20-21` | `toml = "0.8"` (`Cargo.toml:39`) is referenced by no member manifest and appears in no `.rs` file. This is the shape that put `governor` in the v1 audit table three rows above. **Corrected mid-audit:** `serde_rusqlite = "0.36"` was the second such entry and another agent removed it, and amended `port-manifest/03-storage.md:1218,1243`, while this was being written — `target-architecture.md:16` (finding 12) was *not* amended with it, and is now the last text in the tree claiming the crate is in use. | **false** |
| **18** | `security-framework.workspace = true`, unconditional in the daemon's macOS target table | `crates/copypaste-daemon/Cargo.toml:52` | No `security_framework` usage anywhere under `crates/copypaste-daemon/src/`. The Keychain lives in `copypaste-core`. Unused on the one platform where it compiles. | **false** |
| **19** | `PeerStore::revoke` / `revoke_all` / `revoked()` | `crates/copypaste-p2p/src/peers/store.rs:227,260,289` | Implemented, tested, persisted to the peer file — and called from nowhere outside `copypaste-p2p`. No `Method` variant, no CLI verb, no daemon handler, no Tauri command. Device revocation is built and unreachable, which is CLAUDE.md rule 6's failure shape. `README.md:56` and `SECURITY.md:172` describe it as absent, which is true for a user and false for the tree. | **built, no caller** |
| **20** | "this crate's `Cargo.toml` **still needs** `security-framework` as an optional dependency and the feature that enables it. Until then macOS falls through to the file backend" | `crates/copypaste-core/src/crypto/keystore.rs:37-40` | Both landed: `crates/copypaste-core/Cargo.toml:35-36` declares the optional dependency and `:41` the feature. The comment describes work already done — and, ironically, its conclusion is still true for a different reason (finding 1). | **stale** |
| **21** | "Configuration is not wired yet; when it is, §3.10 applies" | `crates/copypaste-daemon/src/clipboard/mod.rs:39-41` | Configuration is wired: `max_item_bytes` is a live setting enforced at `ingest.rs:108`. What remains true is the narrow point the comment was making — `MAX_TEXT_BYTES` is still a compiled-in `const` and is *not* driven by that setting, so the two gates it warns about are still two numbers. The sentence overstates it. | **stale** |
| **22** | "works on macOS and Android, which is the whole of **CLAUDE.md rule 5**" | `crates/copypaste-daemon/Cargo.toml:42` | Rule 5 is the 500-line file-size budget. Cross-platform scope is **rule 7**. The `rustix` comment eight lines above cites "rules 1 and 7" correctly. | **false** |
| **23** | Counts | `docs/supabase-deployment.md:7` "7,500 lines, 156 tests" → actual `copypaste-cloud` is 8,619 lines, 178 test attributes. `docs/rewrite/parity-audit.md:20-23` "26,147 Rust lines… 523 tests" → actual 38,306 lines, 797 test attributes. `README.md:109` "~9,000 lines" vs `CLAUDE.md:54` "~9,100" vs `parity-audit.md:30` "9,582" for `port-manifest/`; actual `wc -l` is **9,121**, so CLAUDE.md is right and the other two are not. | Noise on its own; listed because each is a number someone will quote. | **stale** |

### One test that pins a claim and cannot fail as named

`crates/copypaste-core/src/ingest.rs:269` —
`a_repeat_inside_the_window_deduplicates_and_one_outside_it_does_not`. The name
asserts that a repeat outside the window does *not* deduplicate; the body
asserts `f.store.count() == 1`, i.e. that it **does**. The test is correct and
the name is the artefact — but it is the sentence a reader greps for, and it is
where `README.md:56-57`'s wrong claim most likely came from.

`crates/copypaste-core/src/storage/retention.rs:337-357`
(`a_recopy_far_outside_any_window_bumps_the_original_row`) is the test that
proves finding 7, and it exercises `insert_or_bump` directly rather than through
`ingest_into`. It happens to be sound — `Store::insert` delegates to
`insert_or_bump` — but nothing in the test says so, so the equivalence is
carried by a one-line delegation at `items.rs:122-124` and by nothing else.

---

## 2. Claims checked and found sound

Recorded because a verified claim is a result. Each was read on both sides.

| Claim | Where asserted | What upholds it |
|---|---|---|
| Content sealed with XChaCha20-Poly1305, **item id bound as AAD**, fail-closed on wrong key/AAD/tamper | `SECURITY.md:30-40` | `crypto/aead.rs:24-26,337-340`; the id must be chosen before the seal, and `ingest.rs:134-142` records why |
| HKDF info strings are exactly `copypaste/v2/sqlcipher-db-key` and `copypaste/v2/item-content-key` | `SECURITY.md:36-38` | `crypto/keys.rs:25,28`, pinned by `keys.rs:220-222` |
| SQLCipher keyed with a raw 32-byte key, no passphrase KDF pass | `SECURITY.md:33-35` | `daemon/src/dbfile.rs:44-50`, `core/src/storage/dbfile.rs:53-63` — and the `x'…'`-must-be-literal trap is written down |
| The keystore fails closed: only an unambiguous "no entry" mints a fresh secret | `SECURITY.md:51-53` | `crypto/keystore.rs:1-8,48-52` (`errSecItemNotFound` = `-25300`, the only authorising status) |
| mDNS advertises a `pairing_id` that **is** a domain-separated BLAKE2s of the token, truncated to 128 bits | `SECURITY.md:105-108` | `p2p/transport/token.rs:34,38,129-132` — the corrected claim now matches the code exactly, including the domain separator and the truncation length |
| Sensitive items never leave the device, gated twice on the cloud path | `SECURITY.md:66-68,142-145` | Outbound query filters `is_sensitive = 0` (`daemon/src/meta/read.rs:30,106,121,147`); the driver re-checks (`cloud/sync/push.rs:54-61`); peer advertisement filters at `meta/read.rs:30` and `sync/session.rs` refuses to serve anything unadvertised |
| A peer version stamped >24 h in the future is skipped, never fatal, never a delete | `SECURITY.md:110-112` | `p2p/sync/plan.rs:17,36-47`; enforced end-to-end because `receive_items` drops any item whose `summary()` differs from what was promised (`sync/session.rs:220-232`). The cloud path mirrors it at `cloud/sync/pull.rs:56,111` |
| The merge order is four keys, `deleted` third, and the two shapes cannot drift | `p2p/sync/merge.rs:14-48` | `merge.rs:80-85,97-99`; `merge_is_exactly_the_four_key_lexicographic_order` pins it to a tuple comparison over the whole decision space. This is the claim that was wrong twice before; it is right now, in both the header and the contract paragraph |
| One comparator, reached by both transports | `cloud/sync/mod.rs:3-10` · `daemon/src/merge.rs:4-6` | `daemon/src/merge.rs` is the only call site of `merge_decision` in the daemon; `cloud/source.rs:122` and the p2p path both route through it |
| No user-facing error contains a path; the redaction pass is shared | `SECURITY.md:84-86` | `copypaste_ipc::redact::scrub_paths` is re-exported by the CLI (`cli/src/error.rs:117`) and used by the Tauri bridge (`ui/src-tauri/src/backend/error.rs:23,84,92`) |
| Sign-in has no password or passphrase flag | `SECURITY.md:136-138` | `cli/src/cli.rs:240-244` — `SignIn` takes `--email` and nothing else |
| Cloud sync is wired to the daemon and the CLI and has never spoken to a real project | `SECURITY.md:116-121` | `daemon/src/cloud/{mod,handlers,poll,source}.rs` exist and dispatch; the review's caveat 2 asked for this correction and it was made |
| Distinct database filename, so a v1 file is never opened | `README.md:10-13` · `SECURITY.md:200-201` | `copypaste-ipc/src/lib.rs:408` → `copypaste-v2.db` |
| Every source file is inside the 500-line budget | `CLAUDE.md:119` · `.github/workflows/ci.yml:103` (`EXEMPT=""`) | **Ran it.** `./scripts/check-file-size.sh 500` → "All files within the 500-line budget", with an empty exemption list |
| `backon` is the one retry implementation | `Cargo.toml:38-41` | Seven call sites, all `backon::` (`cloud/rest/client.rs`, `cloud/auth/{client,http}.rs`, `cloud/realtime/socket.rs`, `ui/src-tauri/src/service/mod.rs`); no second backoff in the tree |
| Rate limiting, telemetry, `aead::stream` chunking: not built | `docs/rewrite/target-architecture.md:20,23,28` | No rate limiter outside error *handling* of a backend's 429; no telemetry; no `aead::stream` in `crypto/aead.rs` |
| The ci.yml comment about the demo scripts' broken `command -v cargo +1.96` pin | `.github/workflows/ci.yml:190-196` | Still accurate in substance and now over-cautious: all three scripts have been fixed to `cargo +1.96 --version >/dev/null` (`scripts/demo.sh:22`, `demo-p2p.sh:29`, `demo-cloud.sh:43`). `RUSTUP_TOOLCHAIN` is now belt-and-braces, not load-bearing |
| `excluded_app_bundle_ids` is "**stored and returned, not yet enforced**" | `crates/copypaste-ipc/src/config.rs:89-95` | Correct, and unusually well stated — it names *why* (no frontmost-app attribution), names what *is* enforced instead (`org.nspasteboard.*`), and tells a client to say so. `README.md:66` calling the exclusion list "absent" is consistent with it. This is the model the other config docs are not following |

---

## 3. The pattern, and the two repairs that close most of it

**Nine of the twenty-four findings are one shape:** a document lists a
capability as missing, the capability lands, and nobody edits the list.
`README.md`'s "Missing" section and `SECURITY.md`'s "Not implemented" paragraph
account for findings 2, 4, 5, 6, 7, 8 and half of 19 between them. Both are
prose inventories of absence, which is the single most perishable thing a
repository can write down — every commit that adds a feature falsifies one, and
nothing fails when it does.

**Two repairs, in order of value:**

1. **Findings 1, 2 and 3 first, and separately from the rest.** They are the
   ones where believing the document leads to a worse outcome than having no
   document: a device secret in a file on the platform we ship, a hard delete
   running by default that the security page says is not implemented, and a
   third enforcement layer that three texts promise and no code provides.
2. **Stop writing inventories of absence in prose.** The "Missing" and "Not
   implemented" lists should name their evidence the way the parity audit's
   "Where I looked" column does, or be replaced by a generated list — the CLI
   verb table, the `Method` enum and the workspace manifest are all
   machine-readable, and all three are wrong in `README.md` today.

A third, smaller: `SECURITY.md:22`'s "where the two disagree, believe it" should
be deleted. It hands authority to a document that is now partly stale, and
finding 14 is the result.
