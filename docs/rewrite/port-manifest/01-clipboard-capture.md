# Port Manifest 01 — Clipboard Capture

**Status:** normative. This file is the acceptance criteria for the clipboard-capture
subsystem in the library-first rewrite. A rule here exists because a real bug was
fixed; deleting a rule is a regression unless the deletion is argued explicitly.

**Harvested from (v1 source, for traceability only — do NOT port structure):**

| Path | Lines | Role |
|---|---|---|
| `crates/copypaste-daemon/src/clipboard/mod.rs` | 17 | facade re-exports |
| `crates/copypaste-daemon/src/clipboard/monitor.rs` | 542 | NSPasteboard poll |
| `crates/copypaste-daemon/src/clipboard/content.rs` | 169 | content model |
| `crates/copypaste-daemon/src/clipboard/macos_util.rs` | 193 | UTI log-once, percent-decode, MIME |
| `crates/copypaste-daemon/src/clipboard/meta.rs` | 114 | content hash, thumb id, meta JSON |
| `crates/copypaste-daemon/src/daemon/capture/tick.rs` | 491 | per-tick dispatch |
| `crates/copypaste-daemon/src/daemon/capture/text.rs` | 582 | text ingest |
| `crates/copypaste-daemon/src/daemon/capture/image.rs` | 569 | image ingest |
| `crates/copypaste-daemon/src/daemon/capture/file.rs` | 154 | file ingest |
| `crates/copypaste-daemon/src/daemon/capture/frontmost.rs` | 259 | lsappinfo TTL cache |
| `crates/copypaste-daemon/src/daemon/capture/cleanup.rs` | 218 | TTL + byte-cap prune |
| `crates/copypaste-daemon/src/daemon/monitor_loop.rs` | 270 | poll ticker / hot-reload |
| `crates/copypaste-daemon/src/ipc/handlers_items_paste.rs` | 437 | the *write* side that arms self-write suppression |
| `crates/copypaste-daemon/src/platform/macos.rs` | 297 | alternate backend wrapper |
| `crates/copypaste-daemon/tests/clipboard.rs` | 333 | real-NSPasteboard integration tests (all `#[ignore]`) |

---

## 1. Purpose & scope

### In scope

The **capture pipeline**: everything between "the OS clipboard changed" and "a
row exists in the local encrypted DB and has been broadcast to sync subscribers".

Concretely, five responsibilities:

1. **Change detection** — decide *whether* the system clipboard changed, without
   reading (or even materialising) its contents when it did not.
2. **Privacy gating** — decide whether the change is one we are *allowed* to
   observe (password-manager markers, private mode, app exclusion list).
3. **Extraction** — decide which single representation of a multi-representation
   pasteboard item to capture (text / image / file), and pull it out.
4. **Ingest** — encrypt, deduplicate, stamp identity/ordering metadata, persist,
   prune, broadcast.
5. **Attribution** — record which application was frontmost at capture time, and
   escalate the item to "sensitive" when that app is a credential store.

### Out of scope (owned by other manifests)

- Paste-back / `write_to_pasteboard` semantics, *except* the self-write sentinel
  protocol (§3.3) which is a two-sided contract that capture cannot honour alone.
- Encryption primitives (key derivation, AAD layout, chunking) — the manifest
  states only *which* key/AAD capture must use.
- Sync/merge/LWW — the manifest states only which fields capture must stamp.
- The sensitive-content pattern detector — capture only calls it.

### Platform posture

macOS is the only platform with a real implementation. Every other target MUST
compile and MUST behave as "clipboard capture is unavailable": no panics, no
background polling cost, no partially-wired state. See §3.11.

---

## 2. Invariants (MUST hold)

Numbered for citation in review. "MUST" is normative.

### 2.1 Change detection

- **I-1.** A poll on an unchanged pasteboard MUST return "no content" and MUST
  NOT read any representation. The change-count comparison MUST be the *first*
  thing the poll does; on an unchanged clipboard the poll MUST perform zero
  Cocoa reads and zero heap allocations.
  *(v1: `monitor.rs:163-172` — the `count == self.last_change_count → return None`
  guard sits above every `stringForType`/`dataForType`.)*
- **I-2.** The monitor MUST hold a change-count cursor initialised to a sentinel
  that is distinguishable from every valid change count (v1 uses `-1`;
  `NSPasteboard.changeCount` is a non-negative, monotonically increasing
  `NSInteger`). The very first poll after startup MUST NOT be treated as a burst.
  *(`monitor.rs:100`, `monitor.rs:433-437`.)*
- **I-3.** Every code path that decides "do not capture this change" MUST still
  advance the cursor. Skipping without advancing causes the same content to be
  re-offered forever; advancing without capturing is the intended
  "acknowledge and drop". This applies to: privacy markers (§2.2), self-writes
  (§3.3), private mode, and the app-exclusion gate.
  *(`monitor.rs:193-210`, `monitor.rs:419-429`, `tick.rs:35-40`, `tick.rs:103-128`.)*
- **I-4.** Cursor advancement MUST happen *after* the burst delta is computed
  from the old cursor value, never before.
  *(`monitor.rs:431-438`.)*

### 2.2 Privacy

- **I-5.** Before any representation is read, the poll MUST probe for the three
  `org.nspasteboard.*` opt-out markers (`TransientType`, `ConcealedType`,
  `AutoGeneratedType`). If **any** is present the change MUST be dropped: no
  content read, no buffering, no logging of content, cursor advanced.
  *(`monitor.rs:174-203`.)* The probe MUST precede content reads so that a
  password never enters process memory at all — "read then discard" is NOT
  compliant.
- **I-6.** Private mode MUST suppress recording while still advancing the cursor,
  so that disabling private mode does not replay everything copied during it.
  *(`tick.rs:34-40`.)*
- **I-7.** When the exclusion list (`excluded_app_bundle_ids`) is non-empty and
  the frontmost application cannot be determined, capture MUST be skipped for
  that tick (**fail closed**). When the exclusion list is empty, an unknown
  frontmost app MUST NOT block capture (fail open is safe there because the
  content-pattern gate still applies).
  *(`tick.rs:99-129`, P1-2.)*
- **I-8.** Frontmost-app resolution MUST run on every tick on macOS **regardless
  of whether the exclusion list is empty**, because its second consumer is
  sensitive-app classification.
  *(`tick.rs:79-84`, CopyPaste-44rq.43 — see §3.9.)*
- **I-9.** Logs MUST NOT contain clipboard content, file paths, filenames, or
  MIME types. Permitted: byte counts, name *lengths*, item ids, bundle ids,
  change counts. On error paths where a name is genuinely needed for
  diagnosis, only the basename may be logged.
  *(`tick.rs:221-230`, `tick.rs:286-311`, `tick.rs:328-351` — CopyPaste-am9w.)*
- **I-10.** The plaintext content hash used for dedup MUST NOT be logged
  alongside anything else that could correlate it to content.
  *(`text.rs:86-91`.)*

### 2.3 Representation selection

- **I-11.** Exactly one representation is captured per change, chosen by a
  **strict priority**: `text > image > file`. Lower-priority representations
  present on the same item MUST be dropped for that change, not queued.
  *(`monitor.rs:139-152`, `monitor.rs:209-359`.)*
- **I-12.** Image *presence* MUST be probed without materialising the bytes.
  Only when text is absent **and** an image type is available may the bytes be
  copied out. A multi-MB image accompanying text MUST never be copied into the
  process heap.
  *(`monitor.rs:215-250`.)*
- **I-13.** Image type precedence is `public.png` first, `public.tiff` as
  fallback. TIFF MUST be consulted only when PNG data is absent.
  *(`monitor.rs:226-247`.)*
- **I-14.** File URLs MUST be resolved from `public.file-url` first and
  `NSFilenamesPboardType` second, and only when both text and image are absent.
  *(`monitor.rs:252-342`.)*
- **I-15.** A resolved file path MUST be absolute. Non-absolute paths (remote
  URLs, relative garbage) MUST be dropped with a debug log.
  *(`monitor.rs:279-287`, `monitor.rs:316-327`.)*
- **I-16.** File bytes MUST NOT be read on the async executor thread. The poll
  returns a *reference* (path + derived filename + derived MIME); the read is
  performed on a blocking thread by the tick handler.
  *(`monitor.rs:351-355`, `content.rs:30-41`, `tick.rs:261-281`.)*

### 2.4 Resource safety

- **I-17.** Every poll MUST drain an autorelease pool around the entire Cocoa
  interaction. Without it, autoreleased `NSString`/`NSData` (including multi-MB
  image data) accumulate on the async worker thread and are never freed,
  inflating reserved virtual memory without bound.
  *(`monitor.rs:159-163`.)* The same applies to the paste-back path
  (`handlers_items_paste.rs:176-180`).
- **I-18.** The size of pasteboard data MUST be checked *before* copying it into
  the process heap. `NSData.length` is a field read; `.bytes().to_vec()` on a
  multi-GiB item is a multi-GiB allocation.
  *(`monitor.rs:227-247`, CopyPaste-1f5c.)*
- **I-19.** File size MUST be checked and the file read within a **single**
  blocking operation, so no other process can substitute the file between
  `stat` and `read`.
  *(`tick.rs:251-281`, CopyPaste-b5iz — TOCTOU.)*
- **I-20.** All synchronous work — SQLite access, image encode/decode,
  encryption, filesystem reads, subprocess spawn — MUST run off the async
  executor. Holding the DB mutex across an `.await` point on the executor is
  forbidden.
  *(`text.rs:98-107`, `image.rs:26-40`, `file.rs:45`, `cleanup.rs:33`,
  `frontmost.rs:104`.)*
- **I-21.** Spawned helper processes MUST be reaped. A dropped child handle
  leaves a zombie in the process table for the daemon's lifetime.
  *(`tick.rs:170-180`.)*

### 2.5 Ingest & identity

- **I-22.** A capture MUST be encrypted with the *same* `(key, AAD, key_version)`
  triple the read path will use for that `key_version`. v1's contract: rows are
  stamped `key_version = 2`, which means the v2-derived key and the v2 AAD
  format. Encrypting with the v1 key while stamping `key_version = 2` produced
  an auth-tag mismatch on every paste-back.
  *(`text.rs:15-46` — "the v0.4 ingest fix".)*
- **I-23.** Identical text content MUST NOT create a second row. The existing
  row is bumped to the top of history instead. Dedup is keyed on
  `SHA-256(plaintext)` hex, searched across **all** history (no time window) —
  a pinned item is never expired and must therefore always be found and bumped.
  *(`text.rs:110-183`.)*
- **I-24.** Identical images and identical files MUST also converge to one row,
  and a re-copy MUST bump the existing row's recency. Content-addressed identity
  is the mechanism: `file_id = SHA-256(raw)[..16]` and
  `item_id = UUID::from_bytes(file_id)`.
  *(`image.rs:60-70`, `image.rs:120-130`, `file.rs:46-77`; bump: `image.rs:152-191`,
  `file.rs:93-127` — CopyPaste-8ebg.57.)*
- **I-25.** Content-addressed `item_id` is required for *cross-device*
  convergence: a random per-capture `item_id` gives the same image a different
  identity on every device, so LWW never fires and duplicates accumulate forever.
  *(`image.rs:121-130`.)*
- **I-26.** The image content hash MUST be a cryptographic digest, not a
  `DefaultHasher`/time-mixed value. It must be deterministic across runs and
  processes, otherwise dedup silently stops working.
  *(`meta.rs:13-20` — security LOW #19.)*
- **I-27.** A thumbnail MUST be encrypted with the same content key but a
  **distinct, domain-separated, deterministic** `file_id`
  (`SHA-256("copypaste-thumb-v1" ‖ file_id)[..16]`), so its AEAD AAD can never
  collide with the full image's while remaining recomputable by a reader.
  *(`meta.rs:22-36`.)*
- **I-28.** When an insert is deduplicated by the storage layer against a
  pre-existing row, the value broadcast to sync subscribers MUST be the
  **stored** row, never the rejected candidate. Broadcasting the rejected id
  makes every subscriber look up a row that does not exist.
  *(`text.rs:243-270` — fix MED #4.)*
- **I-29.** Every captured row MUST be stamped with: a content-derived or fresh
  `item_id`, the stable on-disk `origin_device_id`, a lamport timestamp in the
  unified value space (`next_lamport_ts(0, now)` — never a hardcoded `0`), and
  the frontmost `app_bundle_id` when known.
  *(`text.rs:205-225`, `image.rs:115-136`, `file.rs:70-81` — CopyPaste-ojhe.)*
  A `0`-stamped capture can never win LWW against an older pin/delete derived
  from a small counter.
- **I-30.** An item MUST be marked sensitive if **either** the content matches a
  high-confidence credential pattern **or** the frontmost app is a known
  credential store. App-origin alone is sufficient — a freshly generated
  password is a high-entropy random string that no content pattern will match.
  *(`text.rs:73-78`, `image.rs:34-39`, `file.rs:41-44` — mtf5 / PG-22.)*
- **I-31.** A sensitive item's expiry MUST be derived from the **user-configurable**
  sensitive TTL. Reading a different, non-user-settable TTL field makes the
  setting dead.
  *(`text.rs:227-235` — CopyPaste-8ebg.1.)*
- **I-32.** Re-copying a sensitive item MUST recompute its expiry from *now*,
  not leave it pinned to the original capture's deadline.
  *(`text.rs:140-157` — CopyPaste-8ebg.2.)*
- **I-33.** A dedup lookup failure MUST fall through to insert. Losing a capture
  is worse than storing a duplicate.
  *(`text.rs:188-192`.)*
- **I-34.** Every row-fetch between "find" and "bump" MUST tolerate the row having
  been concurrently deleted: produce no broadcast, log at debug, continue. Never
  panic, never unwrap.
  *(`text.rs:128-137`, `text.rs:170-178`, `image.rs:180-190`, `file.rs:116-126`.)*
- **I-35.** Capture MUST always store locally, regardless of whether sync is
  enabled. `sync_enabled` gates the outbound transports only.
  *(`text.rs:428-455`.)*

### 2.6 Failure posture

- **I-36.** No failure inside the capture pipeline may kill the poll loop. Encode
  failure, encrypt failure, DB failure, blocking-task panic, subprocess panic,
  malformed pasteboard payload — each logs and returns "no item this tick".
  *(`text.rs:290-296`, `image.rs:198-217`, `file.rs:134-153`, `tick.rs:328-352`,
  `frontmost.rs:125-134`, `monitor.rs:329-335`.)*
- **I-37.** A malformed / non-parseable pasteboard payload MUST NOT panic. In
  particular the legacy filenames payload is an attacker-adjacent binary blob
  written by arbitrary third-party apps.
  *(`macos_util.rs:183-192`, CopyPaste-q5ab.)*
- **I-38.** Unsupported clipboard types MUST be observable but MUST NOT flood the
  log: log once per distinct type per process.
  *(`macos_util.rs:8-29`.)*
- **I-39.** A size rejection MUST be observable as more than a log line — it must
  increment a readable counter, because "silently not working" and "rejecting
  oversized content" are indistinguishable to a user otherwise.
  *(`monitor.rs:37-45`, CopyPaste-8ebg.57.) **See §6.5: v1 wired the counter but
  never exposed it, and the image path bypasses it — fix in the rewrite.**

---

## 3. Edge cases & quirks

Each entry: **Rule** / **Why** / **Bug id** / **v1 site**.

### 3.1 `changeCount` is the only change signal, and it is lossy

- **Rule.** Treat `changeCount` as a monotonically increasing counter, not a
  sequence you can replay. A delta > 1 means intermediate clipboard values
  existed and are **irrecoverable** — NSPasteboard does not buffer history.
- **Why.** Users copy faster than any sane poll interval. The OS keeps only the
  latest.
- **Bug id.** *(unlabelled "CRITICAL fix" in v1)*
- **v1 site.** `monitor.rs:431-458`.
- **Amended for Windows (ADR-0013).** `GetClipboardSequenceNumber` is not the
  same kind of counter. Measured on Windows 11 26200: it moves once per
  *mutation*, so one text copy moves it by 5 — `EmptyClipboard`,
  `CF_UNICODETEXT`, and the `CF_TEXT`/`CF_OEMTEXT`/`CF_LOCALE` Windows
  synthesises — and one image copy by 4. The delta therefore carries no count of
  lost values, because the synthesised set depends on what the other application
  wrote. Windows reports no burst telemetry rather than a number that read 4
  after every single copy. §3.2 is unaffected: it is about what is captured.

### 3.2 Burst handling MUST NOT discard the surviving item ⚠ highest-value rule here

- **Rule.** When the delta is at/above the burst threshold, emit telemetry and
  **fall through** to capture the current pasteboard value. Never return a
  "burst happened" event *instead of* the content.
- **Why.** The original code advanced the cursor and early-returned a
  `SkippedBatch` event. The next poll then saw `count == last_change_count` and
  returned nothing — so the **most recent clipboard item was permanently lost**
  every time a user copied three things quickly. This is the single worst bug in
  the subsystem's history: burst detection silently ate the one item the user
  actually wanted.
- **Bug id.** *(unlabelled "CRITICAL fix"; downstream cleanup CopyPaste-mdhx)*
- **v1 site.** `monitor.rs:440-458`, `tick.rs:354-376`.
- **Rewrite note.** Do not model the burst as a content variant at all (§6.1).

### 3.3 Self-write suppression is a two-sided sentinel protocol

- **Rule.** The writer and the reader share one atomic "expected change count"
  cell, with sentinel `-1` = "no pending self-write". The protocol:
  1. Writer reads `pre = changeCount`.
  2. Writer stores `pre + 2` into the cell **before** touching the pasteboard.
     (`clearContents` increments by 1; `setString/setData:forType:` increments by 1.)
  3. Writer performs `clearContents` + `set…:forType:`.
  4. Writer reads `actual = changeCount`. **Only if `actual == pre + 2`** does it
     overwrite the cell with `actual`.
  5. On *any* write failure or error return, the writer resets the cell to `-1`.
  6. The poller, on seeing `changeCount == cell` (and `cell >= 0`), advances the
     cursor, records nothing, and **consumes** the sentinel by resetting it to `-1`.
  Use acquire/release ordering on this cell.
- **The `+2` is wrong on macOS 14, measured.** `clearContents` +
  `setString:forType:` moves `changeCount` by **1**, not 2 (CI run
  30632553103, macOS 14.8.7: `Observed 15 -> 16`). The delta is a *prediction*
  the pre-stamp needs, so predicting 2 leaves the sentinel armed at a count
  that never arrives: our own paste-back is captured as a fresh item — the
  duplicate-on-copy bug this protocol exists to prevent — and the stale
  sentinel then suppresses whichever genuine copy next lands on `pre + 2`.
  Both halves of §3.3 fail from one wrong constant. The protocol is otherwise
  as described; only the number is wrong, and `clearContents` returns the
  change count it produced, so the writer need not predict one at all.
- **Why (step 2, pre-stamp).** The original code stamped the change count
  *after* the write. A poll landing in the window between the write and the
  stamp saw an incremented `changeCount` with an unset sentinel, and recorded the
  just-pasted item as a brand-new capture — the duplicate-on-copy bug.
- **Why (step 4, conditional post-stamp).** If a third-party app wrote to the
  pasteboard between our write and our read, `actual > pre + 2`. Unconditionally
  storing `actual` would stamp *their* change count, causing the monitor to
  suppress **their** content as if it were ours — silently dropping a genuine
  user copy.
- **Why (step 5, reset on error).** A stale sentinel that is never consumed
  permanently suppresses a future genuine capture that happens to land on that
  change count.
- **Why (step 6, consume once).** The suppression is for exactly one change.
- **Bug id.** Fix-4 / "DUP-ON-COPY"; conditional post-stamp = **CopyPaste-8yzf**.
- **v1 site.** `monitor.rs:32-36`, `monitor.rs:412-429`;
  `handlers_items_paste.rs:118-136`, `:189-243`, `:339-348`, `:394-400`.
- **Also note.** The same sentinel is reused (not duplicated) by sync auto-apply
  and relay auto-apply, so remotely-synced items written to the pasteboard are
  not re-captured as local ones. *(`daemon/mod.rs:676-690`, `:752-760` — CopyPaste-7ub.)*
  The rewrite MUST keep this a single shared primitive.

### 3.4 `org.nspasteboard.*` markers — probe before read, all three

- **Rule.** Probe `org.nspasteboard.TransientType`,
  `org.nspasteboard.ConcealedType`, `org.nspasteboard.AutoGeneratedType` as a
  set (one "any of these available?" query). Presence of any ⇒ drop the change.
- **Why.** This is the de-facto cross-vendor convention (nspasteboard.org)
  honoured by 1Password, Bitwarden, KeePassXC, Maccy, etc. `ConcealedType` is
  how a password manager says "this is a secret, do not persist it";
  `TransientType` is "this is a scratch value"; `AutoGeneratedType` is "a machine
  produced this, not a human". A clipboard manager that ignores these markers
  will happily write users' master passwords into its searchable history.
  Probing *before* reading is the difference between "we never had the secret"
  and "we had the secret in memory and then dropped it".
- **Bug id.** *(Maccy parity / security requirement, unlabelled)*
- **v1 site.** `monitor.rs:174-203`; test `tests/clipboard.rs:312-332`.
- **Caveat carried forward.** A pasteboard carrying a marker *and* a normal
  string is still dropped entirely — that is intentional and correct.

### 3.5 `NSFilenamesPboardType` is a binary plist, not a URL string

- **Rule.** Read `NSFilenamesPboardType` with a **data** accessor and parse it as
  a binary property list containing an array of absolute POSIX path strings.
  Take the first absolute entry. Do **not** call a string accessor on it, and do
  **not** strip a `file://` prefix from it — the entries are bare POSIX paths
  with no scheme and no percent-encoding.
- **Why.** The original code called `stringForType` and then stripped `file://`.
  A binary plist is not a plain string, so the accessor returned nil and **every
  file copied from Finder was silently discarded**. The failure was invisible:
  no error, no log, the file branch just never fired.
- **Bug id.** **CopyPaste-q5ab**
- **v1 site.** `monitor.rs:291-340`; tests `macos_util.rs:141-192`.
- **Malformed payload.** Third-party apps put garbage on this type. Parse failure
  ⇒ debug log ⇒ skip. Never panic.

### 3.6 `public.file-url` is a percent-encoded URL string

- **Rule.** Strip the `file://` scheme prefix, percent-decode `%HH` sequences,
  require the result to be absolute. Invalid `%` sequences pass through
  unchanged rather than erroring. Invalid UTF-8 after decoding falls back to
  lossy conversion rather than dropping the path.
- **Why.** Paths with spaces (`%20`) and non-ASCII names are the common case, not
  the exotic one. A naive path-from-string yields a nonexistent file.
- **Bug id.** *(unlabelled)*
- **v1 site.** `monitor.rs:266-289`; `macos_util.rs:38-70`.
- **Rewrite note.** Use a maintained percent-decoding crate; keep the
  "invalid sequence passes through" and "lossy on invalid UTF-8" behaviours.

### 3.7 Promised / lazy pasteboard data is deliberately unsupported

- **Rule.** `com.apple.pasteboard.promised-file-url` is treated as an
  *unsupported kind*: detected, logged once, never resolved.
- **Why.** Promised files require driving the promise protocol (the source app
  materialises the file on demand into a destination directory you supply). v1
  never implemented it, so Finder drag-promises and some browser image drags are
  a known capture gap. This is a *known limitation*, not an oversight — recording
  it here so the rewrite makes an explicit decision instead of rediscovering it.
- **Bug id.** *(edge HIGH #7 — "unsupported types")*
- **v1 site.** `monitor.rs:81-93`, `monitor.rs:361-379`.

### 3.8 Unsupported-type probing is an allowlist, not an enumeration

- **Rule.** Probe a fixed list of known-unsupported UTIs
  (`public.rtf`, `public.rtfd`, `public.html`, `public.url`,
  `com.apple.pasteboard.promised-file-url`) and only when text, image, and file
  are all absent. Log each distinct kind once per process.
- **Why.** Enumerating `pb.types()` needed an API surface v1's binding version
  didn't expose; and repeated RTF copies (e.g. inside a text editor) would
  otherwise emit a log line every 500 ms forever.
- **Bug id.** *(edge HIGH #7)*
- **v1 site.** `monitor.rs:361-379`, `macos_util.rs:8-29`; test
  `macos_util.rs:118-139`.
- **Rewrite note.** Enumerating the real type list is *better* — but keep the
  once-per-kind log gate.

### 3.9 Frontmost-app resolution: fail-closed, cached, and always-on

Three separate hard-won rules, all on the same subsystem:

- **Rule (a) — always resolve.** Resolve the frontmost app on every tick, even
  when the exclusion list is empty.
  **Why.** An earlier optimisation short-circuited resolution to "unknown" when
  the exclusion list was empty (the default). That made the second consumer —
  sensitive-app classification — receive "unknown" forever, so passwords copied
  from 1Password/Bitwarden/etc. were **never flagged sensitive and never
  auto-wiped** on a stock configuration.
  **Bug id.** **CopyPaste-44rq.43** (regressed by **CopyPaste-zdyw**).
  **v1 site.** `tick.rs:79-95`; test `tick.rs:419-490`.
- **Rule (b) — fail closed on the exclusion gate only.** Unknown frontmost app +
  non-empty exclusion list ⇒ skip capture this tick (advance cursor). Unknown +
  empty list ⇒ proceed.
  **Why.** Otherwise a transient resolution failure silently captures from an
  app the user explicitly excluded. Failing closed with an empty list would
  disable capture entirely for everyone, which is worse than the risk it avoids.
  **Bug id.** P1-2.
  **v1 site.** `tick.rs:99-129`.
- **Rule (c) — TTL-cache the result, including failures.** Cache for slightly
  more than one poll period. Cache negative results too.
  **Why.** The v1 mechanism forks a subprocess (`lsappinfo front`); forking
  every 500 ms is a measurable battery cost. Caching failures prevents a
  fork-storm during a transient error. **But** the TTL was originally 2000 ms
  (4× the tick), which meant a copy made shortly after an app switch could reuse
  a stale bundle id for up to ~1.5 s — **bypassing both the exclusion list and
  sensitive-app detection for that window**. Tightened to 750 ms: just above the
  tick period, so at most one extra tick can observe a stale value.
  **Bug id.** **CopyPaste-44rq.33** (cache), **CopyPaste-8ebg.57** (2000 → 750 ms).
  **v1 site.** `frontmost.rs:6-61`, `frontmost.rs:91-148`; test `frontmost.rs:167-233`.
- **Known limitation to fix in the rewrite (PRIV-6 / PRIV-2).** v1 shells out to
  `lsappinfo`, an undocumented Apple CLI. It can be absent, sandboxed, or return
  a helper process's bundle id instead of the password manager's. The documented
  long-term fix is `NSWorkspace.frontmostApplication` / the Accessibility API.
  The rewrite SHOULD use the framework API; the surrounding rules (a)/(b)/(c)
  still apply, and (c) becomes cheap enough that the cache may be unnecessary —
  but the *staleness bound* in (c) must still be honoured.
  *(`tick.rs:47-66`.)*

### 3.10 Both size gates for the same content type must agree

- **Rule.** The read gate (how many bytes we accept off the pasteboard) and the
  encode gate (how many bytes the storage layer will accept) MUST be driven by
  the same user-configured value, and MUST hot-reload together.
- **Why.** The monitor originally defaulted its image read gate to the library's
  hardcoded floor while the encoder used the user's configured cap. Any
  configured value above the library floor was silently ineffective — images
  between the two limits were rejected at read time with no explanation.
- **Bug id.** *(unlabelled; see `monitor.rs:20-31` and `daemon/mod.rs:786-789`)*
- **v1 site.** `monitor.rs:118-137`, `monitor_loop.rs:124-133` / `:222-231`,
  `image.rs:41-58`.

### 3.11 Non-macOS behaviour

- **Rule.** On non-macOS the poll is a no-op returning "no content". The whole
  crate MUST still compile under `-D warnings`, which in v1 required cfg-gating a
  large number of imports, helpers, and constants that only the macOS path uses.
- **Why.** CI runs on Linux; MSRV/clippy jobs are green-gated.
- **Bug id.** **CopyPaste-l07l** (also: the tokio `select!` macro rejects
  attributes on branches, forcing the non-macOS SIGTERM future to be a boxed
  always-defined future rather than a `#[cfg]`-ed branch).
- **v1 site.** `monitor.rs:4-15`, `monitor.rs:506-510`, `monitor_loop.rs:177-199`,
  `macos_util.rs:48`, `macos_util.rs:79`, `meta.rs:73`.
- **Rewrite note.** Model this as a platform trait with a null implementation, so
  the cfg-noise disappears instead of being ported. But keep the *contract*:
  non-macOS = silent no-op, never a panic or an error.
- **Amended for Windows (ADR-0013).** "Non-macOS" now means "no real backend",
  not "not macOS": Windows has one, and §2's invariants bind it exactly as they
  bind `NSPasteboard`. §3.4's opt-out markers are `Clipboard Viewer Ignore`,
  `ExcludeClipboardContentFromMonitorProcessing` and the `DWORD`-valued
  `CanIncludeInClipboardHistory` / `CanUploadToCloudClipboard`. The no-op
  contract still holds for every other target, which is what keeps Linux CI a
  test surface.

### 3.12 Cocoa string constants must not be reallocated per tick

- **Rule.** The invariant UTI strings (3 privacy markers, 2 image types, 2
  file types, 5 unsupported probes) are process-lifetime constants. Build each
  bridged string once and reuse it.
- **Why.** v1 allocated ~12 fresh Cocoa strings on every changed tick. They are
  immutable and thread-safe, so a lazily-initialised static is sound. Note these
  are *strong* references owned by the static and therefore deliberately NOT
  placed in the per-tick autorelease pool.
- **Bug id.** **CopyPaste-pbre**
- **v1 site.** `monitor.rs:48-94`.

### 3.13 Image ingest: reject on size before hashing, decode exactly once

- **Rule (a).** Apply the size cap **before** computing the content hash.
  **Why.** A SHA-256 pass over a 25 MB image costs ~25 ms of CPU and is then
  thrown away by the encoder's own size gate.
  **Bug id.** **CopyPaste-44rq.39**. **v1 site.** `image.rs:41-58`; test
  `image.rs:361-391`.
- **Rule (b).** Decode the clipboard image once and reuse the decoded bitmap for
  both the full-resolution re-encode and the thumbnail downscale.
  **Why.** Decoding twice doubles the CPU cost and the peak memory of the most
  expensive operation in the pipeline. **v1 site.** `image.rs:71-85` (Variant-B).
- **Rule (c).** The decode budget (max decoded pixels/MB) MUST come from live
  config, not a compile-time default, because it is the decompression-bomb
  defence.
  **v1 site.** `image.rs:78-84`.
- **Rule (d).** Thumbnail failure MUST NOT fail the capture. An empty thumbnail
  blob is stored as "no thumbnail" and regenerated lazily later.
  **v1 site.** `image.rs:105-113`.
- **Rule (e).** Boundary: `len == cap` is accepted; `len > cap` is rejected.
  **v1 site.** `image.rs:51`, test `image.rs:355-360`.

### 3.14 File ingest: bytes are stored verbatim

- **Rule.** Unlike images, file bytes are chunked and encrypted **as-is** — no
  decode, no re-encode, no normalisation. The filename is sanitised only at
  paste-back time (basename extraction), not at capture.
- **Why.** A file's bytes are its identity; re-encoding would corrupt it and
  break the content hash's cross-device convergence.
- **v1 site.** `file.rs:1-2`, `file.rs:46-59`; sanitisation at
  `handlers_items_paste.rs:71-77`.

### 3.15 MIME is derived from the extension, best-effort

- **Rule.** Map the lowercased file extension through a table; unknown ⇒
  `application/octet-stream`. Source-code extensions map to `text/plain`.
- **Why.** No content sniffing at capture time (cost + the file may be huge).
- **v1 site.** `macos_util.rs:72-112`.
- **Rewrite note.** Replace the hand-rolled table with a maintained crate; keep
  the `application/octet-stream` fallback and the "no sniffing" rule.

### 3.16 Metadata JSON must be additive and forward-compatible

- **Rule.** Readers ignore unknown keys. New fields (e.g. thumbnail id and
  dimensions) are added alongside existing ones; existing key names and shapes
  are frozen. Both image and file metadata carry a `file_id` under the *same*
  key so one parser serves both. Filenames and MIME types MUST be JSON-escaped.
- **Why.** These blobs are persisted in user databases across versions; a shape
  change orphans rows.
- **v1 site.** `meta.rs:38-84`; back-compat readers `ipc/pasteboard.rs:42-104`,
  `:195-229`, `:239-273`.
- **Rewrite note.** v1 builds this JSON with `format!` string interpolation
  (`{:?}` debug-formatting of a byte array). Use a real serializer — but emit the
  **identical** shape, including the byte-array representation of ids, or you
  break every existing row.

### 3.17 TTL cleanup: the `0` sentinel means "disabled", not "expire now"

- **Rule.** A sensitive-TTL of `0` means auto-wipe is disabled. It MUST short-
  circuit the cleanup entirely and MUST cause recency-bumps to leave `expires_at`
  untouched.
- **Why.** Treating `0` as a duration makes the threshold equal `now`, which
  **deletes every sensitive item on every cleanup tick** — the exact opposite of
  the user's "turn it off" intent.
- **Bug id.** P2.
- **v1 site.** `monitor_loop.rs:114-123`, `text.rs:140-150`.

### 3.18 Cheap existence probe before the expensive prune

- **Rule.** Gate the sensitive-TTL delete scan behind a cheap "does any sensitive
  row exist?" query.
- **Why.** On a machine that has never copied anything sensitive, the delete
  scan plus its write transaction ran every 5 seconds forever, for nothing. The
  TTL guarantee is preserved because the probe returning true always runs the
  full prune.
- **Bug id.** **CopyPaste-98ja**
- **v1 site.** `cleanup.rs:39-54`; test `cleanup.rs:115-155`.

### 3.19 Startup purge before the IPC socket is bound

- **Rule.** Run the TTL purge once at startup, *before* the daemon accepts IPC
  connections.
- **Why.** Otherwise a client can read sensitive rows that should already have
  expired while the daemon was stopped.
- **Bug id.** P2 / ugv7.
- **v1 site.** `daemon/mod.rs:353-357`; test `cleanup.rs:169-217`.

### 3.20 Dedup lives in ingest, not in the monitor

- **Rule.** The change-detection layer MUST NOT dedup by content. Re-copying the
  same text bumps `changeCount`, so the monitor re-emits it; ingest is what
  collapses it into a recency bump.
- **Why.** The monitor's only natural dedup is "unchanged `changeCount` ⇒ no
  event". Adding content dedup there would hide legitimate re-copies from
  ingest, which needs to see them in order to bump recency.
- **v1 site.** contract pinned in `tests/clipboard.rs:261-310`.

### 3.21 Poll interval, size gates, and feature flags hot-reload

- **Rule.** Re-read live config every tick. When the poll interval changes,
  recreate the ticker (and reset the cleanup tick counters so a large interval
  change does not trigger a spurious early cleanup). Push the current size caps
  into the read gate every tick.
- **Why.** Settings changes must not require a daemon restart.
- **Bug id.** **CopyPaste-at2m**
- **v1 site.** `monitor_loop.rs:100-133`, `:207-231`.

### 3.22 The broadcast channel must absorb bursts

- **Rule.** The new-item broadcast channel needs a buffer large enough for a
  clipboard burst plus a momentarily-backpressured subscriber.
- **Why.** At 64 slots, a rapid copy loop or a network-jittered P2P subscriber
  produced lag errors and subscribers **silently dropped items**. Raised to 256.
- **Bug id.** audit HIGH #8.
- **v1 site.** `daemon/mod.rs:363-372`.

### 3.23 Sound-on-copy is suppressed in test environments

- **Rule.** The capture-completed sound MUST be suppressed when the daemon is
  running with an ephemeral key (test mode), and the player process MUST be
  reaped.
- **Why.** OS hangs and sound spam in CI; zombie processes otherwise.
- **v1 site.** `tick.rs:167-181`, `:203-213`.

---

## 4. Constants & tunables

| Name | Value | Rationale / what breaks if changed |
|---|---|---|
| Poll interval (default) | **500 ms** | Perceived-instant capture vs. wake-ups. Everything else (frontmost cache TTL, cleanup divisors) is expressed relative to it. `config/defaults.rs:5` |
| Poll interval (min) | **100 ms** | Below this the poll loop's own CPU cost becomes visible. Clamped on config load. `config/defaults.rs:6` |
| Poll interval (max) | **5000 ms** | Above this the daemon feels broken; also bursts become the norm rather than the exception. `config/defaults.rs:7` |
| Burst threshold (changeCount delta) | **3** | Delta of 1 = normal; 2 = a paste-back pair (`clearContents` + `set…`); ≥3 = a genuine burst worth reporting. `content.rs:12` |
| Frontmost-app cache TTL | **750 ms** | Must be **just above** one poll period: amortises the lookup across consecutive ticks of the same app while bounding staleness to at most one extra tick. Was 2000 ms → up to ~1.5 s of stale bundle id bypassing the exclusion list and sensitive-app detection (**CopyPaste-8ebg.57**). `frontmost.rs:18` |
| Sensitive-TTL cleanup interval | **5 s** | Sensitive items must disappear promptly; 5 s is short enough to feel automatic, long enough that the gated scan (§3.18) is negligible. `monitor_loop.rs:24` |
| General-TTL cleanup interval | **60 s** | Non-sensitive expiry has no urgency. `monitor_loop.rs:27` |
| Sensitive TTL (default) | **30 s** in v1; **`0` — off — in v2** | 30 s is long enough to paste a password once, short enough that it does not linger, and is still the value the setting should offer when a user turns it on. It is not v2's *default*: v2 has no Settings control for the TTL and the sweep raises no notice, so the delete would be silent, irreversible and undiscoverable — CLAUDE.md rule 4 by way of rule 6. Restore the 30 s default once a Settings control and a visible notice both exist. `0` = disabled (§3.17). `config/defaults.rs:51`; v2 `copypaste_ipc::ConfigData::sensitive_ttl_secs` |
| Autowipe confidence floor | **0.70** | Below this, low-signal patterns (phone numbers, order ids) triggered the 30 s wipe on ordinary text. `sensitive/detector/engine.rs:141` |
| Max text (default) | **10 MiB** | Kept under the 16 MiB P2P/IPC wire-frame cap so a storable item is always transportable. `config/defaults.rs:9` |
| Max text (floor) | **64 KiB** | Below this ordinary copied text is rejected. `config/defaults.rs:42` |
| Max image (default) | **64 MiB** | High-res screenshots at original quality. `config/defaults.rs:11` |
| Max image (floor) | **1 MiB** | Below this even a small screenshot is rejected. `config/defaults.rs:44` |
| Library image floor | **10 MiB** | The storage layer's own hard constant. **Not** the read gate — see §3.10; using it as the read gate silently voided larger user configs. `core/image/mod.rs:35` |
| Max file (default & hard cap) | **100 MiB** | The storable hard cap; the user knob is clamped to it, so the default is honest. `config/defaults.rs:29`, `core/file.rs:23` |
| Max file (floor) | **1 MiB** | Keeps file capture usable. `config/defaults.rs:46` |
| Sync blob cap | **8 MiB** | Files 8–100 MiB are stored locally but **skipped for sync** (warned). A capture is not a promise of syncability. `config/defaults.rs:17-19` |
| Max decoded image | **50 MiB** | Decompression-bomb budget; must come from live config (§3.13c). `config/defaults.rs:69` |
| Thumbnail max dimension | **192 px** | Retina-crisp at typical list-row size. Rows written under an older, larger cap are detected via stored thumb dims and regenerated. `core/image/mod.rs:49` |
| Content hash width | **16 bytes** (SHA-256 prefix) | 128-bit collision resistance; also exactly a UUID's width, which is what makes `item_id = UUID(file_id)` work. `meta.rs:15-20` |
| Thumb id domain separator | `"copypaste-thumb-v1"` | Frozen: changing it orphans every stored thumbnail's AAD. `meta.rs:30` |
| Storage quota (default) | **10 GiB** | The only local bound (there is no row-count cap). `config/defaults.rs:31` |
| Storage quota (floor) | **50 MiB** | A past bug set this to 200 bytes; the byte-cap prune then evicted nearly every unpinned row after **every insert**, producing self-clearing history and dropped images. `config/defaults.rs:34-40` |
| Broadcast channel capacity | **256** | 64 dropped items on bursts (§3.22). `daemon/mod.rs:372` |
| Paste-file staging max age | **10 min** | Files are not deleted immediately after paste because the receiving app may read the URL asynchronously. `ipc/pasteboard.rs:311` |
| Self-write sentinel "none" | **-1** | Must be outside the valid `changeCount` domain (non-negative). `monitor.rs:104` |
| Change-count cursor initial | **-1** | Same reason; also suppresses the first-poll burst signal. `monitor.rs:100`, `:433` |
| Expected self-write delta | v1: **+2**; measured on macOS 14.8.7: **+1** | v1's value, from `handlers_items_paste.rs:212`, is contradicted by the first run that ever measured it (§3.3). Wrong in either direction it breaks self-write suppression *and* eats a later genuine copy |

---

## 5. Acceptance tests to re-create

Grouped; each is given/when/then and implementation-independent. `[macOS]` marks
tests requiring a real window server (v1 marks these ignored-by-default and
serialised, because the general pasteboard is a process-global singleton —
keep both properties).

### 5.1 Change detection

- **T-1 — idle clipboard is free.** Given the clipboard has not changed since the
  last poll, when polling, then no content is returned **and** no pasteboard
  representation is read. *(Assert via a fake/injected pasteboard that records
  accessor calls — v1 could not test this because it had no seam; the rewrite
  MUST introduce one.)* Guards I-1.
- **T-2 — first poll is not a burst.** Given a freshly constructed monitor
  (cursor at sentinel), when the first change arrives with an arbitrary change
  count, then no burst is reported and the content is captured. Guards I-2.
- **T-3 — cursor advances on every drop path.** For each of {privacy marker,
  self-write, private mode, excluded app}: given the drop condition holds, when
  polled twice with no further clipboard change, then the second poll returns
  nothing (i.e. the change was acknowledged, not re-offered). Guards I-3.
- **T-4 `[macOS]` — unchanged pasteboard yields nothing.** Given content was
  captured, when polling again without writing, then nothing is returned.
  *(v1: `tests/clipboard.rs:303-309`.)*

### 5.2 Burst handling — the regression that matters most

- **T-5 — burst does not eat the survivor.** Given the cursor is at N, when the
  clipboard's change count jumps to N+5 with text `"latest"` present, then the
  poll returns `"latest"` (and separately reports 4 lost intermediates via
  telemetry). **The poll MUST NOT return a burst-only result.** Guards §3.2.
  The telemetry half does not bind on Windows, where the counter counts
  mutations rather than changes (§3.1); the survivor half binds everywhere.
- **T-6 — burst then normal resumes.** Following T-5, given one further single
  clipboard write `"after-burst"`, when polled, then `"after-burst"` is returned
  as ordinary text.
- **T-7 — threshold boundary.** Delta of 2 reports no burst; delta of 3 reports a
  burst of 2 lost intermediates. Guards the constant.

### 5.3 Self-write suppression

- **T-8 — happy path.** Given the writer pre-stamps `pre+2`, writes, and the
  post-write count equals `pre+2`; when the monitor polls, then nothing is
  captured, the cursor advances to that count, and the sentinel is reset to
  "none".
- **T-9 — suppression is one-shot.** Following T-8, given a genuine user copy
  bumps the count again, when polled, then the new content **is** captured.
- **T-10 — poll racing the write.** Given the monitor polls in the window
  between `clearContents` and `set…:forType:`, then the just-pasted item is not
  recorded as a fresh capture. (Pre-stamping is what makes this pass; a
  post-stamp-only implementation fails it.) Guards the Fix-4 rationale.
- **T-11 — third-party write after ours.** Given the post-write count is
  `pre+3` (a third-party app wrote in between), when the writer post-stamps,
  then the sentinel is **not** updated to `pre+3`, and a subsequent poll at
  count `pre+3` **captures** the third-party content. Guards **CopyPaste-8yzf**.
- **T-12 — write failure clears the sentinel.** For each writer error path
  (missing content, decrypt failure, non-UTF-8 plaintext, `set…` returning
  false, blocking-task panic): given the failure, then the sentinel is reset to
  "none" and a subsequent genuine capture is not suppressed.
- **T-13 — sync auto-apply shares the sentinel.** Given a remotely-synced item is
  written to the pasteboard by the sync path, when the monitor polls, then it is
  not re-captured as a local item.

### 5.4 Privacy markers

- **T-14 `[macOS]` — concealed-only pasteboard yields nothing.** Given a
  pasteboard declaring only `org.nspasteboard.ConcealedType` with a payload,
  when polled, then no content is returned. *(v1: `tests/clipboard.rs:316-332`.)*
- **T-15 — each marker independently suppresses.** Parameterised over
  `TransientType`, `ConcealedType`, `AutoGeneratedType`: given the marker is
  present **alongside a normal text representation**, when polled, then nothing
  is captured and no accessor for the text representation was ever called.
  Guards I-5 (probe-before-read).
- **T-16 — marker advances the cursor.** Given a marked change, when polled
  twice, then the second poll returns nothing (no re-offer loop).

### 5.5 Representation priority

- **T-17 `[macOS]` — text wins over image.** Given the pasteboard carries both
  text and PNG, when polled, then text is returned and the image bytes were
  never materialised (assert via allocation/accessor spy). Guards I-11, I-12.
- **T-18 `[macOS]` — PNG-only becomes an image.** Given only `public.png`, when
  polled, then the raw bytes round-trip unchanged and the item is typed
  "image". *(v1: `tests/clipboard.rs:171-195`.)*
- **T-19 — TIFF fallback.** Given only `public.tiff`, when polled, then the TIFF
  bytes are captured. Given both PNG and TIFF, then PNG is captured.
- **T-20 — file only when text and image absent.** Given a file URL plus text,
  then text wins. Given a file URL plus PNG, then the image wins. Given only the
  file URL, then a file reference is produced.
- **T-21 `[macOS]` — UTF-8 text round-trips.** Given text containing an em-dash
  and a check mark, when polled, then the exact string is returned.
  *(v1: `tests/clipboard.rs:143-166`.)*

### 5.6 File URLs

- **T-22 — legacy filenames plist parses.** Given a **binary** property list
  encoding an array of absolute POSIX paths, when parsed by the capture path,
  then the first path is recovered exactly (no scheme stripping, no percent
  decoding). Guards **CopyPaste-q5ab**. *(v1: `macos_util.rs:150-178`.)*
- **T-23 — malformed filenames payload is silent.** Given non-plist bytes, then
  parsing fails, the change is skipped, and nothing panics.
  *(v1: `macos_util.rs:183-192`.)*
- **T-24 — multi-file selection takes the first absolute entry.** Given a plist
  with `["relative/path", "/abs/one", "/abs/two"]`, then `/abs/one` is chosen.
- **T-25 — percent-decoding.** `file:///Users/a/My%20Doc.pdf` →
  `/Users/a/My Doc.pdf`. `%ZZ` passes through literally. A trailing bare `%`
  passes through. Non-ASCII (`%C3%A9`) decodes to `é`.
- **T-26 — non-absolute URL rejected.** Given `http://example.com/x`, then no
  file content is produced.
- **T-27 — MIME derivation.** `.pdf`→`application/pdf`, `.PNG`→`image/png`
  (case-insensitive), `.rs`→`text/plain`, `.xyz`→`application/octet-stream`,
  no extension →`application/octet-stream`.
- **T-28 — stat+read is atomic.** Given a file that is under the cap at stat time
  and over the cap at read time, then the size gate still rejects it — i.e. the
  two operations happen inside one blocking unit with no interleaving point.
  Guards **CopyPaste-b5iz**.
- **T-29 — read errors are non-fatal and PII-free.** Given the file is deleted
  between capture and read, then a warning is emitted containing **only the
  basename** and the tick completes normally.

### 5.7 Size gates

- **T-30 — oversized text is rejected and counted.** Given text over the cap,
  then no item is stored, an explicit "too large" outcome is produced, and the
  rejection counter increments by 1.
- **T-31 — oversized image is rejected and counted.** Same, for images. **This
  test fails against v1** — see §6.5 — and must pass in the rewrite.
- **T-32 — oversized image is never copied to the heap.** Given a pasteboard
  image far above the cap, then the capture allocates no buffer of that size.
  Guards I-18 / **CopyPaste-1f5c**.
- **T-33 — boundary.** `len == cap` is accepted; `len == cap + 1` is rejected.
  *(v1: `image.rs:361-391`.)*
- **T-34 — read gate follows configuration.** Given the configured image cap is
  raised above the library's internal floor, when an image between the two is
  copied, then it is captured. Guards §3.10.
- **T-35 — oversized file rejected pre-read.** Given a file whose size exceeds
  the cap, then its bytes are never fully read, and the rejection counter
  increments.

### 5.8 Text ingest

- **T-36 — re-copy bumps, does not duplicate.** Given the same text captured
  twice, then exactly one row exists. *(v1: `text.rs:311-344`.)*
- **T-37 — bump raises recency.** After a re-copy, the row's wall time is
  greater than or equal to the original, and its lamport stamp is strictly
  monotonic with respect to its own previous value. *(v1: `text.rs:349-378`.)*
- **T-38 — distinct text makes distinct rows.** Two different strings → two
  rows. *(v1: `text.rs:382-412`.)*
- **T-39 — dedup finds arbitrarily old rows.** Given a matching row far outside
  any recency window (including a pinned one), then it is found and bumped
  rather than duplicated. Guards I-23.
- **T-40 — ingest/read crypto agree.** Given a freshly captured text item, when
  decrypted by the production **read** path (dispatching on the row's stamped
  key version), then the original bytes are recovered. Guards I-22.
  *(v1: `text.rs:544-581`.)*
- **T-41 — dedup lookup failure still stores.** Given the dedup query errors,
  then the item is inserted anyway. Guards I-33.
- **T-42 — storage-layer dedup broadcasts the stored row.** Given the insert is
  collapsed against an existing row by a uniqueness constraint, then the value
  broadcast to subscribers is the **existing** row, whose id resolves. Guards
  I-28 / fix MED #4.
- **T-43 — concurrent delete during bump.** Given the row disappears between the
  dedup find and the bump, then no item is broadcast and nothing panics.
- **T-44 — local storage ignores the sync master switch.** With sync disabled,
  capture still stores locally. *(v1: `text.rs:428-455`.)*

### 5.9 Sensitivity & attribution

- **T-45 — password-manager origin forces sensitive.** Given text that matches no
  content pattern (e.g. `"xK9mQ3nR7pT2vW5"`) captured while a known credential
  store is frontmost, then the item is sensitive **and** records that bundle id.
  *(v1: `text.rs:465-497`.)*
- **T-46 — ordinary app + ordinary content is not sensitive.** But the bundle id
  is still recorded. *(v1: `text.rs:502-527`.)*
- **T-47 — classification is independent of the exclusion list.** With an empty
  exclusion list, known credential stores still classify as sensitive. Guards
  **CopyPaste-44rq.43**. *(v1: `tick.rs:438-490`.)*
- **T-48 — classification is substring and case-insensitive.** `"com.1Password.…"`,
  `"com.bitwarden.desktop"`, `"…keepass…"` all classify; `"com.apple.finder"`,
  `"com.google.chrome"`, `""` do not.
- **T-49 — content-pattern confidence floor.** High-confidence credentials (AWS
  key, JWT, PEM private key) trigger sensitivity; low-signal matches (phone
  number, order id) do not.
- **T-50 — images and files inherit app-origin sensitivity.** A screenshot taken
  while a credential store is frontmost is marked sensitive.

### 5.10 Frontmost-app cache

- **T-51 — cold cache is stale.** A newly constructed cache reports "not fresh",
  forcing a first-tick resolution. *(v1: `frontmost.rs:169-185`.)*
- **T-52 — hot cache is reused.** Within the TTL the cached value is returned and
  no resolution is performed. *(v1: `frontmost.rs:186-202`.)*
- **T-53 — failures are cached too.** A negative result within the TTL is reused
  (no resolution storm during a transient failure) and remains distinguishable
  from "never primed". *(v1: `frontmost.rs:213-232`.)*
- **T-54 — staleness bound.** With a 500 ms tick, a bundle-id change is observed
  by capture within at most 2 ticks. Guards the 750 ms choice
  (**CopyPaste-8ebg.57**); a 2000 ms TTL fails this.
- **T-55 — resolver output parsing.** A well-formed record yields the bundle id;
  empty input and unrecognised text yield "unknown", never a panic or a bogus
  value. *(v1: `frontmost.rs:239-257`.)*

### 5.11 Exclusion & private mode

- **T-56 — excluded app is skipped, cursor advanced.** Given the frontmost app is
  in the exclusion list, then nothing is captured and the change is not
  re-offered.
- **T-57 — fail closed.** Given the exclusion list is non-empty and the frontmost
  app is unknown, then capture is skipped for that tick and a warning containing
  no content is emitted. Guards P1-2.
- **T-58 — fail open with empty list.** Given an empty exclusion list and an
  unknown frontmost app, then capture proceeds normally.
- **T-59 — private mode suppresses and does not replay.** Given private mode is
  on for several changes, when it is turned off, then only *subsequent* changes
  are captured; nothing copied during private mode appears.

### 5.12 Image & file ingest

- **T-60 — image round-trips through the real read path.** Capture a PNG, then
  read it back through the production decode path; bytes match the canonical
  re-encoded PNG. *(v1: `image.rs:253-290`.)*
- **T-61 — rotated key fails loudly.** An image row encrypted under the previous
  key MUST fail decryption with an explicit error under a rotated key — never
  return garbage — and MUST still decode under its original key.
  *(v1: `image.rs:302-347`.)*
- **T-62 — identical images converge.** The same image captured twice yields one
  row whose recency is bumped on the second capture. Guards **CopyPaste-8ebg.57**.
- **T-63 — identical images on two devices share an identity.** The derived item
  identity is a pure function of the content bytes. Guards I-25.
- **T-64 — content hash is a deterministic digest.** Same input → same 16 bytes,
  across processes; different input → different bytes; equals the first 16 bytes
  of the SHA-256 digest. *(v1: `meta.rs:94-113`.)*
- **T-65 — thumb id is distinct, deterministic, domain-separated.** Never equal
  to the image's own id; stable across runs; derived only from the image id.
- **T-66 — thumbnail failure does not fail capture.** Given thumbnail generation
  yields nothing, the image item is still stored with "no thumbnail".
- **T-67 — decode bomb is bounded by live config.** An image whose decoded size
  exceeds the configured budget is rejected without exhausting memory.
- **T-68 — file bytes are stored verbatim.** Capture a file, read it back through
  the production path; bytes are byte-identical, and filename/MIME survive.
  *(v1: `image.rs:407-568` covers image+file through the real IPC read path.)*
- **T-69 — identical files converge and bump.** Mirrors T-62 for files.
- **T-70 — metadata JSON shape is frozen.** The emitted metadata parses with the
  production parsers and contains the exact historical key names for both image
  and file items; unknown extra keys are ignored by readers.
- **T-71 — filename/MIME are JSON-escaped.** A filename containing `"`, `\`, and
  a newline round-trips through metadata unchanged.

### 5.13 TTL, prune, lifecycle

- **T-72 — TTL of zero disables the wipe.** With sensitive TTL `0`, sensitive
  items survive indefinitely and recency bumps leave expiry untouched. Guards P2.
- **T-73 — no sensitive rows ⇒ no scan.** With zero sensitive rows and an
  aggressively short TTL, a cleanup pass deletes nothing (proving the delete did
  not run). Guards **CopyPaste-98ja**. *(v1: `cleanup.rs:115-155`.)*
- **T-74 — startup purge removes already-expired sensitive rows.** *(v1:
  `cleanup.rs:169-217`.)*
- **T-75 — pinned items are never pruned.** By the byte cap or by TTL.
- **T-76 — cleanup cadence is interval-based, not tick-count-based.** With poll
  intervals of 100 ms, 500 ms, and 5000 ms, the sensitive cleanup runs
  approximately every 5 s and the general cleanup approximately every 60 s
  (and at most once per tick when the poll interval exceeds the cleanup
  interval). Guards the `.max(1)` divisor clamp.
- **T-77 — poll interval hot-reload.** Changing the interval at runtime takes
  effect without a restart and does not trigger a spurious immediate cleanup.
  Guards **CopyPaste-at2m**.
- **T-78 — size caps hot-reload.** Raising the image cap at runtime immediately
  allows a previously-rejected image.

### 5.14 Resource & platform

- **T-79 — sustained polling does not grow memory.** Poll several thousand times
  across a mix of text/image/idle changes; resident and reserved memory return
  to baseline. Guards I-17 (autorelease drain).
- **T-80 — no zombie processes.** After many captures with the completion sound
  enabled, the process table contains no zombie children. Guards I-21.
- **T-81 — executor is never blocked.** During capture of a 100 MiB file, an
  unrelated async task continues to make progress within its normal latency
  budget. Guards I-20.
- **T-82 — non-macOS is a silent no-op.** On a non-macOS build, polling returns
  no content, never errors, and the daemon starts and stops cleanly.
- **T-83 — unsupported types log once per kind.** Repeated captures of the same
  unsupported type produce exactly one log record; a second distinct type
  produces one more. *(v1: `macos_util.rs:121-139`.)*
- **T-84 — broadcast burst is not dropped.** Enqueue a burst of captures with a
  momentarily slow subscriber; no item is silently lost.

---

## 6. Known-unjustified complexity — do NOT port

### 6.1 The `SkippedBatch` content variant

v1 models "a burst happened" as a **variant of the content enum**. After the §3.2
fix, the poll never produces it. v1 then kept the variant alive artificially —
its own comment says "implemented, not dead", justified by "clippy would flag it"
and "a future poll might restore it". That is complexity maintained for its own
sake:

- The variant forces every consumer to handle a case that cannot occur.
- `as_bytes()` returns an empty slice and `content_type()` returns
  `"skipped_batch"` — a fake content type that must be excluded everywhere.
- The alternate backend wrapper (`platform/macos.rs:39`) silently discards it.
- The public integration test `debounce_rapid_writes_only_emits_once_per_window`
  (`tests/clipboard.rs:205-259`) still asserts the **pre-fix** behaviour. It is
  `#[ignore]`d, so it has never failed — a permanently rotting test asserting a
  bug we deliberately removed.

**Port instead:** burst detection as a **counter/telemetry side-channel** (number
of lost intermediates), never as a content value. Delete the rotted test; replace
with T-5/T-6/T-7.

### 6.2 The eager `File { bytes, .. }` variant

The content enum has both `File { bytes, filename, mime }` and
`FileRef { path, filename, mime }`. **`File` is never constructed anywhere in the
workspace** — the poll only ever produces `FileRef`, and the tick handler's
`File` match arm (`tick.rs:216-245`) is unreachable code that nonetheless
duplicates the entire ingest call. Port one file representation: a reference
resolved on a blocking thread.

### 6.3 Two near-identical monitor loops

`monitor_loop.rs` contains the macOS and non-macOS loops as two ~90-line copies
differing only in (a) the tray quit-flag check and (b) threading the frontmost
cache. The file itself documents this ("differ only in… Both are preserved
verbatim"). Port one loop with platform hooks.

### 6.4 Hand-rolled standard-library replacements

Each avoided a dependency and each now has a maintained crate:

| v1 | Site | Replace with |
|---|---|---|
| `percent_decode_path` | `macos_util.rs:49-70` | `percent-encoding` |
| `mime_from_path` (26-arm extension table) | `macos_util.rs:80-112` | `mime_guess` (keep the octet-stream fallback) |
| `parse_lsappinfo_bundle_id` (string scraping of an undocumented CLI) | `frontmost.rs:69-80` | `NSWorkspace.frontmostApplication` (§3.9) |
| `build_image_meta_json` / `build_file_meta_json` via `format!` with `{:?}` byte-array debug formatting | `meta.rs:45-84` | a real serializer emitting the **identical** shape (§3.16) |
| Wi-Fi detection by scraping two `networksetup` subprocesses | `platform/macos.rs:143-218` | `SCNetworkReachability` / `NWPathMonitor` *(adjacent, but the same anti-pattern and the same TTL-cache workaround)* |

### 6.5 The rejection counter that nothing reads — and that the image path bypasses

`too_large_rejection_count` was added (**CopyPaste-8ebg.57**) so oversized
rejections would be visible as more than a log line. It is:

- **Never read** by any IPC handler, status response, or UI — the only readers
  are the field's own getter and the file branch's increment. The stated purpose
  ("a future IPC status response can expose it") never happened.
- **Bypassed for oversized images.** Because the §3.13/I-18 pre-copy size check
  (`monitor.rs:227-247`) turns an oversized image into "no image bytes", the
  post-read gate at `monitor.rs:476-485` — the only place that increments the
  counter for images and the only place that produces the "image too large"
  outcome — **is unreachable for exactly the case it was written for**. An
  oversized clipboard image is therefore dropped in complete silence: no error,
  no counter, no log (the unsupported-kind probe list does not include
  `public.png`).

**Port instead:** a single rejection signal emitted at the point of rejection
(wherever that is), that is actually surfaced through the status API. This is why
**T-31** is listed as an acceptance test that *fails against v1*.

### 6.6 Two parallel capture front-ends

`platform/macos.rs:25-42` defines a second clipboard front-end
(`MacosClipboardBackend`) wrapping the same monitor with a blocking
`thread::sleep` loop, which drops file, file-ref, and burst results and has no
self-write suppression, no privacy-marker awareness at the event level, and no
frontmost attribution. Nothing in the daemon's steady state uses it. Port one
capture entry point.

### 6.7 Documentation-only "contract tests"

Several v1 tests assert nothing about the code under test — they re-state a
contract in comments and then assert a trivially-true fact (e.g.
`tick.rs:400-417` asserts that a default config's exclusion list is empty and
comments that the real behaviour was "verified by code review";
`monitor.rs:528-541` re-implements the burst arithmetic inline and asserts the
re-implementation; `content.rs:148-155` asserts that a `Text` value reports type
`"text"` as a stand-in for the text-wins-over-image rule). These exist because
the monitor has **no seam** — it calls the global pasteboard directly, so nothing
can be driven from a test.

**Port instead:** a pasteboard abstraction (a trait/port with a fake
implementation) so that T-1, T-3, T-5, T-8..T-13, T-15..T-20, T-30..T-35 become
real, fast, platform-independent tests instead of comments. This is the single
highest-leverage structural change available in this subsystem: roughly two
thirds of the invariants in §2 are currently untestable, which is precisely why
several of them regressed at least once.

---

## 7. What is executed, and where

§6.7's fix landed: `clipboard/change.rs` is the change-count cursor and the
self-write sentinel as pure state, run by both backends and tested by an
ordinary `cargo test` on any host. What that cannot reach is the Cocoa half —
`changeCount`, `availableTypeFromArray`, `setString:forType:` — which no Linux
host compiles, let alone runs.

`.github/workflows/ci.yml`'s `macos-check` job runs it, on `macos-14`:
`cargo test -p copypaste-daemon -- --ignored --test-threads=1` drives the real
general pasteboard from `clipboard/macos.rs`. The release pipeline adds the
other end — `scripts/release/smoke-macos-dmg.sh` installs the DMG, starts the
daemon from inside the bundle and asserts that a `pbcopy` reaches the history.

### 7.1 Covered by something that runs

First run: CI 30632553103, `macos-14` (macOS 14.8.7, session `Aqua`). Five of
the six pasteboard tests passed on the first attempt. The sixth found a live
defect and has since been split in two, so that "what the OS does" and "what
our protocol does with it" carry separate verdicts — both rows below fail until
the constant is fixed.

| Rules | What is driven |
|---|---|
| I-1, I-2, T-4, T-21 | A write by another process moves `changeCount`; the poll returns the exact UTF-8; a second poll returns nothing; a first poll is not a burst |
| §4 | **FAILS.** The self-write delta is measured on the real pasteboard: it is **1**, not the 2 the sentinel is armed with |
| §3.3, T-8, T-9 | **FAILS, as a consequence.** Our own paste-back comes back as a capture, and the sentinel left armed at an unreachable count then eats the next genuine copy |
| §3.2, T-5, T-6 | Three fast writes: the survivor is returned, the losses are counted, capture resumes |
| I-5, §3.4, T-14, T-15, T-16 | All three `org.nspasteboard.*` markers, written *alongside* real text, drop the change and advance the cursor |
| I-18, I-39, T-30, T-33 | `cap + 1` bytes rejected and counted on the readable counter, `cap` bytes accepted |
| I-3 | Every drop path this backend has — marker, self-write, empty pasteboard — is polled twice and never re-offers |
| Status | `status` from the installed bundle reports `clipboard_backend = nspasteboard`, so a green run cannot be the fake |

### 7.2 Not covered, and why

- **Image, file and rich-text capture** — I-11..I-16, §3.5, §3.6, §3.7, §3.8,
  T-17..T-29, T-60..T-71. Not implemented; `Capture::content` is a `String`.
- **Frontmost attribution and the exclusion gate** — I-7, I-8, §3.9,
  T-45..T-58. Not implemented.
- **T-10, the poll racing the write.** No seam: `set_contents` performs
  `clearContents` and `setString:forType:` inside one call, so a test cannot
  interleave a poll between them. The pre-stamp that makes T-10 pass is
  asserted in `change.rs` instead, which is the ordering rule but not the race.
- **T-13, the shared sentinel.** Nothing to test at this level, and it holds
  structurally: `ClipboardSource::set_contents` is the only write path in the
  workspace, so sync auto-apply cannot grow a second protocol without adding a
  second writer.
- **Resource rules** — I-17 (autorelease drain), I-20 (executor), I-21
  (zombies), T-79..T-81. The pool is entered on every poll but nothing measures
  memory, thread blocking or the process table.
- **Hot reload** — §3.10, §3.21, T-34, T-77, T-78. The read gate is a constant;
  configuration is not wired to it yet.
- **macOS 15+.** The runner is pinned to `macos-14` deliberately: later
  versions raise a user prompt on programmatic pasteboard reads, which a
  headless runner has nobody to answer. Nothing here says what CopyPaste does
  on a version that prompts.
