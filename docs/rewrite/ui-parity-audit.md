# UI parity audit — what a user could see or do in v1, surface by surface

**Question:** v1 shipped ~22k lines of TypeScript UI, v2 ~6.7k. Which of the
difference is *interface a user could see and use* that v2 does not have?

**Answer:** twelve. Most of v1's interface is present, and several screens are
better. The twelve below are visible behaviour that is gone with no decision
recorded anywhere I could find. Three of them are everyday interactions.

This is **not** the [capability audit](parity-audit.md). That one asked whether
the capabilities were ported; this one asks what the interface showed. Where the
two touch, §5 says so rather than restating.

---

## 0. Method, and one correction

### Read

| | Source |
|---|---|
| v1 | `archive/v0.4.1-pre-rewrite` via `git show` only, never checked out. All 166 `.ts`/`.tsx`/`.rs` files under `crates/copypaste-ui/src/` and `src-tauri/src/`. |
| v2 | `crates/copypaste-ui/src/` (36 files) and `src-tauri/src/` at working-tree `HEAD` (`0d547fbe`). |
| Spec | `port-manifest/06-ui-behaviour.md` (39 invariants, §3.0–§3.8 screen contracts, 15 A11Y requirements, 73 acceptance tests) and `port-manifest/README.md` on what binds. |
| Not used as a requirement | `docs/design/`, `design-reference.html`. Read only to learn what a screen *contained*; v1's look is explicitly not carried over. |

### The correction

**`parity-audit.md` is stale on the UI.** It anchors at `c53be35b`, where v2 had
"1 screen + tray" and no Settings, Devices, pairing, bulk actions, filter, sort,
load-more, tray or hotkey. All of those have since landed. Six of its
UI-related findings — 1, 11, 19 in part, plus INV-4, INV-23/24, the A11Y-4/6/7/8
"N/A" row — are no longer true, and its §2.7 table should not be read as current.
I have not amended it; anyone quoting it on the UI should re-check first.

### Verdict vocabulary

| Verdict | Means |
|---|---|
| **Present** | The user can see and do the same thing. |
| **Reduced** | Present, but with less of it. Said what is gone. |
| **Solved differently** | Different mechanism, same outcome for the user. Not a loss. |
| **Dropped (recorded)** | Absent, and the absence is written down somewhere I cite. |
| **⚠ Missing** | Absent, and I found no record in `CLAUDE.md`, the four ADRs, the manifests, `README.md`, `SECURITY.md`, `parity-audit.md`, `security-review.md`, or a source comment. |

A known bug means v2's UI does not currently render in a real browser. That is
being fixed and does not affect a source reading; nothing below depends on
having run it.

---

## 1. The twelve, ranked by what a user loses

Ranked by loss, not by cost. "Where I looked" is given for every row — a false
alarm costs real time.

| # | Missing | What the user loses | Where I looked |
|---|---|---|---|
| **1** | **Any way to see an item's full content.** v1 had a Details modal (M10) opened by the row's eye button: full text, plus `Name / Type / Copied / Source`. | Every row is clamped to the preview-lines setting (1–6, default 2) with no expand, no hover tooltip carrying the text, and no detail view. Choosing between three similar long clips — a paragraph, a JSON blob, two URLs differing in the path — is guesswork; the only way to read one is to copy it into another app. **The content is already at the frontend**: `model.rs` sends the whole string for any non-sensitive item, so this is a missing view, not missing data. | v2 `components/history/HistoryRow.tsx` (`WebkitLineClamp: previewLines`; `title` is `"Click to select · double-click to copy"`, not the content); `grep -rin "detail\|expand" src/components src/hooks` → only `aria-expanded` in `ui/README.md`; `src-tauri/src/model.rs:55-76`. v1 `views/HistoryView/DetailsModal.tsx` (263 lines), `HistoryRow.tsx:337-345` (the eye button). |
| **2** | **The degraded-database state and its "Reset database" escape hatch.** | When the store cannot be decrypted, v2 shows the generic `Failed to load history` with a **Try again** button, which retries forever against a condition retrying cannot fix. v1 said `Clipboard database can't be opened`, explained that the key no longer matches, and offered `Reset database (erases local history)` behind a confirm modal. This is also where **CLAUDE.md rule 3's one obligation** would be discharged — "a v2 build that stumbles onto [a v1 database] must say so plainly rather than failing with a decryption error that reads like corruption". There is no screen that can say it. | v2 `components/history/HistoryView.tsx:233-258` (the whole state chain: offline / not_ready / generic error / filtered / empty); `grep -rn "degraded\|resetDatabase\|reset_database" crates/copypaste-ui/` → no matches. Manifest 06 §3.1.11 (binding; lists degraded as its own row with exact copy). Related but different: `parity-audit.md` finding 6 covers the *IPC verb* `reset_database`. |
| **3** | **Which device a clip came from, anywhere.** v1's row meta line was `kind · time · origin device · source app`, with a device-filter dropdown, a by-device sort toggle, and a "Group by device" preference. | With sync on, every row looks local. The user cannot tell what arrived from the phone, cannot filter to it, and cannot sort by it. `Item` on the bridge has six fields and none of them is an origin, so this is a bridge gap as much as a UI one. | v2 `src/lib/ipc.ts:23-31` (`Item`: id, content, content_type, created_at, pinned, is_sensitive); `components/history/HistoryRow.tsx:197-208` (meta line: time, Pinned, · Sensitive); `lib/view.ts` (filter is by *kind*; sort is newest/oldest). v1 `lib/clip/ClipMetadata.tsx`, `views/HistoryView/hooks/useHistoryFilter.ts`, `store.ts` (`sortByDevice`). Partly touched by `parity-audit.md` finding 19, whose premise ("v2 has one flat list") has since stopped being true. |
| **4** | **Whether sync is actually working — per device, and at all.** v1's DeviceCard showed `Paired / Last sync / RTT` and a verified badge; the sidebar chip carried last-sync time, account, and a **per-peer stall pill**. | **Wire half closed** — `PeerInfo::last_sync_ms` (`ipc/src/payload.rs`); no component reads it. v2's peer row shows *Last seen* (a discovery signal) and never *last synced*. `StatusChip` is entirely about the local service — running / paused / starting / offline / error — and has no sync dimension. Manifest 06 §3.8 states the exact failure this guards against: "the badge can read `synced` while one peer silently receives nothing". A user has no way to notice that. | v2 `components/devices/DevicesView.tsx:140-149`; `components/shell/StatusChip.tsx` (five states, none about sync); `src/lib/ipc.ts` `PeerInfo` (no `last_sync_at`, no failure counter) and `SyncResult` (per-run only, not persisted). v1 `components/DeviceCard.tsx`, `components/SyncStatusChip.tsx` (458 lines). Manifest 06 §3.8. |
| **5** | **The Recent submenu in the menu-bar item.** v1's tray was `Open CopyPaste`, `Recent ▸` (last 10, 40-char labels, click to copy), `Private Mode`, `Quit`. | While the window is hidden, the tray is the whole app. v2's is `Show CopyPaste`, `Open at Login`, `Quit` — three navigation verbs and nothing you can *do*. Copying a recent item without bringing up a window is gone. | v2 `src-tauri/src/shell/tray.rs` (three ids: `toggle`, `autostart`, `quit`). v1 `src-tauri/src/tray.rs` (`rebuild_recent_submenu`, 10 items, 5 s refresh). Manifest 06 §3.6. (`Private Mode` is dropped-with-a-record — README, SECURITY.md.) |
| **6** | **The Logs tab.** v1 Settings › Logs: tail of the daemon log, per-level colouring, a text filter, a live-tail toggle, download, and the log directory with the username redacted to `~` (`CopyPaste-2b3i`). | No in-app diagnostics at all. A user reporting a bug has nothing to attach, and no path is safe to tell them to open by hand. | v2 `components/settings/SettingsView.tsx` `TABS` (six: appearance, list, shortcut, sync, storage, about); `grep -rn "readLogs\|log_dir\|Logs" src/ src-tauri/src/` → no matches. v1 `views/LogView.tsx` (301 lines). Manifest 06 §3.4.10. |
| **7** | **Escape does not dismiss the window.** | v2's popover hides on blur and on the tray/hotkey toggle. Escape inside the list clears the selection; Escape in the search field clears the query; nothing at the top level hides the window. For a popover summoned by a hotkey, Escape is *the* dismissal, and v1's popup had it. | `grep -rn "Escape" crates/copypaste-ui/src crates/copypaste-ui/src-tauri/src` → `HistoryList.tsx:229`, `SearchBar.tsx:97`, Radix dialog only. `src-tauri/src/shell/window.rs:175-185` handles `CloseRequested` and `Focused(false)`, not a key. v1 `popup/Popup.tsx:302-305`. |
| **8** | **The app's own version, and every external link.** v1's About: app version from the Tauri bundle, tagline, feature list, `Check for Updates` → releases, GitHub, Changelog, Privacy policy. | v2's About reports the **service** version, backend, protocol version and item count. A user cannot tell which build of the app they are running, cannot reach the changelog, and cannot find the privacy policy. | v2 `components/settings/AboutTab.tsx`; `grep -rn "getVersion\|@tauri-apps/api/app" src/` → no matches. v1 `views/AboutView.tsx:1-194`. |
| **9** | **"Too large to sync" on a row.** v1 drew a warning icon, visible even in selection mode (`CopyPaste-f72f`), on any item over the sync size cap. | An item that will silently never reach the other device looks exactly like one that will. | v2 `src/lib/ipc.ts` `Item` (no such field); `HistoryRow.tsx` (no warning affordance). v1 `views/HistoryView/HistoryRow.tsx:315-325`. |
| **10** | **Launch at login is invisible from Settings.** | It exists only as a tray check item, and `tray.rs` states it is read once at build and never re-read, so a change made in System Settings shows the wrong tick until restart. A user looking in Settings for a startup option finds nothing — v1 had it on the General tab. | v2 `src-tauri/src/shell/tray.rs:36-47` (and its own comment on the staleness); all six Settings tabs. v1 `views/SettingsView/tabs/GeneralTab.tsx` (`Launch at login`). |
| **11** | **Bulk copy, and a visible Select-all.** | v2's bulk bar is Pin/Unpin · Delete · Done. v1's also had **Copy** (concatenates the selection) and a **Select all / Deselect all** toggle. ⌘A works in v2's list but has no on-screen control and the bulk bar's own text does not mention it, so in selection mode — the exact moment you want it — it is undiscoverable. | v2 `components/history/BulkBar.tsx`; `components/history/HistoryList.tsx:160-165` (⌘A, list-focused only); `SearchBar.tsx` `title` mentions ⌘A but the bar does not. v1 `views/HistoryView/BulkActionBar.tsx`. |
| **12** | **⌘1–⌘9 row numerals.** v1's popup drew a keycap numeral on each of the first nine rows. | v2 advertises the shortcut once in the footer strip; no row carries its number, so the user counts. Partly substituted by `QuickHint`, which is why this is last. | v2 `components/history/QuickHint.tsx`, `HistoryRow.tsx` (no index rendered). v1 `popup/PopupRow.tsx` (`showKeycap={!showQuery && idx < 9}`). |

**The shape of the list.** Nine of the twelve are *information* rather than
actions: what an item says, where it came from, whether sync ran, whether the
app is stuck. v2 rebuilt the verbs faithfully and lost the readouts around them.

---

## 2. Surface by surface

### 2.1 App shell, navigation, banners

| v1 | v2 verdict |
|---|---|
| Sidebar: History / Devices / Settings, `aria-current="page"`, `Primary` landmark; footer with app name + SyncStatusChip | **Present**, plus a bottom bar under `sm` for phones. Footer keeps the chip; the app-name label is gone (cosmetic). |
| Window: 980×640 document window + a separate 420-wide popup window | **Solved differently** — one 420×600 popover anchored under the tray icon, hides on blur (`shell/window.rs`). Recorded indirectly: ADR-0003 §"popover show/hide/focus", README's "menu-bar item, popover". Consequence worth naming: there is no longer a large window in which a details view or a two-pane layout would be natural, which makes finding 1 harder to close than it looks. |
| Banner priority queue, one at a time (INV-17): daemon-error, protocol-mismatch, stale-daemon, accessibility | **Solved differently, better** — `lib/banners.ts` is a pure ordered function (v1's was a ternary chain that could stack). Set differs: service-offline, protocol-mismatch, capture-paused. Stale-daemon became the `ServiceOffline` screen's "out of date" branch (ADR-0004); the accessibility banner is unnecessary because ADR-0001/`shell/hotkey.rs` keep the app out of Accessibility entirely. |
| Error boundaries `Navigation` / `Main` / view (INV-20) | **Present** — `App.tsx`, and the shell is outside them as INV-20 requires. |
| Toasts: bottom-right stack, pause on hover/focus, dismiss button, 3000 ms (§3.7) | **Solved differently** — `sonner`, mounted once in `main.tsx` with those settings. Correct call under CLAUDE.md rule 1; v1 hand-rolled it and had two implementations. |
| `?view=gallery` dev component gallery | **Dropped**, no record. Dev tooling, not a user surface — not counted. |

### 2.2 History

| v1 | v2 verdict |
|---|---|
| Virtualised list, search (250 ms debounce), copy / pin / delete per row | **Present**. |
| Row anatomy: content tile, preview, `kind · time · origin device · source app`, pin indicator, sync-size warning, hover actions | **Reduced** — see findings 1, 3, 9. Time gains an absolute-time tooltip; actions are always visible rather than hover-only (deliberate: Android has no hover, stated in `HistoryRow.tsx`). |
| Keyboard: Esc, ⌘F, ⌘A, ↑↓, Enter, ⌥Enter (paste as plain text), Backspace/Delete | **Present and extended** — v2 adds ⌘1–⌘9, Home/End, and Space in selection mode. ⌥Enter is **N/A**: v2 stores text only, so there is no rich form to strip. |
| Popup keyboard: ⌘1–⌘9, wrapping ↑↓, Enter, Esc | **Present**, except Esc (finding 7). Non-wrapping ↑↓ is deliberate — AT-10. |
| Multi-select: checkbox column, select all, bulk copy / pin / delete, confirm on bulk delete | **Reduced** — finding 11. |
| Delete with a 5 s undo toast, second delete commits the first | **Present** — `hooks/useDeferredDelete.ts`, and it also commits on unmount and on `pagehide`, which v1 did not. |
| Load-more on near-bottom | **Present**, plus an explicit **Load more** button (INV-4 satisfied; the parity audit's "no load-more" is out of date). |
| Search / filter / sort: FTS, device filter, by-device sort, count badge, display-limit hint | **Reduced** — filter is by *kind*, sort is newest/oldest, device axis gone (finding 3). Count badge present, and `historyCount` is now one function, which is `CopyPaste-g27b.37` fixed. |
| Drag-to-reorder pinned items | **⚠ Missing** — already `parity-audit.md` finding 19. |
| Add-file button + OS drag-drop overlay | **Dropped (recorded)** — text-only, README "Also absent: image, file and rich-text capture". |
| Details modal | **⚠ Missing** — finding 1. |
| Empty / loading / error states | **Present and extended**: loading, offline (a real `ServiceOffline` screen with Start / Restart / recheck, per ADR-0004 — strictly better than v1's message), not_ready, generic error with retry, no-results, empty. Two gone: **degraded** (finding 2) and **private mode is on** (dropped with a record — private mode is in README and SECURITY.md). |
| Skipped-undecryptable count | **Present** — `SkippedNotice.tsx`. `parity-audit.md` finding 17 is closed. |

### 2.3 Devices and pairing

| v1 | v2 verdict |
|---|---|
| Own-device card (This Mac: model, OS, version, local/public IP) | **⚠ Missing**, low impact on its own — folded into finding 4, since the useful half is identity for the peer list. |
| Paired list: name, verified badge, `Paired / Last sync / RTT`, online dot, unpair, revoke | **Reduced** — name, last *seen*, address, online badge, sync-now, unpair. Finding 4. Revocation is `parity-audit.md` finding 7. |
| Discovered-on-network section + Rescan | **⚠ Missing** — already `parity-audit.md` finding 16. |
| QR pairing modal, click-to-reveal | **Present** — `PairCreateDialog.tsx` + `QrCode.tsx`, with the reveal gate and INV-13 kept. |
| SAS pairing modal (both sides confirm a short code, peer metadata labelled unverified) | **Solved differently** — Noise `NNpsk0`, where holding the token *is* the authentication (manifest 02 §6.3 recommends exactly this). INV-15's "unverified" labelling survives on the peer name. |
| Revoke-all + its confirm dialog | **⚠ Missing** — subsumed by `parity-audit.md` finding 7 (no revocation at all). |
| Incoming-pairing event auto-switches to Devices | **⚠ Missing**, minor: v2's responder flow is a dialog the user opens. Not counted separately. |

### 2.4 Settings

v1: 7 tabs. v2: 6. Not the same six.

| v1 tab | v2 |
|---|---|
| **General** — enable sync, private mode, sound on copy, notify on copy, public-IP lookup, paste-as-plain-text, allow screenshots, excluded apps, service version + restart, launch at login | **Gone as a tab.** Every setting on it is either dropped-with-a-record (private mode, exclusion list, notifications/sound → README + SECURITY.md + `parity-audit.md` 18) or missing-with-a-record (`parity-audit.md` finding 9, no daemon config). Two are not covered by either: **launch at login** (finding 10) and **allow screenshots** — INV-35 says capture protection is on by default and `tauri.conf.json` sets none, which is `parity-audit.md`'s INV-35 row. |
| **Display** | **Split into Appearance + List.** Theme / accent / translucency present, and "System" now says what it resolves to. Preview lines and warn-before-reveal present. Gone: image height, group-by-device, mask-sensitive (deliberate and argued in `store/prefs.ts` — the bridge drops the plaintext, so the toggle could not be honoured). |
| **Sync** | **Reduced to a status page** — paired count, Sync now, "Cloud sync: Unavailable". Credentials, passphrase, wifi-only, LAN visibility, auto-apply are `parity-audit.md` findings 9 and the cloud rows. |
| **Shortcuts** | **Present** — `ShortcutTab.tsx` captures a physical-key accelerator (INV-23), refuses permission-costing keys (`shell/hotkey.rs`), keeps A11Y-13's raw-accelerator accessible name, resets to default. |
| **Storage** | **Reduced** — item count, "set by the service", clear history. Limits, quota, sensitive TTL, display limit, export/import, vacuum and DB stats are `parity-audit.md` findings 3, 5, 6, 9. **Not** in that audit: the database's size on disk, which v1 showed. Minor; not counted separately. |
| **About** | **Present, reduced** — finding 8. |
| **Logs** | **⚠ Missing** — finding 6. |

### 2.5 Menu-bar tray

| v1 menu | v2 |
|---|---|
| Open CopyPaste | **Present** (`Show CopyPaste`; left-click also toggles, which v1 did not). |
| Recent ▸ (10 items, copy on click, 5 s refresh, 40-char labels) | **⚠ Missing** — finding 5. |
| Private Mode (check item, reverts on IPC failure — INV-38) | **Dropped (recorded)** — README, SECURITY.md. |
| — | **Added**: Open at Login (finding 10 is that it is *only* here). |
| Quit CopyPaste | **Present**, and load-bearing: the window has no decorations, so this is the only exit (INV-36 kept). |

### 2.6 Things easy to forget

| | Verdict |
|---|---|
| Right-click / context menus | **Neither version has any.** `grep -rn "onContextMenu" `→ no matches on either branch. Not a regression. |
| Onboarding / first-run flow | **Neither version has one.** Only match on either side is `shell/autostart.rs`. Not a regression. |
| Tooltips carrying real information | **Present and mostly better** — the search field carries the shortcut map (`Search (⌘F) · ↓ to move into the list · ⌘A select all`), the time carries the absolute timestamp (v1 did not), the peer name says it is unverified, the status chip's accessible name is the whole sentence. Gone with their features: "Too large to sync" (9), origin device (3). |
| Copy feedback | **Present** — row flash plus `Copied — press ⌘V to paste`. The toast tells the user the app does not synthesise a paste, which is a stated ADR-0001 consequence rather than a gap. |
| Item counts and "is it working" indicators | **Present** — count badge, status chip with version and item count, skipped-item notice, capture-paused banner. Missing the sync half (4). |
| Announcements / live regions | **Present** — list announcer as a sibling (A11Y-14 / `CopyPaste-wrfn`), `aria-live` on the count, on the bulk-bar count, on the skipped notice. |
| Localisation | **Missing in v2**, already `parity-audit.md` §2.9. v1 shipped `values-uk` on Android only, so no desktop UI string was ever translated. |

---

## 3. Where v2 is ahead

Listed so the ledger is honest, and so nobody "restores" one of these.

| | |
|---|---|
| Offline recovery | v1 said the service was down. v2 offers Start / Restart with four distinct states and no path in any of them (`ServiceOffline.tsx`, ADR-0004). |
| Sensitive content | v1 blurred it in place — present in the DOM, in the a11y tree, in a screenshot. v2's bridge drops the plaintext (`model.rs`), so the row renders a withheld slot and reveal is an explicit round trip. |
| Banner de-confliction | One pure ordered function instead of four independent `role="alert"` mechanisms and a ternary chain (`CopyPaste-8ebg.39`). |
| Empty states | v1 shipped classless empty `<span>`s that rendered as nothing (`CopyPaste-8ebg.29`, `bdac.2`). v2's spinner and icon are real elements. |
| Settings tab row | Wraps at the 720px minimum, where v1's last tab was entirely off-screen (`CopyPaste-g27b.31`). |
| Row memoisation | Plain `memo` instead of v1's twenty-field comparator, which silently stopped re-rendering whenever `Item` grew a field. |
| Phone layout | Nav moves to the bottom band, targets take the coarse-pointer floor. v1's desktop UI had no phone form at all. |

---

## 4. What to do, in order

| Do | Why now |
|---|---|
| 1. A details view (finding 1) | Everyday, and the data is already at the frontend — this is a component, not a protocol change. |
| 2. The degraded state and its copy (finding 2) | Discharges CLAUDE.md rule 3's one obligation, which currently has nowhere to be said. Needs `reset_database` back on the wire (`parity-audit.md` finding 6). |
| 3. `origin_device` and `last_sync_at` on the wire (findings 3, 4) | Both are IPC-shaped. Adding them once unblocks the row badge, the device filter, the per-peer readout and the stall pill together — and the UI is what exposes the awkward parts of an API (CLAUDE.md rule 6). |
| 4. Escape dismisses; Recent submenu (findings 7, 5) | Small, self-contained, and both are about the app being usable without its window. |
| 5. About links, launch-at-login row, bulk copy, row numerals (8, 10, 11, 12) | Cheap. Batch them. |
| 6. Logs tab (finding 6) | Needs a log-read verb, so it is the largest of the twelve; also the one a user only needs when something is already wrong. |

---

## 5. Deliberately not repeated from `parity-audit.md`

These have a UI face and are already findings there: pairing UI (1, now landed),
sensitive auto-wipe (3), export/import (5), backup/restore (6), revocation (7),
settings/config (9), keyset pagination (10), popup and hotkey (11, now landed),
streaming updates (15, now partly landed as `usePush`), discovery listing (16),
skipped count (17, now closed), notifications and sound (18), bulk/filter/sort
and drag-to-reorder (19, of which only drag-to-reorder is still true), and
INV-35 screen-capture protection.

## 6. What this does not establish

Nothing here was run. Every verdict is "the source says so". The macOS shell —
tray, popover placement, hotkey, launch-at-login — is unexercised on any Mac by
anyone, as `src-tauri/src/lib.rs` says of itself, so "present" for those means
present in code. I did not audit visual design, and did not treat `docs/design/`
or `design-reference.html` as requirements. I did not read v1's Android UI (304
Kotlin files); v2 has no Android app to compare it to.
