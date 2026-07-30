# Backlog — everything outstanding, in one place

**Built:** 2026-07-30, by sweeping `parity-audit.md` (19), `ui-parity-audit.md`
(12), `security-review.md` (14), `android-clipboard-access.md`, all four ADRs,
the manifest amendments, and the tree itself. **Every item was re-checked
against the working tree**, not copied from its source document — the audits
anchor at older commits and roughly half of what they list has since closed.

CLAUDE.md rule 9 requires unfinished work to live in an issue rather than a
commit message, and there is no issue tracker. This file is that register.

**How to read it.** §1 is what already closed — read it before starting
anything, because the largest single cost here has been items being closed
twice. §2 is being written *right now* by other agents; do not start these.
§3 is the backlog, ranked by what a user loses: data loss, then security, then
a capability nobody can reach, then polish. Cost is a tiebreak, never the sort
key. §4 groups by what unblocks what — the ordering in §3 falls out of it.
§5 is decisions that look like debts; changing one is a reversal, not a fix.
§6 is work waiting on hardware or an account, which is not debt.

---

## 1. Closed — do not re-open

Verified in the tree today. The source document still lists each as missing.

| Source | Item | Closed by |
|---|---|---|
| parity 1 | Pairing / peers / sync UI | `crates/copypaste-ui/src/components/devices/` (5 files, incl. `QrCode.tsx`, `QrScanner.tsx`) |
| parity 2 | Daemon lifecycle ownership | ADR-0004; `src-tauri/src/service/`; `components/shell/ServiceOffline.tsx` |
| parity 3 | Sensitive-item auto-wipe (TTL) | `copypaste-core/src/sensitive/wipe.rs`; swept from the poll loop, `daemon/src/capture.rs:106` |
| parity 4 | Re-copy bumps to the top | `Store::insert_or_bump`, `core/src/storage/items.rs:122`; dedup is unbounded again (`retention.rs:16`) |
| parity 5 | Export / import | `Method::Export` / `Import`; `ipc/src/payload.rs` `ExportData` / `ExportItem` |
| parity 6 | Database backup / restore | `Method::Backup` / `Restore`; `daemon/src/server/dbadmin.rs`. **Partial** — `reset_database` and `vacuum` are still absent (B-11) |
| parity 9 | Daemon config | `ipc/src/config.rs`, `daemon/src/settings.rs`, `Method::GetConfig` / `SetConfig`. **Wire only** — no UI reads it (B-14) |
| parity 11 | Quick-Paste popup, global hotkey | `src-tauri/src/shell/{hotkey,window}.rs` |
| parity 12 · D1 | Stale-socket bind TOCTOU | `BindLock`, an exclusive `flock(2)` over probe→remove→bind, `daemon/src/server/listener.rs:98-122` |
| parity 13 · S8 | IPC connection cap, read/write timeouts | `listener.rs:62-68` (`READ_TIMEOUT` 30 s, `WRITE_TIMEOUT` 10 s) + `Semaphore` |
| parity 14 · F-6 · S7 | Pairing codes never expire | `PAIRING_CODE_TTL` = 300 s, `p2p/src/peers/mod.rs`; refused on every read path, not one gate |
| parity 15 | Push / streaming updates | `Method::Watch`; `ui/src/hooks/usePush.ts` |
| parity 16 | Discovery not reachable | `Method::Discovered` / `Rescan` |
| parity 17 · F-10 | Undecryptable rows not counted | `ItemPage::skipped_undecryptable`; `components/history/SkippedNotice.tsx` |
| parity 19 | Bulk actions, filter, sort | `components/history/BulkBar.tsx`, `lib/view.ts`. Drag-to-reorder is in flight (§2) |
| ADR-0003 | `reveal_item` unsupported on desktop | `Method::Get { id }` exists; only `reorder_pinned` still refuses in `backend/daemon.rs:242` |
| ui-parity 4 (half) | No per-peer sync health on the wire | `PeerInfo::last_sync_ms`, `ipc/src/payload.rs:110`. UI half still open (B-16) |
| sec F-1 | Quoted credential values invisible to the detector | `sensitive/validators.rs` — `unquote()` strips one balanced pair before the code-shape gate |
| sec F-2 | `aws_secret_access_key = …` matched no rule | `sensitive/rules.rs:355`, its own rule at 0.99 |
| sec F-4 | Remote tombstone cleared `pinned` | **Decided, not fixed** — see §5 |
| sec F-7 | `SECURITY.md`'s mDNS claim was false | `SECURITY.md:105` now says "a domain-separated BLAKE2s of the token" |

Two documents are now stale enough to mislead: `parity-audit.md` §2.7 (UI) and
`README.md`'s **Missing** section, which still lists export/import,
backup/restore, config, streaming, discovery and bulk actions as absent. B-22.

## 2. In flight — uncommitted in the working tree right now

Nine agents hold the tree. Do not start any of these; re-check `git status`
before starting anything that touches the same files.

| Work | Evidence | Unblocks |
|---|---|---|
| `capture::ingest` lifted into `copypaste-core` | `core/src/ingest.rs` (untracked), `core/src/lib.rs` gains `pub mod ingest` | B-13a |
| The p2p **node** lifted into `copypaste-p2p` | `p2p/src/node/{mod,dial,listen,error,channel}.rs` (untracked); `daemon/src/p2p/channel.rs` moved | B-13b |
| Origin device on the wire | `ipc/src/payload.rs:202-226` (`origin_device_id`, `origin_device_name`), `daemon/src/meta/devices.rs` | B-15, B-16 |
| Signed LWW cloud metadata | `cloud/src/crypto/sign.rs` (untracked) | B-7 |
| Drag-to-reorder pins | `core/src/storage/pinning.rs`, `Method::ReorderPinned` | B-23 |
| `Method::Shutdown` (ADR-0004's request) | `ipc/src/lib.rs:252`, `daemon/src/server/dispatch.rs:238` | ADR-0004 §"still missing" |
| Localisation | `ui/src/i18n/` (untracked, 8 files) | parity §2.9 |
| Android capture ladder, rungs 0 and 2 | `src-tauri/src/capture/{mod,model,intake,desktop}.rs` (untracked) | §6 |
| One content-type vocabulary | `ipc/src/content_type.rs` (untracked) | — |
| Keyed-connection consolidation, own-address enumeration | `core/src/storage/dbfile.rs`, `p2p/src/netif.rs` | — |
| `BulkBar.tsx`, `QuickHint.tsx`, `SearchBar.tsx` edits | modified | B-24, B-27 |

`src-tauri/src/capture/mod.rs` cites `ADR-0005` and
`docs/rewrite/android-spike.md`; neither file exists yet. Expect them from the
same agent.

## 3. The backlog, ranked by user consequence

Status: **open** · **open (in flight nearby)** — an adjacent file is being
edited, re-check first.

### Tier 1 — data loss

| # | Item | What the user loses | Evidence | Depends on |
|---|---|---|---|---|
| **B-1** | **Keyset pagination is in `copypaste-core` and on no wire.** `Method::List` is still `{ limit, offset }`. | Page 2 repeats or skips rows whenever a row lands above the window — `CopyPaste-8ebg.57`, parity D6. **This became live today**: it was masked while the app fetched one page, and load-more has since shipped. | `Store::list_from` at `core/src/storage/page.rs:83`, callers: tests only. `Store::list` at `core/src/storage/items.rs:134-144` (`LIMIT ?1 OFFSET ?2`). `ipc/src/lib.rs:61-64`. Manifest 03 §3.12 + Q11 (which specifies the test). | Nothing. Core half is done and tested. |
| **B-2** | **Peer-supplied `content_hash` is stored and ordered on, unverified.** | A paired-but-hostile or simply buggy peer picks a hash colliding with a local row, `idx_items_dedup` refuses the insert, and a chosen item silently never lands. It also controls merge key 2, and the receiver re-advertises the forged hash onward. | `daemon/src/merge.rs:132-138` — `Some(hash) => hash` passes through; the `None` arm (cloud) recomputes. Security review F-3. | Nothing. Fix is in the `!deleted` arm of `apply_remote_version`. |
| **B-3** | **`is_sensitive` is decided once, at capture, and never revisited; the "purge pass" three texts promise does not exist.** | F-1 and F-2 were fixed *today*. Every item captured before that fix stays unflagged: its plaintext stays in `clipboard_fts` — the one table not under the item AEAD — and keeps syncing. There is no mechanism to correct it. | `ipc/src/payload.rs:199` and `daemon/src/server/items.rs:349` both promise a purge pass; `grep` finds no rescan, reindex or purge anywhere. CLAUDE.md rule 4 makes the same claim. Security review F-12. | Nothing. Either build the rescan or delete the sentence from all three places and record the limitation. |
| **B-4** | **Nothing detects a v1 database, and no screen could say so.** | CLAUDE.md rule 3's *one* obligation is half-met: `copypaste-v2.db` means a v1 file is never touched (tested), but "a v2 build that stumbles onto it must say so plainly rather than failing with a decryption error that reads like corruption" has no code and nowhere to appear. Today the user gets `Failed to load history` and a **Try again** button that retries forever against a condition retrying cannot fix. | `ipc/src/lib.rs:408`; `components/history/HistoryView.tsx` state chain; no `degraded` / `reset_database` anywhere. ui-parity 2; manifest 06 §3.1.11 (binding, with exact copy). | B-11 (`reset_database` on the wire) for the escape hatch; the *detection and the sentence* need neither. |
| **B-5** | **Mutations made during a backend outage rely on the next full push round.** | A pin or delete made while Supabase is unreachable is not queued; nothing replays it. LWW bounds the blast radius, so this is not a guaranteed loss — but v1's `CopyPaste-1t38` test has no analogue. | No outbound queue in `cloud/src/sync/` or `daemon/src/cloud/`. Parity D8, AT-33. | Nothing. |

### Tier 2 — security

| # | Item | What the user loses | Evidence | Depends on |
|---|---|---|---|---|
| **B-6** | **Screen-capture protection is off (INV-35).** | Any screen recorder, and any app with screen-recording permission, captures the history window — including revealed secrets. The manifest says on by default. | `src-tauri/tauri.conf.json` → `app.windows[0]` has no `contentProtected`; no `set_content_protected` call in `src-tauri/src/`. Parity S11, ui-parity §2.4. | Nothing. One config key plus the macOS call. |
| **B-7** | **Forged cloud metadata can outrank and censor a real item.** | Someone with the account password but not the sync passphrase cannot read anything, but can stamp a competing version of a known `item_id` that wins the merge on every device. | Manifest 05 §5.3; parity S12; `SECURITY.md` does not record it. **In flight** — `cloud/src/crypto/sign.rs`. | §2. Also owes the threat-model paragraph (manifest 05 §5.4 obligation 3). |
| **B-8** | **Device revocation is built and unreachable.** | `PeerStore::revoke` / `revoke_all` / `revoked` exist, are tested, and enforce on every read path — and there is no IPC verb, no CLI verb and no UI. `unpair` is local-only, so a lost or stolen device keeps syncing. Sync-key rotation does not exist at all. | `p2p/src/peers/store.rs:227,260,289`; `Method` has `Unpair` and nothing else. Parity 7; ui-parity §2.3. CLAUDE.md rule 6's exact shape. | Nothing for revoke (store half done). Rotation is a separate, larger piece. |
| **B-9** | **The clock-skew ceiling is enforced per transport, not at the shared merge.** | Both transports do check, so this is a latent hole rather than a live one: `apply_remote_version` now has two callers and guards neither. The next caller inherits nothing. Separately, a peer stamping `now + 24 h − ε` wins every comparison for a day — the accepted trade in R-CLK-2, which `SECURITY.md` overstates. | `p2p/src/sync/plan.rs:17` and `cloud/src/sync/pull.rs:56` both hold `MAX_FUTURE_SKEW_MS`; `daemon/src/merge.rs` holds none. Security review F-5(b). | Nothing. Move the ceiling to where the paths converge; keep the two early-outs. |
| **B-10** | **Five bounded hazards in the transport and the socket.** | Each is small on its own; grouped because they are one afternoon. **(a)** TOCTOU between `bind()` and `chmod 0600` — under `umask 002` the socket is 0775 for the window, and the parent-dir `0700` is warn-only (F-9). **(b)** PSKs copied unzeroized into freed heap on every *unauthenticated* inbound connection (F-8). **(c)** No cap on stored pairings, so `accept_any` does one X25519 per pairing per anonymous connect (F-13). **(d)** Reassembly `Vec` grows inside `Zeroizing`, leaving peer plaintext in freed heap (F-14). **(e)** `--data-dir` does not relocate the device secret, so a "fully isolated" demo daemon reads and creates the real user's key (F-11). | (a) `daemon/src/server/listener.rs:89-92`. (b) `daemon/src/p2p/mod.rs:247` — and the in-flight lift carries it forward at `p2p/src/node/listen.rs:88`, so fix `psks()` itself. (c) no `max_pairings` in `peers/store.rs`. (d) `p2p/src/transport/session.rs:150,196`. (e) `core/src/crypto/keystore.rs:103-107`, and its `ProjectDirs` qualifier differs in case from `ipc::data_dir`. | (b) is cheaper *after* §2's node lift lands. |

### Tier 3 — a capability nobody can reach

CLAUDE.md rule 6: shipped code with no interface is not a shipped feature.

| # | Item | What the user loses | Evidence | Depends on |
|---|---|---|---|---|
| **B-11** | **`reset_database` and `vacuum` have no verb.** | The escape hatch B-4 needs, and the only way to recover from an unopenable store without deleting a file by hand — which no error may name (rule 4). | `Method` has `Backup` / `Restore`, not these. `daemon/src/server/dbadmin.rs`. Parity 6 (residue). | Nothing. |
| **B-12** | **No log-read verb, so no Logs tab.** | A user reporting a bug has nothing to attach, and no path is safe to tell them to open by hand. | ui-parity 6; manifest 06 §3.4.10. v1 redacted the directory to `~` (`CopyPaste-2b3i`). | A verb, then the tab. The largest of the twelve UI gaps. |
| **B-13** | **The Android backend refuses eight operations.** Two structural causes, both being removed in §2. **(a)** `add` needs the ingest pipeline. **(b)** `pair_create`, `pair_accept`, `sync`, `discovered`, `rescan` need a running node. **(c)** `watch` needs an in-process event source. **(d)** `reorder_pinned` needs the storage + wire half. | The Android build "can read, copy, pin, delete and clear history, and list and forget peers — but it cannot add an item or sync, which means it is not shippable" (ADR-0003). | `src-tauri/src/backend/embedded.rs:289` (a), `:386,390,424,433,437` (b), `:448` (c), `:361` (d). ADR-0003 §"the fix, in crates this change does not own". | (a) `core/src/ingest.rs`; (b) `p2p/src/node/`; both §2. **This file is not currently being edited** — the lifts land in other crates and someone has to come back and consume them. |
| **B-14** | **Daemon settings have no UI.** | `GetConfig` / `SetConfig` are on the wire and `daemon/src/settings.rs` persists a validated record — and every Settings tab reads local `prefs` only. Poll interval, size caps, storage quota, sensitive TTL and the sync toggles are unreachable from the product. | `ipc/src/lib.rs:209,212`; `daemon/src/settings.rs`; `grep setConfig ui/src` → nothing. Rule 6 again. | Nothing. |
| **B-15** | **Origin device is invisible in the app.** | With sync on, every row looks local: no badge, no device filter, no by-device sort. | `ui/src/lib/ipc.ts` `Item` has six fields. The wire half is in flight (§2). ui-parity 3. | §2 (the wire field). |
| **B-16** | **No per-device sync health, and no sync dimension anywhere.** | The peer row shows *Last seen* (a discovery signal), never *last synced*; `StatusChip` is entirely about the local service. Manifest 06 §3.8 names the exact failure: "the badge can read `synced` while one peer silently receives nothing". | `PeerInfo::last_sync_ms` exists (`payload.rs:110`) and no component reads it. `components/devices/DevicesView.tsx`, `components/shell/StatusChip.tsx`. ui-parity 4. | Nothing — the field landed. A stall pill needs a failure counter too. |
| **B-17** | **No way to see an item's full content.** | Rows clamp to 1–6 preview lines with no expand, no tooltip carrying the text and no detail view. Choosing between three similar long clips is guesswork. **The content is already at the frontend** — this is a missing view, not missing data. | `components/history/HistoryRow.tsx` (`WebkitLineClamp`); `src-tauri/src/model.rs`. ui-parity 1. Harder than it looks: the window is now one 420-wide popover (ui-parity §2.1). | Nothing. |
| **B-18** | **No notifications and no sound on copy.** | A background capture is invisible. v1 gated both on config, which now exists (B-14). | No notification or sound code in the tree. Parity 18; manifest 06 §3.6, manifest 01 §3.23. | B-14 for the toggles. |
| **B-19** | **No `Recent` submenu in the menu-bar item.** | While the window is hidden the tray is the whole app, and v2's is three navigation verbs with nothing you can *do*. | `src-tauri/src/shell/tray.rs` (ids: `toggle`, `autostart`, `quit`). ui-parity 5; manifest 06 §3.6. | Nothing. |

### Tier 4 — polish, hygiene, and the record

| # | Item | Evidence |
|---|---|---|
| **B-20** | Escape does not dismiss the popover. It clears selection in the list and the query in the search field; nothing at the top level hides the window. For a hotkey-summoned popover, Escape *is* the dismissal. | `HistoryList.tsx:229`, `SearchBar.tsx:97`, `shell/window.rs` (handles `CloseRequested` and `Focused(false)`, not a key). ui-parity 7 |
| **B-21** | About reports the *service* version and no external links — no app version, changelog, or privacy policy. | `components/settings/AboutTab.tsx`; no `@tauri-apps/api/app` import. ui-parity 8 |
| **B-22** | `README.md` **Missing** and `parity-audit.md` §2.7 are stale (§1 above). `SECURITY.md`'s "Not implemented" has the same shape. | Both list six-plus capabilities that shipped |
| **B-23** | Drag-to-reorder pins: storage + wire in flight (§2); the UI half is not. | `components/history/HistoryList.tsx`; parity 19 residue |
| **B-24** | Bulk **Copy** and a visible Select-all. ⌘A works but has no on-screen control, in the exact mode where you want it. | `components/history/BulkBar.tsx` (being edited). ui-parity 11 |
| **B-25** | Launch at login is a tray check item only, read once at build and never re-read, so it shows the wrong tick after a change in System Settings. Not in Settings at all. | `shell/tray.rs:36-47`, and its own comment. ui-parity 10 |
| **B-26** | No "too large to sync" affordance. An item that will silently never reach the other device looks like one that will. | `ui/src/lib/ipc.ts` `Item`; `HistoryRow.tsx`. ui-parity 9 |
| **B-27** | ⌘1–⌘9 row numerals — advertised once in the footer, not carried per row, so the user counts. Partly substituted by `QuickHint`. | ui-parity 12 |
| **B-28** | No fuzz targets and no benchmarks. Three parse boundaries (AEAD, IPC line, sync frames) are unfuzzed; manifest 06 §5.4 carries budget numbers nothing measures. Good hand-written hostile-input tests cover much of the first. | No `fuzz/`, no `benches/`. Parity §2.9 |
| **B-29** | Untested behaviour that exists: `useScrollAnchor` (INV-1/INV-6, C10), the React Query delegation (INV-2/3/33, C11), reveal auto-hide (INV-11), `role="listitem"` (INV-8), the a11y announcer (INV-9), window-hide-on-close (INV-36). Every one is in the UI or the macOS binding — a coverage boundary, not scattered neglect. | Parity §3.5 |
| **B-30** | A11Y-10 contrast, A11Y-11 reduced motion, A11Y-12 reduced transparency, A11Y-15 720×460 minimum are behaviour, so the "visual is reference only" carve-out does not cover them. Contrast is now gated in `design/`; the other three are not asserted. | Parity §2.7 |
| **B-31** | ADR-0001's untested self-signed re-sign path. Signing in `postflight` with a per-machine self-signed certificate would give a stable designated requirement, which would make TCC grants survive updates and buy back auto-paste for nothing. Unresolved: whether TCC needs a trusted chain or only a valid signature matching the stored `csreq`. **Ten minutes on a Mac settles it** (§6). | ADR-0001 §"An untested third path" |
| **B-32** | ADR-0001 also asks to **watch** Homebrew's `postflight_steps` mini-DSL: arbitrary Ruby in a cask is now the discouraged form of something with a declarative replacement, which is usually how a feature ends. Not a task; a standing watch item. | ADR-0001 §"Why this path may still close" |

## 4. What unblocks what

Four clusters carry most of the list. Do the head of each and the tail gets
cheap; do the tail first and you pay twice.

| Unblocker | Unblocks | Note |
|---|---|---|
| **The two lifts** — `ingest` into `copypaste-core`, the node into `copypaste-p2p` | **6 of the 8** `Unsupported` returns in `backend/embedded.rs` (B-13a, B-13b) | Both are in flight in *other* crates. Nobody is currently editing `embedded.rs` — the consumption step is unowned and is the whole of Android shippability |
| **`Item.origin_device_id` / `origin_device_name` on the wire** | B-15 and B-16's device axis together: the row badge, the device filter, the by-device sort, the per-peer readout | One field pair, four surfaces. In flight |
| **`Store::list_from` reaching `Method::List`** | B-1, and it stops D6 becoming a live bug now that load-more ships | The hard half — a total order with an `id` tiebreak — is already done and documented |
| **A daemon config UI** (B-14) | B-18 (notify/sound toggles), the sensitive-TTL control, size caps, sync toggles | The wire and the persistence both exist |

Two smaller pairs: B-4 wants B-11 for its escape hatch (but not for its
detection or its sentence); B-10(b) is cheaper after the node lift lands than
before it.

## 5. Decisions, not debts — do not "fix" these

Each is written down where the code lives. Reversing one is a decision to take
deliberately, in a commit that amends the manifest in the same change
(CLAUDE.md rule 2). None is a bug.

| Decision | Recorded | If you reverse it |
|---|---|---|
| **Pin state does not travel over the cloud transport**, and the daemon **refuses a remote delete of a pinned row**. The two stand or fall together: without the refusal, a device that cannot see the pin would delete a row the user pinned. | Manifest 05 §3.6, amended 2026-07-30, contradicting its own binding table; `daemon/src/merge.rs:89-111,164`; `meta/write.rs:35-45`. This also closes security review F-4. | Three fields, a column pair, a migration, a `CloudSource` that reads and writes them, and revisiting the refusal — all in one change. Neither half may move alone |
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

Two things that follow. **An Android Keystore backend does not exist** — the
`0600` file store, whose own docs call it a development posture, is what would
ship on Android today (ADR-0003 §"Also outstanding"); that one *is* a debt,
and it is B-13's neighbour rather than a hardware item. And the embedded
backend's data directory resolves through `directories`, not the Android
context.

## 7. What this register does not establish

Nothing was executed. Every verdict above is a source reading plus a `grep`
against a working tree that nine agents are editing — several items moved
while it was being written. Re-check `git status` before starting anything in
§2 or anything marked *in flight nearby*. The audits it draws on state their
own limits: `parity-audit.md` §7, `ui-parity-audit.md` §6, `security-review.md`
§"Suspected, not confirmed".
