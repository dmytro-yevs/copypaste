# Backlog — everything outstanding, in one place

**Last swept against the tree:** 2026-07-30, over `parity-audit.md` (19),
`ui-parity-audit.md` (12), `security-review.md` (14), `claims-audit.md` (24),
`android-clipboard-access.md`, all six ADRs, the manifest amendments, and the
tree itself. **Every item is re-checked against the working tree**, never copied
from its source document — the audits anchor at older commits and roughly half
of what they list has since closed.

**This is the only list of outstanding work that is maintained.** `README.md`
and `SECURITY.md` point here rather than keeping their own, because a prose
inventory of absence is falsified by every commit that lands a feature and
nothing fails when it is.

CLAUDE.md rule 9 requires unfinished work to live in an issue rather than a
commit message, and there is no issue tracker. This file is that register.

**How to read it.** §1 is what already closed — read it before starting
anything, because the largest single cost here has been items being closed
twice. §3 is the backlog, ranked by what a user loses: data loss, then security,
then a capability nobody can reach, then polish. Cost is a tiebreak, never the
sort key. §4 groups by what unblocks what — the ordering in §3 falls out of it.
§5 is decisions that look like debts; changing one is a reversal, not a fix.
§6 is work waiting on hardware or an account, which is not debt.

**Numbers are never reused.** A closed item's `B-n` moves to §1 and the gap
stays, so a reference from a commit, an audit or a source comment cannot come to
mean something else.

---

## 1. Closed — do not re-open

Verified in the tree. Each source document now marks its own row closed in
place; this table is where to look first, because it is one list rather than
four.

| Source | Item | Closed by |
|---|---|---|
| parity 1 | Pairing / peers / sync UI | `crates/copypaste-ui/src/components/devices/` (5 files, incl. `QrCode.tsx`, `QrScanner.tsx`) |
| parity 2 | Daemon lifecycle ownership | ADR-0004; `src-tauri/src/service/`; `components/shell/ServiceOffline.tsx` |
| parity 3 | Sensitive-item auto-wipe (TTL) | `core/src/sensitive/wipe.rs`, swept from the poll loop in `daemon/src/capture.rs`. Default `0` — off until a user asks, and `ServiceTab.tsx` is where they ask |
| parity 4 | Re-copy bumps to the top | `Store::insert_or_bump`, `core/src/storage/items.rs`; dedup is unbounded again (`retention.rs`) |
| parity 5 | Export / import | `Method::Export` / `Import`; `ipc/src/payload.rs` `ExportData` / `ExportItem` |
| parity 6 | Database backup / restore | `Method::Backup` / `Restore`; `daemon/src/server/dbadmin.rs`. **Partial** — `reset_database` and `vacuum` are still absent (B-11) |
| parity 9 | Daemon config, wire and CLI | `ipc/src/config.rs`, `daemon/src/settings.rs`, `Method::GetConfig` / `SetConfig`, CLI `config show` / `config set`. Its UI half is two rows below |
| parity 11 | Quick-Paste popup, global hotkey | `src-tauri/src/shell/{hotkey,window}.rs` |
| parity 12 · D1 | Stale-socket bind TOCTOU | `BindLock`, an exclusive `flock(2)` over probe→remove→bind, `daemon/src/server/listener.rs`. Distinct from F-9 (bind→chmod), which is still open as B-10 |
| parity 13 · S8 | IPC connection cap, read/write timeouts | `listener.rs` — `READ_TIMEOUT` 30 s, `WRITE_TIMEOUT` 10 s, `MAX_CONCURRENT_CONNECTIONS` 64, `MAX_WATCHERS` 8 |
| parity 14 · F-6 · S7 | Pairing codes never expire | `PAIRING_CODE_TTL` = 300 s, `p2p/src/peers/mod.rs`; refused on every read path, not one gate |
| parity 15 | Push / streaming updates | `Method::Watch`; `ui/src/hooks/usePush.ts` |
| parity 16 | Discovery not reachable | `Method::Discovered` / `Rescan` |
| parity 17 · F-10 | Undecryptable rows not counted | `ItemPage::skipped_undecryptable`; `components/history/SkippedNotice.tsx` |
| parity 19 | Bulk actions, filter, sort | `components/history/BulkBar.tsx`, `lib/view.ts`. Drag-to-reorder is B-23 |
| ADR-0003 | `reveal_item` unsupported on desktop | `Method::Get { id }`. `reorder_pinned` is the last thing `backend/daemon.rs` refuses, and its reason is stale — B-23 |
| ui-parity 4 (half) | No per-peer sync health on the wire | `PeerInfo::last_sync_ms`. UI half still open (B-16) |
| sec F-1 | Quoted credential values invisible to the detector | `sensitive/validators.rs` — `unquote()` strips one balanced pair before the code-shape gate |
| sec F-2 | `aws_secret_access_key = …` matched no rule | `sensitive/rules.rs`, its own rule at 0.99 |
| sec F-4 | Remote tombstone cleared `pinned` | **Decided, not fixed** — see §5 |
| sec F-7 | The mDNS claim was false on both sides | `SECURITY.md` and `p2p/src/discovery/record.rs`'s module doc both now say the id is a one-way domain-separated digest; `advertisement_carries_the_pairing_id_and_nothing_else_of_the_token` builds its record from a real `PairingToken` |
| sec F-8 · B-10(b) | PSKs copied unzeroized on every unauthenticated connection | `PeerStore::psks` returns `Zeroizing<Vec<PskCandidate>>`, so the next caller cannot re-create the hazard |
| sec F-11 · B-10(e) | `--data-dir` did not relocate the device secret | `Keyring::load_or_create(&data_dir)`; and a directory holding a database but no secret is now **refused** rather than minted into (`a_database_without_its_secret_is_refused_rather_than_re_keyed`) |
| sec F-12 · B-3 | No purge pass; `is_sensitive` never revisited | `core/src/sensitive/purge.rs`, run from `daemon/src/main.rs` before the socket binds. Index only — it never deletes a row and never rewrites the flag, deliberately; see §5 |
| sec F-13 · B-10(c) | Pairings uncapped, so `accept_any` scaled with the list | `MAX_PAIRINGS`, enforced by refusing a *new* pairing rather than evicting an old one |
| sec F-14 · B-10(d) | Reassembly `Vec` left peer plaintext in freed heap | `Reassembly` moves into a fresh `Zeroizing<Vec>` and drops the old one; the ceiling is checked before anything is reserved, so the size is ours and not the peer's |
| parity §2.2 | macOS Keychain reachable only behind a cargo feature no build passed | The feature is deleted. `security-framework` is target-gated and the backend is chosen by `target_os` alone (`core/src/crypto/keystore/`) |
| ADR-0003 §"Also outstanding" | No Android Keystore backend | `core/src/crypto/keystore/android.rs` — an AES-GCM key that never leaves the Keystore wraps the secret; the blob sits in app-private storage. Never compiled (§6) |
| parity §2.6 · B-7 | Forged cloud metadata could outrank a real item | `cloud/src/crypto/sign.rs`. Every row carries an HMAC over the ordering fields plus ciphertext and nonce; `pull` verifies before the merge and refuses what does not |
| parity §2.6 (§5.4 obligation 3) | The cloud threat-model change was unrecorded | `docs/cloud-privacy.md` |
| parity §2.9 · ui-parity §2.6 | Localisation | `ui/src/i18n/`, one catalogue behind i18next with `catalogue.test.ts` over it |
| parity 18 (half) | No sound on copy | `daemon/src/notify.rs`, called from the capture tick; macOS only, gated on `sound_on_copy`, suppressed on any fake clipboard backend. The notification half is B-18 |
| ui-parity 3 (half) · B-15 | Origin device nowhere on the wire | `Item::{origin_device_id, origin_device_name}` and `UiItem`'s pair. The frontend has not consumed them — B-15 |
| ui-parity 9 (half) · B-26 | No "too large to sync" signal | `Item::too_large_to_sync` and `UiItem`'s copy, plus `CloudSyncData::skipped_too_large`. Not consumed by any component — B-26 |
| ADR-0004 §"still missing" | No `Method::Shutdown` | `ipc/src/lib.rs`, `daemon/src/server/dispatch.rs`, CLI `shutdown` |
| parity 19 (below the UI) · B-23 | No `reorder_pinned` anywhere | `Store::reorder_pinned`, `Method::ReorderPinned`, CLI `reorder`, `useReorderPinned`. The drag affordance and the two Tauri backends are B-23 |
| parity 9 (UI half) · ui-parity §2.4 · B-14 | Daemon settings had no UI | `components/settings/ServiceTab.tsx` over `hooks/useServiceConfig.ts`: poll interval, history limit, retention days, max item bytes, sensitive TTL, notify, sound, sync, LAN visibility. `e2e/tests/daemon-config.e2e.test.ts` drives it |
| B-22 | `README.md` **Missing**, `SECURITY.md` **Not implemented**, both stale | Both prose inventories are deleted rather than corrected; each now names this file. `claims-audit.md`, `security-review.md`, `parity-audit.md` and `ui-parity-audit.md` mark their stale rows closed in place |

## 2. In flight

Everything the 2026-07-30 build of this section listed has landed and moved to
§1. Check `git status` before starting anything — this section is only ever true
for the minutes after it is written, which is why it holds no work of its own.

## 3. The backlog, ranked by user consequence

Everything below is open. Line numbers rot faster than paths — where one
disagrees with the tree, trust the identifier.

### Tier 1 — data loss

| # | Item | What the user loses | Evidence | Depends on |
|---|---|---|---|---|
| **B-1** | **Keyset pagination is in `copypaste-core` and on no wire.** `Method::List` is still `{ limit, offset }`. | Page 2 repeats or skips rows whenever a row lands above the window — `CopyPaste-8ebg.57`, parity D6. **This became live today**: it was masked while the app fetched one page, and load-more has since shipped. | `Store::list_from` at `core/src/storage/page.rs:83`, callers: tests only. `Store::list` at `core/src/storage/items.rs:134-144` (`LIMIT ?1 OFFSET ?2`). `ipc/src/lib.rs:61-64`. Manifest 03 §3.12 + Q11 (which specifies the test). | Nothing. Core half is done and tested. |
| **B-2** | **Peer-supplied `content_hash` is stored and ordered on, unverified.** | A paired-but-hostile or simply buggy peer picks a hash colliding with a local row, `idx_items_dedup` refuses the insert, and a chosen item silently never lands. It also controls merge key 2, and the receiver re-advertises the forged hash onward. | `core/src/sync/merge.rs:136` — `Some(hash) => hash` still passes through; the `None` arm (cloud) recomputes. The merge moved out of `daemon/src/merge.rs` into `copypaste-core` and the gap moved with it. Security review F-3. | Nothing. Fix is in the `!deleted` arm of `apply_remote_version`. |
| **B-4** | **The v0.4 detector is written and never asked.** | `core/src/storage/legacy.rs` identifies a v0.4 schema correctly and `StoreError::LegacyDatabase` carries the sentence — and **the check cannot fire in a shipping build.** `is_v1_database` is reached only from `Store::open` and `open_validated`; `Store::open` is handed `copypaste-v2.db` and nothing else, `open_validated` has no non-test caller, and `v1_database_in(dir)` — the function shaped for the real question, "is an old history sitting beside the new one?" — has zero callers anywhere. So the daemon cannot *produce* the error, and a screen wired to it would be a state nothing can enter. On upgrade the user gets a fresh empty history and silence. CLAUDE.md rule 3's one obligation is not discharged. | `core/src/storage/legacy.rs`, `storage/store.rs:43`, `storage/dbfile.rs:35`; `daemon/src/main.rs` (`db_path` is always `copypaste-v2.db`). `post-merge-review.md` §2 reaches this by call-site audit; ui-parity 2; manifest 06 §3.1.11 (binding, with exact copy). | Nothing. A `v1_database_in(&data_dir)` probe at startup, *then* the wire code and the screen. Doing the surface half alone closes nothing. |
| **B-5** | **Mutations made during a backend outage rely on the next full push round.** | A pin or delete made while Supabase is unreachable is not queued; nothing replays it. LWW bounds the blast radius, so this is not a guaranteed loss — but v1's `CopyPaste-1t38` test has no analogue. | No outbound queue in `cloud/src/sync/` or `daemon/src/cloud/`. Parity D8, AT-33. | Nothing. |

### Tier 2 — security

| # | Item | What the user loses | Evidence | Depends on |
|---|---|---|---|---|
| **B-6** | **Screen-capture protection is off (INV-35).** | Any screen recorder, and any app with screen-recording permission, captures the history window — including revealed secrets. The manifest says on by default. | `src-tauri/tauri.conf.json` → `app.windows[0]` has no `contentProtected`; no `set_content_protected` call in `src-tauri/src/`. Parity S11, ui-parity §2.4. | Nothing. One config key plus the macOS call. |
| **B-8** | **Device revocation is built and unreachable.** | `PeerStore::revoke` / `revoke_all` / `revoked` exist, are tested, and enforce on every read path — and there is no IPC verb, no CLI verb and no UI. `unpair` routes to `PeerStore::remove` and is local-only, so a lost or stolen device keeps syncing. Sync-key rotation does not exist at all. | `p2p/src/peers/store.rs:236,269,298`; `Method` has `Unpair` and nothing else. Parity 7; ui-parity §2.3. CLAUDE.md rule 6's exact shape. | Nothing for revoke (store half done). Rotation is a separate, larger piece. |
| **B-9** | **The clock-skew ceiling is enforced per transport, not at the shared merge.** | Both transports do check, so this is a latent hole rather than a live one: `apply_remote_version` has two callers and guards neither. The next caller inherits nothing. Separately, a peer stamping `now + 24 h − ε` wins every comparison for a day — the accepted trade in R-CLK-2, which `SECURITY.md` overstates. | `p2p/src/sync/plan.rs:17` and `cloud/src/sync/pull.rs:56` both hold `MAX_FUTURE_SKEW_MS`; `core/src/sync/merge.rs` holds none. Security review F-5(b). | Nothing. Move the ceiling to where the paths converge; keep the two early-outs. |
| **B-10** | **One bounded hazard left of the five.** | **(a)** TOCTOU between `bind()` and `chmod 0600` — under `umask 002` the socket is 0775 for the window, and the parent-dir `0700` is applied warn-only (F-9). (b) through (e) closed; see §1. | `daemon/src/server/listener.rs:90-91`. | Nothing. `umask(0o177)` around the bind, or bind to a temporary name inside the already-`0700` directory and rename it into place. |

### Tier 3 — a capability nobody can reach

CLAUDE.md rule 6: shipped code with no interface is not a shipped feature.

| # | Item | What the user loses | Evidence | Depends on |
|---|---|---|---|---|
| **B-11** | **`reset_database` and `vacuum` have no verb.** | The escape hatch B-4 needs, and the only way to recover from an unopenable store without deleting a file by hand — which no error may name (rule 4). | `Method` has `Backup` / `Restore`, not these. `daemon/src/server/dbadmin.rs`. Parity 6 (residue). | Nothing. |
| **B-12** | **No log-read verb, so no Logs tab.** | A user reporting a bug has nothing to attach, and no path is safe to tell them to open by hand. | ui-parity 6; manifest 06 §3.4.10. v1 redacted the directory to `~` (`CopyPaste-2b3i`). | A verb, then the tab. The largest of the twelve UI gaps. |
| **B-13** | **The Android backend refuses seven operations, and they are no longer the same seven.** `add`, `pair_create`, `pair_accept`, `sync`, `discovered` and `rescan` all closed once `copypaste-core`'s ingest and `copypaste-p2p`'s node landed and `backend/embedded/` consumed them. What refuses now: `set_config` (so the Service tab is desktop-only), `export` / `import`, `backup` / `restore`, `watch`, and `reorder_pinned` (B-23). | The Android build can read, copy, add, pin, delete and clear history, pair, sync and discover. It cannot change a setting, get data out, or receive a push. | `src-tauri/src/backend/embedded/mod.rs` — `MSG_NO_SETTINGS`, `MSG_NO_TRANSFER`, `MSG_NO_BACKUP`, `MSG_NO_WATCH`, `MSG_NO_REORDER`. ADR-0003. | Nothing. `get_config` is already implemented there; only the write half refuses. |
| **B-15** | **Origin device reaches the WebView boundary and stops.** | With sync on, every row still looks local: no badge, no device filter, no by-device sort. | `UiItem` carries `origin_device_id` and `origin_device_name` (`src-tauri/src/model.rs`), fed by `Item` on the wire. `ui/src/lib/ipc.ts`'s `Item` still declares six fields and no component reads either. ui-parity 3. | Nothing. Both halves below the TS type are done. |
| **B-16** | **No per-device sync health, and no sync dimension anywhere.** | The peer row shows *Last seen* (a discovery signal), never *last synced*; `StatusChip` is entirely about the local service. Manifest 06 §3.8 names the exact failure: "the badge can read `synced` while one peer silently receives nothing". | `PeerInfo::last_sync_ms` exists (`payload.rs:110`) and no component reads it. `components/devices/DevicesView.tsx`, `components/shell/StatusChip.tsx`. ui-parity 4. | Nothing — the field landed. A stall pill needs a failure counter too. |
| **B-16a** | **A cloud round counts forged rows and cannot report them.** | `SyncStats::skipped_forged` is the number that says something wrote a row into the account that does not hold the passphrase. It reaches no client: `CloudSyncData` carries six skip counters and not this one, and `CloudStatusData` carries none. Today it exists only in the daemon's log, so the one signal that distinguishes an attack from a quiet day is unreachable. | `cloud/src/sync/outcome.rs:32`; `ipc/src/payload.rs` `CloudSyncData` / `CloudStatusData`. `docs/cloud-privacy.md` states the gap. | Nothing. One field on `CloudSyncData`, then a readout. |
| **B-17** | **No way to see an item's full content.** | Rows clamp to 1–6 preview lines with no expand, no tooltip carrying the text and no detail view. Choosing between three similar long clips is guesswork. **The content is already at the frontend** — this is a missing view, not missing data. | `components/history/HistoryRow.tsx` (`WebkitLineClamp`); `src-tauri/src/model.rs`. ui-parity 1. Harder than it looks: the window is now one 420-wide popover (ui-parity §2.1). | Nothing. |
| **B-18** | **No notification on copy.** | The sound landed and its switch is reachable (`daemon/src/notify.rs`; `ServiceTab.tsx`). The notification did not, and it cannot come from the daemon: `UNUserNotificationCenter` needs an application bundle, so the app has to post it off `EventData::captured` and `notify_on_copy`. Both are wired *to* the toggle and read by nothing — a user can switch on a notification that will never arrive. | `daemon/src/notify.rs`; `EventData::captured` is never set `true`; no notification plugin in `src-tauri`. Parity 18; manifest 06 §3.6, manifest 01 §3.23. | Nothing. |
| **B-19** | **No `Recent` submenu in the menu-bar item.** | While the window is hidden the tray is the whole app, and v2's is three navigation verbs with nothing you can *do*. | `src-tauri/src/shell/tray.rs` (ids: `toggle`, `autostart`, `quit`). ui-parity 5; manifest 06 §3.6. | Nothing. |

### Tier 4 — polish, hygiene, and the record

| # | Item | Evidence |
|---|---|---|
| **B-20** | Escape does not dismiss the popover. It clears selection in the list and the query in the search field; nothing at the top level hides the window. For a hotkey-summoned popover, Escape *is* the dismissal. | `HistoryList.tsx:229`, `SearchBar.tsx:97`, `shell/window.rs` (handles `CloseRequested` and `Focused(false)`, not a key). ui-parity 7 |
| **B-21** | About reports the *service* version and no external links — no app version, changelog, or privacy policy. | `components/settings/AboutTab.tsx`; no `@tauri-apps/api/app` import. ui-parity 8 |
| **B-22a** | **A document can be added and the index will not notice.** `docs/README.md` claims to list every ADR, audit and study, and `README.md` tells the reader to start there. It has now fallen behind twice: first ADR-0004 and `ui-parity-audit.md`, then ADR-0005, ADR-0006, `claims-audit.md` and `android-spike.md`. Nothing fails when it happens. | `docs/README.md`; `README.md` §Decisions. Claims audit finding 13. A `ls docs/adr docs/rewrite` diffed against the index, run in CI, would end it |
| **B-23** | Drag-to-reorder pins. Every layer below the screen landed — `Store::reorder_pinned` (`core/src/storage/pinning.rs`), `Method::ReorderPinned`, the daemon's dispatch, CLI `reorder`, `useReorderPinned`. There is no drag affordance, and **both Tauri backends still refuse on stated reasons that are false**: `backend/daemon.rs` says "`copypaste_ipc::Method` has no reorder, so there is nothing to send"; `backend/embedded/mod.rs` says the transaction "belongs beside the other `pin_order` writes in `copypaste-core`" as though it were not already there. Two comments, one wrong fact each, and a capability the user cannot reach through any surface but the CLI. | `components/history/HistoryList.tsx`; `src-tauri/src/backend/`; parity 19 residue. Claims audit finding 10 |
| **B-24** | Bulk **Copy** and a visible Select-all. ⌘A works but has no on-screen control, in the exact mode where you want it. | `components/history/BulkBar.tsx` (being edited). ui-parity 11 |
| **B-25** | Launch at login is a tray check item only, read once at build and never re-read, so it shows the wrong tick after a change in System Settings. Not in Settings at all. | `shell/tray.rs:36-47`, and its own comment. ui-parity 10 |
| **B-26** | No "too large to sync" affordance, though the data is at the boundary: `UiItem::too_large_to_sync` and `CloudSyncData::skipped_too_large` both landed and neither is declared in `ui/src/lib/ipc.ts` or drawn by `HistoryRow.tsx`. An item that will silently never reach the other device still looks like one that will. | `src-tauri/src/model.rs`; `ui/src/lib/ipc.ts`; `HistoryRow.tsx`. ui-parity 9 |
| **B-27** | ⌘1–⌘9 row numerals — advertised once in the footer, not carried per row, so the user counts. Partly substituted by `QuickHint`. | ui-parity 12 |
| **B-28** | No fuzz targets and no benchmarks. Three parse boundaries (AEAD, IPC line, sync frames) are unfuzzed; manifest 06 §5.4 carries budget numbers nothing measures. Good hand-written hostile-input tests cover much of the first. | No `fuzz/`, no `benches/`. Parity §2.9 |
| **B-29** | Untested behaviour that exists. Narrowed by `e2e/` (74 tests in a real WebKitGTK WebView): `scroll-anchor`, `push`, `service-lifecycle`, `sensitive`, `error-strings` and `history-render` now cover INV-1/INV-6, the push path and the a11y-adjacent render rules on Linux. Still unexercised anywhere: reveal auto-hide (INV-11), window-hide-on-close (INV-36), and everything in the macOS binding — a coverage boundary, not scattered neglect. | Parity §3.5; `e2e/tests/` |
| **B-30** | A11Y-10 contrast, A11Y-11 reduced motion, A11Y-12 reduced transparency, A11Y-15 720×460 minimum are behaviour, so the "visual is reference only" carve-out does not cover them. Contrast is now gated in `design/`; the other three are not asserted. | Parity §2.7 |
| **B-31** | ADR-0001's untested self-signed re-sign path. Signing in `postflight` with a per-machine self-signed certificate would give a stable designated requirement, which would make TCC grants survive updates and buy back auto-paste for nothing. Unresolved: whether TCC needs a trusted chain or only a valid signature matching the stored `csreq`. **Ten minutes on a Mac settles it** (§6). | ADR-0001 §"An untested third path" |
| **B-32** | ADR-0001 also asks to **watch** Homebrew's `postflight_steps` mini-DSL: arbitrary Ruby in a cask is now the discouraged form of something with a declarative replacement, which is usually how a feature ends. Not a task; a standing watch item. | ADR-0001 §"Why this path may still close" |

## 4. What unblocks what

Four clusters carry most of the list. Do the head of each and the tail gets
cheap; do the tail first and you pay twice.

| Unblocker | Unblocks | Note |
|---|---|---|
| **`backend/embedded/`'s write half** | B-13 — `set_config`, transfer, backup, `watch` | The read half already routes through the same core; six of its refusals closed this way |
| **`ui/src/lib/ipc.ts`'s `Item` catching up with `UiItem`** | B-15, B-16's device axis, and B-26 together: the row badge, the device filter, the by-device sort, the too-large warning | Three fields already crossing the bridge and declared by nothing on the far side |
| **`Store::list_from` reaching `Method::List`** | B-1, and it stops D6 becoming a live bug now that load-more ships | The hard half — a total order with an `id` tiebreak — is already done and documented |
| **A `BackendError` a screen can name** | B-4's sentence, and every future condition a retry cannot fix | The condition is already detected (`StoreError::LegacyDatabase`); nothing carries it across the bridge |

One smaller pair: B-4 wants B-11 for its escape hatch, but its *sentence* needs
only a route through the bridge — the detection is done.

## 5. Decisions, not debts — do not "fix" these

Each is written down where the code lives. Reversing one is a decision to take
deliberately, in a commit that amends the manifest in the same change
(CLAUDE.md rule 2). None is a bug.

| Decision | Recorded | If you reverse it |
|---|---|---|
| **Pin state does not travel over the cloud transport**, and the merge **refuses a remote delete of a pinned row**. The two stand or fall together: without the refusal, a device that cannot see the pin would delete a row the user pinned. | Manifest 05 §3.6, amended 2026-07-30, contradicting its own binding table; `core/src/sync/merge.rs` (`refuses_delete`); `daemon/src/meta/write.rs`. This also closes security review F-4. | Three fields, a column pair, a migration, a `CloudSource` that reads and writes them, and revisiting the refusal — all in one change. Neither half may move alone |
| **The purge pass removes from the search index and never rewrites `is_sensitive`.** | `core/src/sensitive/purge.rs`; security review F-12 | `sweep_sensitive` selects on that flag, so writing it would hand a changed ruleset a hard delete over data the user never reviewed. The cost to accept, stated: a row the current ruleset would flag stays listable and stays syncable — only its plaintext leaves the index |
| **No `expires_at` column**; the auto-wipe deadline is *derived* (`created_at + ttl`). | Manifest 07 §6.2 amended; `core/src/sensitive/wipe.rs:18-31`. v1 paid three bugs for the stored form (`3e7y`, `44rq.62`, `8ebg.2`) | Stated cost, not hidden: changing the TTL re-dates every existing sensitive item, not only new ones |
| **The wipe decision is re-derived from the plaintext** rather than read from a stamped flag. | Manifest 07 §6.2 amendment; `wipe.rs` "two gates, and both must agree" | A persisted "may be deleted" bit written by a ruleset that has since changed is a deletion nobody can review before it fires |
| **The cloud poll cadence is a backoff ladder, not two fixed intervals** — 5 s, doubling while quiet, snapping back on any change, ceiling 300 s rather than 60 s for phone battery. | Manifest 05 §4.8, amended 2026-07-30, in the first row only | Defensible only because `RealtimeEvent::Resubscribed` forces a round on reconnect, which v1 had no equivalent of |
| **Lamport clocks replaced by a four-key comparator plus a future-stamp ceiling.** | port-manifest README; manifest 05 §7.2/§7.3 | v1 had four Lamport implementations and a `LamportClock` with no caller |
| **mTLS + rcgen + two hand-written DER parsers replaced by Noise `NNpsk0`**; the SAS step is redundant because holding the token *is* the authentication. | Manifest 02 §6.3; ui-parity §2.3 | The *UI* for SAS is correctly gone, not missing |
| **No span/redaction API, no password-manager bundle list, no telemetry scrubber.** | `core/src/sensitive/mod.rs:33-36`; manifest 07 §7.4/§7.5 | An unused API recreates §7.4's three dead entry points |
| **Everything the v1 byte formats bought** — migration ladder, `key_version` dispatch, rotation sweep, bespoke chunk framing, the 60-second dedup "minute". | CLAUDE.md rule 3; port-manifest README's reference column | Retrofitting a migration path is materially harder once the v1 formats leave the tree |
| **`--options runtime` / hardened runtime and an entitlements file are not wanted.** | ADR-0001 §"Two things this ADR did not say" | It is the natural thing to copy from any macOS signing guide, and v1 did copy it |
| **`AccessibilityService` is not an Android clipboard exemption**, despite a decade of folklore and v1's own UI strings. | `android-clipboard-access.md` §1, from `ClipboardService.clipboardAccessAllowed` in AOSP | It would cost a scary permission and a Play declaration, and it would not work |
| **v1's `READ_LOGS`/logcat approach is not ported**; **we do not ship an IME.** | Same, §1 and §3 | Rung 4 is structurally dead on Android 13+; rung 3 makes this a keyboard product |

## 6. Not verifiable on this host — waiting on hardware, not on work

Nothing macOS or Android has ever executed here. There is no Android SDK, no
NDK, no device, no camera and no Mac; no Supabase project has ever been
contacted. **These are not debts.** Each is written, compiled and reviewed, and
each needs one thing this container does not have.

| Needs | What is unproven | Where it says so |
|---|---|---|
| **A Mac** | `NSPasteboard` capture, the Keychain device-secret store, tray placement and the popover landing under the icon, the global hotkey, launch-at-login, whether `Contents/MacOS/copypaste-daemon` starts from inside a quarantined ad-hoc-signed bundle, whether `codesign` / `hdiutil` / the Tauri bundler work, whether the cask's `xattr` + re-seal is sufficient, and **B-31** — whether TCC accepts a self-signed certificate | ADR-0003 §"Verification status"; ADR-0004 §"Consequences"; ADR-0001 §"Verified, 2026-07-30"; `README.md` **Unverified** |
| **An Android device + SDK** | The whole ladder. The central claim — that a binder call as the shell uid reads the clipboard with no focus — is derived from AOSP source and has never been observed. Also: whether the listener registers and fires as shell, whether `mAppOps.checkPackage(2000, "com.android.shell")` passes through Shizuku, what the Android 12+ toast looks like in practice, and OEM variance in on-device wireless-debugging pairing | `android-clipboard-access.md` §4 (marked **blocking**) and Open questions; `src-tauri/src/capture/mod.rs` §Unverified |
| **A real Supabase project** | Argon2id parameters and the row AEAD against a live backend; the RLS policies and the `pg_cron` retention job have never been applied to a deployment; AT-51's static policy audit. The demo drives a **local stub** (`scripts/cloud-stub.py`) | `README.md` **Unverified**; security review §"Suspected, not confirmed" |
| **Multicast** | mDNS discovery at runtime. `Discovery::start` degrades to an empty peer list and an explicit address always pairs, so this is a convenience, not a dependency | `p2p/src/node/mod.rs`; `README.md` |
| **A shipping WebView** | The `e2e/` suite runs WebKitGTK 2.52 under Xvfb. That is real JavaScript and real layout — but it is neither WKWebView nor Android's WebView, so a green run is evidence about Linux | `e2e/README.md` |

**Two debts that used to sit here have moved out of it**, and neither was ever a
hardware question. The Android Keystore backend exists
(`core/src/crypto/keystore/android.rs`), so the `0600` file store is no longer
what would ship on Android. And the embedded backend now takes its data
directory from Tauri's `app.path().app_data_dir()` rather than `directories`,
which has no notion of an Android context. Both are unrun, which is what §6 is
for; neither is outstanding work.

The same distinction applies to the macOS Keychain. What needed a Mac was
running it. What needed nobody was noticing that a cargo feature guarded it and
no release script passed it — this section filed that under hardware for a day,
and it was a reading of `build-macos-app.sh` away.

## 7. What this register does not establish

Almost nothing was executed. Every verdict above is a source reading plus a
`grep` against a working tree several agents are editing, and items have moved
mid-sweep more than once. The two exceptions, both run on 2026-07-30: `design`'s
`npm run check` (828 contrast pairs clear, usage gate clear) and a count of
`e2e/tests` (74). Re-check `git status` before starting anything. The audits
this draws on state their own limits: `parity-audit.md` §7,
`ui-parity-audit.md` §6, `security-review.md` §"Suspected, not confirmed",
`claims-audit.md` §0.
