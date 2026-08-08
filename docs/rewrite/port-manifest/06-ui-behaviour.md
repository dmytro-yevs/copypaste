# 06 — UI Behaviour Contract (harvested from the Tauri/React UI)

> **Audience:** the team rewriting the UI on React Query + TanStack Virtual +
> Radix/shadcn + sonner + Tailwind + zod + zustand.
>
> This is a **behaviour spec, not a code port.** Nothing here prescribes an
> implementation. Every clause exists because a user-visible bug was fixed, a
> security review demanded it, or a perf/token-burn budget required it. Where a
> clause has a bug id (`CopyPaste-<id>`), the old code carries the receipt.

---

## 1. Purpose & scope

### 1.1 What this covers

The complete user-facing behaviour of the desktop app across its **two windows**
and **one tray surface**:

| Surface | Old entry point | Notes |
|---|---|---|
| Main window (`main`) | `src/App.tsx`, `index.html` | History / Devices / Settings |
| Quick-Paste popup (`popup`) | `src/popup/Popup.tsx`, `popup.html` | separate WebView, separate JS realm |
| Menu-bar tray | `src-tauri/src/tray.rs` | native menu, no WebView |

Screens: **History**, **Devices**, **Settings** (with `general / display / sync /
shortcuts / storage / about / logs` sub-tabs), plus the **Quick-Paste popup** and
a dev-only **Gallery** (`?view=gallery`, DEV builds only).

### 1.2 What this does NOT cover

- Daemon/IPC wire protocol — see `docs/protocol.md` and the other port-manifest
  documents.
- Crypto/pairing internals — see `02-crypto.md`.
- Sync/backend semantics — see `05-sync-and-backend.md`.
- Android parity details beyond what the desktop UI mirrors.

### 1.3 Reference material that must survive the rewrite

- **`copypaste-design-reference.html`** (repo root) — the canonical visual
  reference. `crates/copypaste-ui/src/styles/tokens.css` is **value-for-value
  parity-locked** to its token block, enforced by
  `src/styles/tokens.parity.test.ts`. See §8 for the full token dump.
- `crates/copypaste-ui/PERF-BASELINE.md` — perf baselines the polling budget was
  tuned against.
- `docs/liquid-glass-styleguide.html`, `STYLEGUIDE.md`.

---

## 2. Invariants (MUST hold)

These are the non-negotiables. A rewrite that breaks any of these regresses a
shipped bug fix, a security property, or a perf budget.

### INV-1 — Scroll position must be anchored to *content*, never to pixels
When the history list mutates (poll prepends a new clip, load-more appends a
page, a delete/undo/pin re-sorts), the row the user is looking at MUST stay under
the same point on screen. Anchor by **item id + intra-row offset**, re-derived
against the new layout. If the anchor item is gone (it was the one deleted), fall
through to the browser's natural position and clamp.
*Old: `VirtualList.tsx` (CopyPaste-8ebg.44).*

### INV-2 — Identical data MUST NOT produce a new list reference
A background poll that returns byte-identical data MUST NOT cause a re-render or
a new array identity. The old code hashed `id|pinned|wall_time` per item into a
signature and short-circuited `setItems` on a match. This is load-bearing for
INV-1 (the anchoring effect keys on reference change) **and** for CPU/battery.
*Old: `HistoryView/historySignature.ts`, `hooks/useHistoryData.ts`.*

### INV-3 — Any local mutation MUST invalidate the dedup signature
Every optimistic mutation (copy-to-top, delete, undo, bulk delete, bulk pin,
clear-all, reset-db) MUST clear the cached signature, or the next poll compares
fresh server data against a stale fingerprint, "matches", and silently discards
the server truth. *Old: CopyPaste-8ebg.18, 8ebg.16.*

### INV-4 — Load-more must merge, never replace
After pages 2..N are loaded, the signature/cache MUST reflect the **merged** list.
Otherwise the next poll's first-page response replaces the merged list and every
loaded-more item vanishes. Merge de-duplicates by id. *Old: CopyPaste-8ebg.16.*

### INV-5 — Row heights MUST be over-reserved, never estimated by character count
The virtualizer's reserved height for a row MUST be ≥ the row's real rendered
box. For multi-line previews, reserve the **full `previewLines` cap** — the
rendered height depends on pane width, which a pure height function cannot know.
Width-agnostic char-count heuristics caused site-wide row overlap.
*Old: `historyVirtualizer.ts` (CopyPaste-g27b.30).*

### INV-6 — Scroll position MUST be clamped when content shrinks
When a display setting or a filter/delete shrinks total content height below the
current `scrollTop`, both the tracked scroll state and the real DOM `scrollTop`
MUST be clamped immediately — not on the next user scroll. Otherwise the visible
window degenerates to the last row or two and the list looks broken.
*Old: CopyPaste-f2ec #17.*

### INV-7 — `aria-activedescendant`-style pointers MUST only reference mounted nodes
With virtualization the active row may be outside the rendered window. The active
pointer MUST be cleared when its target is not rendered. *Old: CopyPaste-5917.33.*

### INV-8 — Rows carry interactive children, so they MUST NOT be `role="option"`
`role="option"` is `childrenPresentational`, which flattens the per-row
Pin/Preview/Delete buttons and the multi-select checkbox → axe
`nested-interactive` (serious, WCAG 4.1.2). Use `role="list"`/`role="listitem"`
and expose selection via `aria-current`. Compensate for the lost automatic
announcement with a polite live region (INV-9). *Old: CopyPaste-g27b.29, 8ebg.45.*

### INV-9 — Keyboard selection changes MUST be announced
Because of INV-8 there is no automatic AT announcement on arrow-key movement. A
visually hidden `aria-live="polite" aria-atomic="true"` region MUST mirror the
active row's accessible label. It MUST be a **sibling** of the list element, not
a child — a live region inside `role="list"` fails
`aria-required-children`. *Old: CopyPaste-8ebg.45, CopyPaste-wrfn.*

### INV-10 — Masked/sensitive content MUST NOT reach the accessibility tree
A blurred sensitive row announces a fixed placeholder
(`"Sensitive item, hidden — activate to reveal"`), never the plaintext. Its
checkbox announces `"Select (sensitive item)"`, not the preview.
*Old: `ClipPreview.tsx` `MASKED_A11Y_LABEL`, `HistoryRow.tsx` (A11Y-1).*

### INV-11 — Revealed secrets MUST re-hide automatically
Revealed sensitive content re-blurs on **window blur** (SCRH-7) **and** after a
**10 s** idle timer (CopyPaste-5917.56). Both, independently.

### INV-12 — Raw errors MUST NOT be rendered
No code path may render `String(err)` or `IpcError.message` as visible text —
those strings can contain the daemon Unix socket path, which leaks the local
username into the DOM, screenshots, and the a11y tree. All user-visible error
text goes through a code→copy mapping; the raw error is `console.error`'d only.
*Old: `friendlyIpcError()`, ERR-1/ERR-2, CopyPaste-tzzu, CopyPaste-j5qg.*

### INV-13 — The pairing QR payload string MUST NEVER enter the DOM
Only the rendered QR **SVG** may be shown. The `CPPAIR2.*` payload (PAKE
password, cert fingerprint, Supabase anon key) must never be rendered as text,
even when the QR is revealed. *Old: CopyPaste-1jms.5.*

### INV-14 — The SAS code is display-only
Six digits, rendered one glyph per element, `user-select: none`, no
click-to-copy. A clipboard reader must not be able to lift the live PAKE
secret. *Old: CopyPaste-1jms.1.*

### INV-15 — Peer-advertised metadata shown next to the SAS MUST be labelled unverified
mDNS-advertised name / model / OS / version / IP / fingerprint are self-reported
and unauthenticated until the SAS handshake completes. They MUST be visually and
textually de-emphasised and labelled ("Unverified — reported by the peer, not yet
confirmed" / "Unverified device details"). *Old: CopyPaste-8ebg.51.*

### INV-16 — Closing a pairing modal MUST reset the daemon state machine
Every close path calls `pair_abort` (best-effort, failure ignored) so a
subsequent LAN pairing attempt is not blocked — including after a terminal
`confirmed`. *Old: CopyPaste-1jms.3, 1jms.12.*

### INV-17 — Only one alert banner is visible at a time (priority queue)
Four independent `role="alert"` mechanisms exist. They MUST be ranked and only
the winner rendered:
1. `daemon-error` (P0, non-dismissible) — background service failed to start.
2. `protocol-mismatch` (P1, dismissible).
3. `stale-daemon` (P1, dismissible + Restart action).
4. `accessibility` (P2, dismissible + Open Settings action).
Losers keep their own state so they appear once the winner clears.
*Old: `App.tsx` (CopyPaste-8ebg.39).*

### INV-18 — Only one confirm modal may be open at a time
Opening "Revoke all" closes any open single-device revoke prompt, and vice
versa. Two coexisting portals stack their scrims. *Old: CopyPaste-g27b.36a.*

### INV-19 — Body scroll-lock MUST be reference counted
Nested/stacked dialogs: only the last one to close restores the original
`overflow`. *Old: `lib/dialog/scrollLock.ts`.*

### INV-20 — The shell chrome must never be inside an error boundary
`.app` (the shell) is never wrapped. Sidebar and the main pane each get their
**own sibling** boundary. A crash in a view must not take down navigation, and
every fallback renders inside the shell layout, not against a bare document
body. *Old: CopyPaste-8ebg.12.*

### INV-21 — Prefs corruption defaults **per field**, never wholesale
An invalid `theme` must not discard a valid `accent`. Unknown keys are dropped
and never re-persisted. Malformed JSON / non-object payload / storage exception
→ full defaults, logged, never thrown. *Old: `store.ts`, `prefsSchema.ts`.*

### INV-22 — First paint MUST already carry the persisted appearance
A synchronous, dependency-free, pre-paint script sets
`data-theme` / `data-theme-pref` / `data-accent` / `data-translucency` on
`<html>` before the app module runs. No default-theme flash. It cannot use
`import`/`eval` (CSP is `script-src 'self'`, no inline, no nonce).
*Old: `public/theme-bootstrap.js`.*

### INV-23 — Global shortcuts are captured from the **physical** key
Accelerators derive from `KeyboardEvent.code`, never `.key`, so a Cyrillic /
Dvorak / AZERTY layout still records the same physical binding.
*Old: `ShortcutCapture.tsx` `eventToAccelerator()`.*

### INV-24 — A shortcut that fails to register MUST NOT crash startup
The saved accelerator is preserved in state and a warning logged; on macOS the
CGEventTap may still service it. *Old: `src-tauri/src/lib.rs` setup.*

### INV-25 — Hiding the popup must hand focus to the **prior app**, not the main window
Never call `window.hide()` from JS. Always route through the backend hide path,
which activates the recorded prior bundle id first (or temporarily flips
activation policy to Accessory when no prior app is recorded).
*Old: V-10 / V-11 / D7 fixes in `src-tauri/src/popup/window.rs`.*

### INV-26 — Copy-then-hide, never hide-then-copy
In the popup, the daemon copy must complete **before** hiding. Hiding first
swallowed errors and produced every-other-click races. On copy failure the popup
stays visible with the error. *Old: HW-M6.*

### INV-27 — Polling is visibility-gated
Every recurring poll stops when `document.visibilityState !== "visible"` and
does an **immediate** refresh on becoming visible. This is a battery/token-burn
requirement, not an optimisation. Applies to: history poll, popup poll, QR
countdown/regeneration.

### INV-28 — Single-use pairing tokens must not burn while nobody is looking
The QR auto-refresh tick is visibility-gated; a hidden window must not keep
minting fresh single-use PAKE tokens. *Old: `useQrCode.ts`.*

### INV-29 — Optimistic writes must revert on failure
Toggles, excluded-app add/remove, slider saves, drag-reorder: apply optimistically,
revert the **specific field** on failure (never a full reload that resets every
other unsaved control), and surface the error.
*Old: P1 fix in `saveLimitsField`, CopyPaste-8ebg.20.*

### INV-30 — Busy flags must always be released
Bulk operations release their busy flag in `finally`, even if the refresh throws,
so the bulk bar is never permanently disabled. *Old: V-13.*

### INV-31 — Pinned items must not jump to the top on copy
Copy optimistically moves an **unpinned** item to the top of the unpinned
section (i.e. after the last pinned row). A pinned item keeps its `pin_order`
slot; the daemon only bumps `wall_time`.

### INV-32 — Selection is tracked by **id**, never by index
A background poll can reorder the list between keydown and Enter. Track the
selected item's id and re-resolve its index each render.
*Old: CopyPaste-8ebg.17.*

### INV-33 — Late responses must not clobber newer ones
Concurrently-triggered refreshes (mount / focus / poll / manual retry) are
sequence-tagged; only the latest issued response is applied.
*Old: CopyPaste-8ebg.56 `requestSeqRef`.*

### INV-34 — `migration_in_progress` is the only auto-retried error
Retry with exponential backoff 250 → 500 → 1000 → 2000 → 2000 ms, max 5
attempts, then propagate. No other error code is retried — retrying arbitrary
errors masks bugs. *Old: `transport.ts` (ro0r).*

### INV-35 — Screen-capture protection is on by default
Both windows set content protection (macOS `NSWindowSharingNone`) unless the
user explicitly enables "Allow screenshots". Failure is non-fatal (log &
continue). *Old: PG-25 / CopyPaste-13a3 / CopyPaste-6uy9.*

### INV-36 — Closing the main window hides it; only tray → Quit exits
Close is intercepted (`prevent_close` + `hide`). Only an explicit Quit
terminates the process and stops the app-owned daemon.

### INV-37 — Blocking IPC must never run on the UI/main thread
Tray menu handlers (private-mode toggle, recent-item copy) and tray startup MUST
offload IPC to a background thread; a blocking call freezes the menu bar for up
to the socket read timeout. *Old: CopyPaste-8ebg.23.*

### INV-38 — Tray checkmark reflects daemon truth, and reverts on IPC failure
The tray pre-toggles optimistically; on IPC error the checkmark is reverted and
the corrected value broadcast. *Old: V-21-B.*

### INV-39 — Private mode converges across every surface
Tray, Settings, and the daemon converge via a `private-mode-changed` broadcast
plus re-fetch on window focus/visibility. Whoever toggles it, everyone agrees.
*Old: M4.*

---

## 3. Screen-by-screen behaviour contract

### 3.0 App shell

*Old: `src/App.tsx`, `src/components/Sidebar.tsx`, `src/styles/shell.css`.*

- Layout: fixed left sidebar (`Primary` nav landmark) + main column. Sidebar
  items: History, Devices, Settings; active item carries `aria-current="page"`.
  Sidebar footer shows the app name and the **SyncStatusChip**.
- Window: 980×640 default, **minWidth 720 / minHeight 460**, transparent,
  decorations on, `titleBarStyle: "Overlay"`, `hiddenTitle: true`, drag-drop
  enabled. A `data-tauri-drag-region` element sits at the top of the sidebar.
- Routing: in-memory only (zustand `view`). Any unrecognised value (including
  `"gallery"`) resolves to `"history"` — defensive narrowing, **not** persisted
  state recovery (`lib/resolveView.ts`).
- No view transition animation. The old animated crossfade was deliberately
  stripped (CopyPaste-h1n3); a `data-testid="view-transition"` wrapper with
  `display: contents` remains so it does not break the flex height chain that
  scroll regions depend on.
- Error boundaries per INV-20, labelled `"Navigation"`, `"Main"`, and the active
  view's label.
- Banner priority queue per INV-17. Exact copy in §3.6 / below.
- App-global side effects started on mount:
  - peer-presence polling (§5),
  - `open-settings` listener (popup gear → navigate to Settings),
  - `incoming-pairing` listener → seeds the responder SAS modal and switches to
    the Devices tab,
  - protocol-mismatch handler registration,
  - daemon spawn-error probe + `daemon-spawn-result` listener,
  - one-shot stale-daemon detection,
  - accessibility-permission polling (only while the banner would be shown).
- Live appearance sync: whenever `theme`/`accent`/`translucency` change, re-apply
  to `<html>`; the pre-paint bootstrap owns only the first paint.
- **Tauri feature-detection gate:** all `listen()` subscriptions are skipped when
  `window.__TAURI_INTERNALS__` is absent, otherwise every mount logs a console
  error in the browser/mock harness (audit P1-7).

**Banner copy (exact):**

- daemon-error (`role="alert"`, error, non-dismissible):
  `Background service error: The background service failed to start. Please reinstall CopyPaste or restart your Mac.`
- protocol-mismatch (`role="alert"`, warn, `Dismiss`):
  `CopyPaste app and background service are on incompatible versions (service protocol vN). Restart the app or the background service to resolve.`
- stale-daemon (`role="alert"`, warn, Restart + `Dismiss`):
  `CopyPaste was updated but an older background service is still running (build X). Restart it to use the new version.`
- accessibility (`role="alert" aria-live="assertive"`, warn,
  `Open Settings` + `Dismiss`):
  `Accessibility permission is required for the global paste shortcut and hotkey capture. Grant it in System Settings to enable these features.`
- accessibility-granted (`role="status" aria-live="polite"`, 3 s, fade at 2.5 s,
  `aria-label="Accessibility permission granted — closing shortly"`):
  `Accessibility permission granted — global paste shortcut and hotkey capture are active.`

---

### 3.1 History

*Old: `src/views/HistoryView.tsx` + `src/views/HistoryView/**`.*

#### 3.1.1 Data & refresh

- Initial fetch: `history_page(limit=200, offset=0)`. Daemon returns pinned
  first, then newest-first within each group; the UI does not re-sort by default.
- Auto-refresh: **3000 ms** while visible; **5000 ms** backoff while
  `offline | error | not_ready`. Visibility-gated with immediate refresh on
  becoming visible (INV-27).
- Immediate refetch triggers: window became visible; a `history-refresh` event
  (emitted by Settings after a backup import); after copy / pin / unpin /
  reorder / undo / bulk op / clear-all / db-reset.
- Dedup: per INV-2/3/4.
- The refresh interval reads load-state through a ref, **not** a dependency —
  adding it re-created the effect on every state recovery and double-fired.
- Total count comes from the daemon envelope (`page.total`) — the full DB count,
  not the loaded slice.
- Own device id comes from the page envelope (`own_device_id`, empty string on
  old daemons).
- Private-mode flag fetched once on mount (drives the empty-state copy).

#### 3.1.2 Load-more / pagination

- Threshold: fire when the scroll container is within **300 px** of the bottom.
- Guards: skip unless `ready`, `loaded < total`, and no fetch in flight.
- Page size 200 (server clamps with its own `MAX_PAGE`).
- Merge de-duplicated by id; update the dedup signature to the merged list
  (INV-4). A failed load-more is silent and self-healing (the next near-bottom
  event retries).
- **Load-more is disabled while a search query is active** — the filtered view
  operates over the already-loaded set, so "near bottom" doesn't mean "more data".
- **But**: when a search *is* active and `loaded < total`, an effect repeatedly
  drives load-more until everything is loaded, so FTS hits beyond the first page
  are not silently missing (CopyPaste-crh3.106).

#### 3.1.3 Virtualization

- Variable heights, prefix-sum offset table, binary search for the first visible
  row → O(log n) per scroll event.
- **Overscan: 240 px** above and below the viewport.
- Height rules (`rowHeightFor`), with `ROW_PAD_V = 18` (row's real 9px+9px
  vertical padding) as a hard floor:
  - **Image row:** `max(imageMaxHeight + max(ROW_PAD_V, densityPad), 34)`.
    `densityPad` = 20 spacious / 12 comfortable / 8 compact. The thumbnail is
    CSS-capped at exactly `imageMaxHeight` via a per-row `--img-max` var — the
    HTML `height` attribute is only a decode hint and does not bound layout
    (CopyPaste-g27b.25).
  - **File row:** fixed **44 px** (fits the FileChip).
  - **Text row:** `single = max(previewSize, base, 22)` where
    `base = max(SINGLE_LINE_FLOOR, 42|34|28)` and
    `SINGLE_LINE_FLOOR = 21 (title) + 2 (meta margin) + 18 (meta line) + 18 (pad) = 59`.
    For `previewLines > 1`: `single + (previewLines - 1) * 21`.
- Each row also publishes its computed height as `--row-max` so CSS collapse
  animations are bounded by the same number the virtualizer reserved.
- The density axis is **frozen to `"comfortable"`** in production. The three-value
  API survives in the height function only as historical floors.
- Rows are keyed by item id at the `map()` call site, not by index within the
  sliding window.
- Anchoring, clamping, and active-descendant rules per INV-1/6/7.
- A single absolutely-positioned "glide" layer draws the single-selection
  highlight behind the rows. **In multi-select it is hidden entirely** — the old
  first→last rectangle visually covered unselected interleaved rows
  (CopyPaste-5917.75). Multi-selection is shown per-row instead.
- Mount stagger: applied only to the first ≤10 rows on the very first painted
  frame, then permanently disabled via a ref (so filter/search re-renders are
  instant and never re-stagger).

#### 3.1.4 Keyboard (list focused)

| Key | Behaviour |
|---|---|
| `↓` / `↑` | Move selection, clamped at both ends (no wrap). Marks the move as keyboard-nav so the scroll-into-view effect runs. |
| `Enter` | Copy the selected item (same sound/notification gates as click-to-copy). |
| `Alt`+`Enter` | Paste as plain text (strip rich formatting). Failures surface a toast — not just a console log (CopyPaste-crh3.111). |
| `Backspace` / `Delete` | Delete selected, with undo window. Selection moves to the next row **before** removal. |
| `Escape` | Clear multi-selection if in selection mode, else clear single selection. |
| `⌘F` / `Ctrl+F` | Focus the search field and select its existing text. |
| `⌘A` / `Ctrl+A` | Select all currently-filtered items. |

- Keyboard scroll-into-view is computed from the **height model**, not
  `scrollIntoView` — the target row may not be in the DOM.
- Mouse hover clears the keyboard-nav flag so the auto-scroll doesn't fight the
  pointer.
- Shortcuts are discoverable via the search field's `title` tooltip
  (`Search (⌘F) · ⌘A select all · ⌥⏎ paste as plain text`) rather than a
  permanently-visible hint that crowded the header (CopyPaste-7w060.6).

#### 3.1.5 Row anatomy & actions

- Left: multi-select checkbox (`role="checkbox"`, `aria-checked`,
  `tabIndex` 0 only in selection mode, Enter/Space activate). Revealed by the
  list's `selecting` state.
- Content tile: image thumbnail, or a kind glyph coloured by `--c-<kind>`.
- Body: title + metadata. Title rendering by kind:
  - image → literal `"Image"`,
  - file → filename parsed from the daemon's `[file: <name>]` placeholder
    (fallback `"file"`),
  - masked/sensitive → the masked preview component,
  - url → hostname emphasised + path/query/hash dimmed,
  - code/json/num/color → monospace,
  - otherwise → preview with per-span redaction applied when the item has
    `sensitive_spans` and masking is on.
- Right: `too_large_to_sync` warning icon (**stays visible in selection mode** —
  CopyPaste-f72f), then Pin/Unpin, Preview, Delete (hidden in selection mode
  because the bulk bar duplicates them).
- Row click: in selection mode → toggle checkbox; otherwise → select **and**
  copy.
- Copy flash: `.copied` class for ~700 ms (CSS flash is 650 ms).
  *Old: CopyPaste-8ebg.55.*
- Row memo comparator is explicit and field-by-field (entry mutable fields +
  per-row display state + display settings + drag state); handler identities are
  deliberately ignored.

#### 3.1.6 Pinning & drag-to-reorder (`pin_order`)

- Pin toggle → immediate refetch so the server's re-sort is reflected.
- Only **pinned** rows are draggable. Drop target and above/below position are
  computed from the cursor's Y relative to the row's vertical midpoint.
- Reorder is optimistic: pinned items reordered locally, then
  `reorder_pinned(newIds)`. On failure: toast + refetch server state (revert).
- Drops are only accepted while a drag originating inside the pinned section is
  active.
- Bulk pin/unpin is a single toggle whose label reflects whether **every**
  selected item is already pinned (CopyPaste-8ebg.55).

#### 3.1.7 Search, filter, sort

- Client-side **fuzzy** subsequence match over the preview, scored; results
  sorted by score descending (stable, so daemon recency order is preserved within
  equal scores).
- Daemon **FTS** runs in parallel, debounced **250 ms**, limit **500**. FTS-only
  hits (found by the daemon but not by client fuzzy) are included at score 0, so
  they rank after scored fuzzy matches. FTS failure degrades silently to the
  client filter.
- Device filter dropdown appears only when **more than one** origin device has
  been seen. Labels prefer the daemon-supplied `origin_device_name`, then a
  UUID-prefix fallback; own device reads `"This device"`.
- Sort toggle (also only shown with >1 device): recency ↔ group-by-device. Device
  grouping puts own device first, then alphabetical by id, preserving recency
  within a group. **Grouping is skipped while a search is active** so relevance
  ranking is not discarded.
- The sort toggle persists to the `sortByDevice` pref so Settings › Display stays
  in sync.
- Toolbar count badge: shows the **daemon total** when unfiltered; switches to
  the **filtered/visible** count whenever a search or device filter is active
  (otherwise a zero-match search still read "14 items"). Hidden until the first
  page resolves. *Old: `historyBadge.ts` (CopyPaste-g27b.37).*
- A display-limit hint (`aria-live="polite"`) appears when the
  `historyDisplayLimit` pref caps the rendered list:
  `Showing first {limit} of {n} results — adjust the display limit in Settings › Storage`.
  Sentinel **100000** means "Unlimited".

#### 3.1.8 Delete + undo

- Delete is optimistic and deferred: the row is removed immediately; the actual
  IPC fires after **5000 ms**.
- A second delete within the window commits the first immediately.
- Undo cancels the timer, invalidates the signature, and refetches.
- On unmount, any pending deferred delete is **committed immediately** — items
  must not be silently left undeleted.
- Undo toast: `role="status" aria-live="polite"`, preview truncated to 40 chars,
  and must render **below** the details modal in z-order (SCRH-12).

#### 3.1.9 Bulk actions

- Selection mode activates on first checkbox toggle and auto-exits when the last
  item is deselected (via an effect on set size — a microtask hack raced with
  concurrent selection).
- Bulk delete requires an explicit confirm modal (no undo for bulk).
- Bulk copy: copies the **first** selected item via the daemon (that's what lands
  on the pasteboard), then best-effort writes the newline-joined previews of all
  selected non-sensitive, non-image items to the browser clipboard. Selection
  order follows the on-screen (filtered) order.
- Partial failures report `Deleted 3/5 (2 failed)` style messaging.
- Busy flag always released (INV-30).

#### 3.1.10 File add & OS drag-drop

- Hidden multi-file `<input type="file">` behind an "Add file" icon button.
  Reads bytes via the File API and calls `add_file_item`. The input is reset so
  the same file can be re-picked.
- OS drag-drop via the webview drag-drop event: `enter` shows a dashed drop-zone
  overlay ("Drop to add to clipboard", `aria-hidden`, pointer-events none),
  `leave`/`cancel` hide it, `drop` ingests each path. Per-file failures toast
  individually; a summary toast reports `Added N files (M failed)`.
- Tauri-only; silently absent in a plain browser (the file picker still works).

#### 3.1.11 History states & copy

| State | Title | Body | Action |
|---|---|---|---|
| loading | `Loading…` | `Fetching your clipboard history.` | — |
| offline | `Clipboard service offline` | `The background service is not running.` | Restart button |
| not_ready | `Starting up…` | `The clipboard service is initialising. History will appear in a moment.` | — |
| error (generic) | `Failed to load history` | friendly error, else `The background service returned an error.` | `Restart background service` |
| error (degraded) | `Clipboard database can't be opened` | `The local database could not be decrypted (its key no longer matches). Reset it to recover — this permanently erases this device's clipboard history.` | `Reset database (erases local history)` → confirm modal |
| empty (private mode on) | `Private mode is on` | `Clipboard is not recorded while private mode is active.` | — |
| empty | `Nothing copied yet` | `Copy something and it will appear here.` | — |
| no search results | `No results for "<query>"` | `Try a different search term.` | — |

Confirm modals (exact copy):
- **Bulk delete** — title `Delete N item(s)?`, body `This will permanently remove the selected clipboard items. This action cannot be undone.`, confirm `Delete`.
- **Reset database** — title `Reset clipboard database?`, body `This will permanently erase all clipboard history on this device and recreate a fresh database. This cannot be undone.`, confirm `Erase and reset`.
- **Clear all** — title `Clear all clipboard history?`, body `This will permanently delete all clipboard items on this device. This cannot be undone.`, confirm `Clear all`.
  The "Clear all" toolbar button only renders when `totalCount > 0`.

Reset-database success path: clear degraded/error state, clear selection, empty
the item list, **drop the image thumbnail cache**, invalidate the signature,
toast `Database reset — local history erased`, then reload.

---

### 3.2 Devices

*Old: `src/views/DevicesView/**`.*

#### 3.2.1 Structure

1. Header actions: `Pair a new device` (opens QR modal) and `Revoke all`
   (danger; disabled while pending, while not ready, or when there are no peers).
2. "Paired devices" section header with an online count.
3. Unified device list: **this Mac first**, then peers.
4. "Discovered on your network" section with an always-present `Refresh` button
   (HB-9 — a manual rescan must be reachable even when passive discovery has
   found nothing).

#### 3.2.2 Polling

- `list_peers`: **10 000 ms**.
- own-device info: **10 000 ms** (Public IP resolves asynchronously via STUN;
  local IP changes on network switch).
- discovered devices: **3 000 ms**.
- peer presence event drain: **5 000 ms** active / **30 000 ms** idle (app-global,
  see §5).
- 1 Hz clock tick purely so "last seen Xm ago" advances between 10 s polls; the
  displayed elapsed time is `daemon snapshot + (now − fetchedAt)`.

#### 3.2.3 Presence resolution (tri-state)

`presenceOnline[fp]` is:
- `true` — explicit `connected` event within TTL,
- `false` — explicit `disconnected` event,
- **absent** — unknown or expired.

Consumers MUST fall back to the daemon's `list_peers` truth when absent:
`live !== undefined ? live : peer.online === true`. Expired `true` entries are
**deleted** (not set to false) precisely so this fallback engages — the earlier
behaviour lied "Offline". *Old: CopyPaste-5917.11, SCRD-3/SYNC-5.*

On a detected daemon restart (previous poll failed, this one succeeded) all
presence entries reset to offline immediately rather than waiting for TTL.

#### 3.2.4 Peer list hygiene

- De-duplicate by fingerprint client-side (the daemon-side fix may not be
  deployed).
- Filter out this device's own fingerprint (read through a ref so it is never a
  stale closure).
- Per-row transient state (`pending` / `error` / `revokedAt`) survives reloads
  but is discarded for fingerprints no longer present.

#### 3.2.5 Device states & copy

| State | Title | Body |
|---|---|---|
| loading | — | centered spinner, `aria-busy="true"`, `aria-label="Loading devices…"` |
| offline | `Clipboard service offline` | `The clipboard service is not running.` + Restart |
| not_ready | `Starting…` | `CopyPaste is starting up. Your devices will appear in a moment.` |
| degraded | `Database degraded` | `Device list unavailable. Reset the database in History to recover.` + Restart |
| error | `Failed to load devices` | `Try restarting the clipboard service.` + Restart |
| no peers | `No other devices paired` | `Pair your phone or another Mac to sync your clipboard — end-to-end encrypted.` |
| no discovered | — | `No devices found on the network yet.` |

Loading indicators must be **visible**: the old code shipped classless empty
`<span>`/`<div>` elements that rendered as nothing, indistinguishable from a
layout bug (CopyPaste-8ebg.29, bdac.2).

Canonical user-facing vocabulary: **"clipboard service" / "background service"**.
Never "daemon" (bdac.34/36). American spelling ("initializing") in newer strings.

#### 3.2.6 Revoke / unpair

- **Unpair** — plain removal, per-row pending state.
- **Revoke** — P2P-only (mTLS allowlist + denylist). Does *not* cut off
  cloud/relay.
- **Revoke & rotate** — revoke plus sync-key rotation so the device is also cut
  off from cloud/relay. Requires a new passphrase; the daemon derives the new key
  **first**, so a too-short passphrase fails before anything is revoked.
  Success toast: `Revoked & rotated sync key — re-provision remaining devices`
  (5 s).
- **Revoke all** — confirm modal, title `Revoke all paired devices?`, body
  `This will immediately break trust with all paired devices.` /
  `All devices will need to re-pair before syncing can resume.`, confirm
  `Revoke all`. Success toast `Revoked N device(s)` (3 s).
- Unmount guard prevents setState after navigating away mid-request.

---

### 3.3 Pairing — QR modal and SAS modal

> **Current product decision (ADR-0015).** The v2 backend currently persists a
> pairing before it can bind either device's decision to a SAS and has no safe
> abort state machine. Until that API exists, Pair/Add-device controls are not
> rendered on any platform. The Devices view still manages known devices. The
> QR/SAS requirements below are the re-entry contract and must all be met before
> the controls return; a blurred QR, local-only SAS, or clipboard copy of the
> long-lived credential is not an acceptable partial implementation.

#### 3.3.1 QR pairing modal

*Old: `DevicesView/index.tsx` + `hooks/useQrCode.ts`.*

- Title `Pair a new device`; close button labelled `Close`.
- QR is generated eagerly and **rendered blurred by default** behind a
  `Click to reveal` overlay button (`aria-label="Click to reveal QR code"`).
  Privacy-first (spec §10 / CopyPaste-1jms.2).
- Blur state is **independent** of generation: regenerating does **not** clear
  the blur. `Regenerate` explicitly **re-blurs first** — a fresh PAKE token is a
  new credential and must not be visible without re-confirmation
  (CopyPaste-crh3.21).
- TTL: `QR_TTL_SECS = 120` (daemon `PAKE_SESSION_TTL`); refresh margin
  `QR_REFRESH_MARGIN_SECS = 15`. The countdown ticks at 1 Hz, and when
  `remaining <= 15` the countdown is **zeroed immediately** and a regenerate
  fires — the daemon replaces the pending token the moment generation resolves,
  so showing "15" would be misleading (CopyPaste-1jms.7).
- The drain bar's basis is the token's actual `expires_in_secs` (or
  `QR_TTL_SECS`), never a hardcoded literal — the old code divided by a stale
  `300`, so the bar started at ~40% and never reached 100%
  (CopyPaste-8ebg.15).
- Concurrency: an in-flight guard drops duplicate generate calls (auto-refresh
  racing a manual click would waste single-use tokens).
- Visibility-gated per INV-28.
- QR SVG is injected as raw markup; it originates from our own backend. The
  frame keeps a white background so the code is always scannable.
- Copy: `Expires in Ns`, `Scan from CopyPaste on another device to pair automatically.`
- Error copy (never the raw error, INV-12):
  `Could not generate pairing code. Make sure the clipboard service is running and try again.`
- Idle/loading copy: `Generating pairing code…` / `Generating…` — static text, no
  pulse animation (MOT-21).

#### 3.3.2 SAS pairing modal

*Old: `DevicesView/SasPairingModal.tsx`.*

**Roles.** Two entry points, never both at once (initiator takes precedence):
- **Initiator** — user clicked Pair on a discovered device; starts from the
  modal's own default state (`initiating`). It must **never** be seeded from the
  responder payload (CopyPaste-8ebg.28).
- **Responder** — backend detected an inbound request and emitted
  `incoming-pairing` with `state="awaiting_sas", role="responder"`. The app
  switches to Devices and seeds the modal. Once consumed, the app-level payload
  MUST be cleared, or tabbing away and back re-opens a phantom modal from a
  finished episode (CopyPaste-8ebg.28).

**Polling & termination.** Poll `pair_get_sas` every **700 ms** until terminal.
Terminal states: `confirmed`, `rejected`, `aborted`, `timed_out`, plus the
synthetic `ended`.

The daemon's standing responder resets to `idle` right after any outcome, so a
trailing `idle` **observed after the handshake was seen active** is itself
terminal:
- if the local user already accepted → treat as **success** (`confirmed` +
  `onPaired`),
- otherwise → neutral **`ended`** state, copy `Pairing ended — check the other device.`

An `idle` seen *before* any active state is not terminal (the daemon simply
hasn't started) — keep polling.

**Watchdog.** `SAS_WATCHDOG_MS = 60 000`, matched to the daemon's own SAS
watchdog. It was 30 s, which reported "timed out" while the Match / Doesn't-match
buttons stayed live and functional for another 30 s (CopyPaste-8ebg.52). When
the watchdog fires, the SAS digits and the decision buttons MUST be hidden —
gating every non-terminal branch on `error === null` (CopyPaste-8ebg.30 /
bdac.9).

**What the user must see (security).**
1. Title: `"<peer>" wants to pair` (responder) or `Pair "<peer>"` (initiator).
2. Instruction: `Confirm this code matches the one shown on the other device.`
3. The **6 SAS digits**, one glyph per element, `aria-label="Security code: <digits>"`,
   display-only (INV-14).
4. Peer metadata, explicitly labelled unverified (INV-15).
5. Two decision buttons: `Doesn't match` (secondary) and `Match` (primary,
   `aria-label="Codes match — confirm pairing"`). While in flight the label
   becomes `Confirming…` with a spinner, and both are disabled.
6. A `Cancel` affordance in every non-terminal state.
7. Never an empty body: a pre-handshake placeholder shows
   `Waiting for the other device…` with a spinner (audit P2).

**Decision handling.**
- Local accept is recorded **before** the IPC so a trailing `idle` arriving
  before `confirmed` is still read as success. If the IPC throws, that optimistic
  flag is **undone**.
- Reject → `pair_confirm_sas(false)` then `pair_abort` then close.
- Unmount guard prevents setState after close.

**Terminal copy.** `Paired ✓` · `Pairing timed out.` · `Pairing was rejected.` ·
`Pairing was cancelled.` · `Pairing ended — check the other device.` ·
Watchdog error: `Pairing timed out. Check that both devices are on the same network and try again.`

**Close.** Always `pair_abort` (INV-16). After close, refresh **both** the paired
and discovered lists (a freshly paired device must move sections).

**Start-pairing errors.** `rate_limited` →
`Another pairing is already in progress.`; otherwise the friendly error.

---

### 3.4 Settings

*Old: `src/views/SettingsView.tsx`, `SettingsView/**`.*

#### 3.4.1 Shape

Seven tabs: `general`, `display`, `sync`, `shortcuts` (desktop-only; never
rendered on Android), `storage`,
`about`, `logs`. Panes are `role="tabpanel"` with `id="tabpanel-<id>"` /
`aria-labelledby="tab-<id>"`. The tab row **wraps** at the app's 720 px minimum
width — it previously overflowed behind a hidden scrollbar and "Logs" was
entirely off-screen (CopyPaste-g27b.31).

Tabs unmount when inactive. The **logs filter is lifted to the view** so it
survives a tab switch (CopyPaste-8ebg.54) — a documented partial fix; the
general "keep all tabs mounted" change was deferred.

#### 3.4.2 Loading & error banners

Everything loads in one batched pass (`get_private_mode`, `get_config`,
`get_sync_status`, `status`, app version, `list_peers` — one round of
`Promise.all`, each individually `.catch(() => null)`), plus three Tauri-direct
calls that work even when the daemon is down: current shortcut, default shortcut,
allow-screenshots.

State resolution:
- transport `daemon_offline` → `offline`
- `ipc_not_ready` → `not_ready`
- daemon answered but calls failed → probe `status`: degraded → `degraded`,
  else → `error`
- `status` reports `degraded === true` or `ready === false` → `degraded`

Banner copy (`StatusBanners.tsx`) — all with a single recovery action (#21):
- **stale daemon** (`role="alert"`, non-dismissible):
  `A previous CopyPaste background service is still running after an update (build X). Restart it to use the latest version.` + Restart.
- **not ready** (`role="status"`, info):
  `Clipboard service is starting up — settings will be available in a moment.` + `Retry`.
- **offline** (`role="alert"`, error):
  `Background service not running — clipboard sync paused.` + `Restart service`.
- **degraded** (`role="alert"`, error):
  `Clipboard database unavailable (reason) — its key no longer matches. Open History to reset the database and recover.` + `Restart service`.
- **error** (`role="alert"`, error):
  `Failed to load settings — the background service is running but returned an error.` + `Restart service`.

Retry = bump a reload key, which re-runs the whole load.

#### 3.4.3 Save semantics

- Every toggle is optimistic with a **field-scoped** revert on failure (INV-29).
- Config patches are built from **live component state** for all fields plus the
  override, so a toggle can never clobber unsaved slider/credential values. The
  toggle handlers are deliberately **not memoised** for exactly this reason.
- Slider values are snapped to the nearest step-array entry both on load and on
  change, so an arbitrary pre-existing config value always loads cleanly.
- Feedback is per-field, typed `{ ok, message }` — never inferred from a string
  comparison against `"Saved"` (bdac.106, crh3.51).
- Settings confirmations route to a **bottom-right toast** (see §3.7), not inline
  text.

#### 3.4.4 Restart-on-save paths

Supabase URL/key/relay and `p2p_enabled` are read only at daemon startup, so
saving them triggers a daemon restart:
- the control is disabled while the restart is in flight (prevents queuing two),
- transient status `Restarting sync service…` → `Sync service restarted`,
- **a failed restart is NOT non-fatal** — it must clear the "Saved" state and
  surface an error, because the daemon keeps running with the old credentials and
  sync breaks silently (CopyPaste-8ebg.19).
- `Test connection` first saves; if the save failed it aborts with
  `Fix the save error above, then test again.` rather than testing the stale
  config (CopyPaste-crh3.50).

#### 3.4.5 Write-only credentials

Supabase email/password are write-only: the daemon never returns them, the UI
shows only presence flags, the inputs are **cleared after a successful save**,
and an empty field is **omitted** from the patch (sending `null` would erase the
stored value). Same rule for `supabase_anon_key`: blank + already-set → omit.
Email and password are trimmed before sending (leading/trailing whitespace
otherwise caused silent auth failures).

#### 3.4.6 Private mode

Optimistic toggle → daemon echoes the confirmed value → UI uses the **echo**, not
the assumption → broadcast `private-mode-changed` so the tray converges. On
failure: revert and show the error for 3500 ms. Re-fetch on window focus and on
visibility change; also listen for the broadcast (INV-39).

#### 3.4.7 Backup / restore

- **Export**: `export_items(includeSensitive)` → pretty JSON → Blob + temporary
  `<a download>` named `copypaste-backup-YYYY-MM-DD.json`; object URL revoked
  after 10 s. Toast `Exported N item(s)`.
- **Import**: FileReader → JSON parse → must be `{ "items": [...] }`. Errors:
  `Invalid JSON — file may be corrupted or wrong format`,
  `Invalid backup file — expected { "items": [...] }`, `No items in backup file`.
  Parsed items are held pending an explicit confirm modal — the live database is
  never touched before confirmation (vcnv). Modal copy:
  `Import clipboard history?` /
  `This will import N item(s) from the file into your clipboard history. Duplicate items will be skipped. Existing items are not deleted.` / confirm `Import`.
  On success: `Imported N item(s), M skipped (duplicates)` **and** emit
  `history-refresh` so History updates immediately.
- The file input is reset after every selection so the same file can be retried.

#### 3.4.8 Display tab (appearance + list prefs)

- **Theme** — segmented `System / Dark / Light`, `role="group"`,
  `aria-pressed` per option. When `System` is selected, a live hint reads
  `Currently resolves to Dark|Light.`, updated from a live `matchMedia`
  subscription (CopyPaste-8ebg.63).
- **Accent** — 6 swatches in a `role="group"` labelled `Accent`.
- **Translucency** — toggle.
- **Preview lines (app)** — slider 1–6, formatted `N line(s)` (a bare number was
  meaningless — CopyPaste-8ebg.63).
- **Image preview height** — slider 1–200, formatted `Npx`.
- **Group by device** — toggle (mirrors the History toolbar sort toggle).
- **Warn before revealing sensitive items** — toggle, default on (Android parity).
- **Mask sensitive data** — toggle, default on.
- **Preview lines (popup)** — slider 1–6, independent of the app setting.

#### 3.4.9 Shortcuts tab

See §7 for capture rules. Save is a no-op when unchanged. Success:
`Saved` (2.5 s) / `Reset to default` (2.5 s). Failure: message + revert pending
to current (4 s). The default accelerator is fetched **from Rust**
(`get_default_popup_shortcut`) so the reset button can never drift from the Rust
constant (CopyPaste-sqw0). Rust constant is `CmdOrCtrl+Shift+V`, cross-checked by
a Rust test that names the TS file.

#### 3.4.10 Logs tab

Live tail refresh every **3000 ms**. Toolbar wraps rather than pushing
Refresh/Export off-screen; the level badge has a fixed width so message text
aligns (CopyPaste-g27b.31).

---

### 3.5 Quick-Paste popup

*Old: `src/popup/**` + `src-tauri/src/popup/**`.*

#### 3.5.1 Window behaviour

- Frameless, transparent, always-on-top, skip-taskbar, non-resizable, created
  **hidden**. Logical size **403 × 624**.
- **Lazy-created on first hotkey press**, not at app launch — saves ~84 MB idle
  RSS. Once created the WebView stays warm; only the JS heap is freed on hide.
- macOS vibrancy: `HudWindow`, active state, radius 12.
- Content protection applied at creation unless "Allow screenshots" is on.
- **Positioning** (`PopupPosition`), all arithmetic in physical pixels, always
  clamped to the target monitor's frame so the popup is never partly off-screen:
  - `Cursor` — cursor + 8 px offset, on whichever monitor physically contains the
    cursor (iterate all monitors; handles negative coords on secondary displays),
    falling back to primary.
  - `Center` — centred on the primary monitor.
  - `Menubar` — top-right of the primary monitor, `24 pt` menu-bar height + `4 pt`
    gap below, `8 pt` right inset.
- **Toggle**: visible → hide via the shared hide path; hidden → position, record
  the current frontmost app bundle id, show, focus.
- **Blur-to-close**: on `Focused(false)`, hide — but skip if already hidden, so a
  JS-initiated hide racing the blur event doesn't double-activate the prior app
  and flicker focus (V-12). A child/system dialog stealing focus is the known
  false-positive case this guard also mitigates.
- **Hide path** (INV-25): activate the recorded prior app; if none recorded,
  temporarily switch to `Accessory` activation policy, hide, restore `Regular`
  (invisible to the user because the popup is still on screen during the switch).
- After hiding, the backend calls `window.__copypasteFreeMemory()` in the WebView
  to drop the image LRU and the item list, without navigating away from
  `popup.html` (which would force a full bundle re-parse on next show).
- JS-side hide is debounced by a guard flag reset after **100 ms** so a
  concurrent blur + row-click can't both trigger it.

#### 3.5.2 Popup data

- Fetch `history_page(limit=50, offset=0)`. `page.total` is surfaced so the cap
  is visible (`50 of 214`) rather than silently truncating (CopyPaste-8ebg.56).
- Refresh triggers: mount, window focus-changed→focused, **3000 ms**
  visibility-gated poll, manual retry from the offline empty state.
- All four are sequence-tagged (INV-33).
- On focus, prefs are **re-read from storage** — the popup's JS realm is built
  once and never re-evaluates, so a Settings change in the main window would
  otherwise never reach it (task 1.17).
- Appearance is re-applied on every prefs change, same as the main window.

#### 3.5.3 Popup interaction

| Key | Behaviour |
|---|---|
| `↓` / `↑` | Move selection, **wrapping** (modulo) — unlike the History list. |
| `Enter` | Copy + hide + paste to frontmost. |
| `⌥Enter` | Copy + hide + paste as **plain text**. |
| `Escape` | Hide. |
| `⌘1`–`⌘9` | Paste the Nth item directly — **only when no search query is active**. |

- The key handler is attached to the **popup root**, not the search input, so
  clicking Pin or tabbing to another control does not dead-end the keys
  (CopyPaste-8ebg.10).
- Selection by id (INV-32); index re-resolved each render; falls back to index 0
  when the selected item disappears.
- Search input auto-focuses on mount and **50 ms** after the window is shown
  (native activation and React render are not synchronous; focusing too early
  silently no-ops on macOS).
- **Hover suppression**: mouse-enter does not steal selection within **250 ms**
  of a keyboard-nav event (the pointer can end up over a different row after
  `scrollIntoView` moves the list under a stationary cursor) — mirrors
  Raycast/Alfred (CopyPaste-8ebg.36).
- **Scroll-momentum suppression**: mouse-enter is ignored while the list is
  scrolling; "scrolling" ends **120 ms** after the last scroll event. The glide
  highlight also freezes/hides during scroll (zuzu).
- Blur on the popup root (focus leaving the subtree) hides the popup.
- Result counter (`aria-live="polite"`): `N of M` while searching, `items of total`
  when capped, else the count. `…` while loading.
- Pin failures surface an error (previously console-only — CopyPaste-crh3.110).
- Footer hint pills: `↑↓ navigate`, `⌘1-9 quick paste` (hidden while searching,
  because the shortcut is inactive then), `⌥⏎ plain text`, `⏎ paste`, `Esc close`,
  and a gear button (`Open settings`) which hides the popup, shows+focuses the
  main window, and emits `open-settings`.
- Filtering is fuzzy over a **display label** — `[Image]` for images, `••••••••`
  for sensitive items, span-masked text when applicable — so masked content is
  never matched against its plaintext.
- Empty/error states: while `loading` with an empty list, render a **blank list
  area**, not "Nothing copied yet" — items are cleared on hide and refetched on
  show, so the misleading message used to flash on every open
  (CopyPaste-8ebg.37). Otherwise: `No matches for "<q>"` / `Nothing copied yet` /
  `Clipboard service offline` (+ Restart) / `Starting up…` /
  `Something went wrong`.

---

### 3.6 Menu-bar tray

*Old: `src-tauri/src/tray.rs`.*

- Template icon (monochrome, adapts to menu-bar appearance), menu opens on left
  click.
- Menu: `Open CopyPaste`, `Recent ▸`, `Private Mode` (check item), separator,
  `Quit CopyPaste`.
- **Tray setup must not block on IPC** — it runs synchronously on the main thread
  during startup, so the Recent submenu is built with a disabled
  `No recent items` placeholder and Private Mode defaults unchecked; background
  threads fill in the truth (CopyPaste-8ebg.23).
- **Private-mode resync**: poll every **250 ms** up to **30 s**, and require
  **two consecutive identical** successful replies before settling — the first
  reply can come from a dying old daemon during eviction. Only write the
  checkmark when the value actually differs (avoids visible flicker).
- **Recent resync**: wait for daemon readiness (250 ms poll, 30 s give-up), then
  rebuild every **5 s**. Up to 10 items, labels truncated to **40 characters**
  with interior newlines/tabs collapsed to spaces and a trailing `…`. Falls back
  to the placeholder when empty/offline. A stop flag set on app exit lets the
  thread exit cleanly instead of holding the AppHandle.
- Copying from the Recent submenu fires the **same** sound + rich notification as
  a row-click copy (audit P1 / M12 parity).
- The same 5 s loop watches the newest item's `wall_time` and fires a rich
  notification for background captures (respecting the daemon's `notify_on_copy`),
  seeded on startup so it never fires for pre-existing items.
- Private-mode toggle behaviour per INV-37/38/39.

### 3.7 Toasts

*Old: `src/components/Toast.tsx`.*

- Stack at **bottom-right** — not bottom-centre: the persistent left sidebar and
  its footer chip occupy the bottom band and centring bled into it at narrow
  widths (CopyPaste-7w060.2).
- The stack container positions the group; individual toasts flow inside it with
  `column-reverse` so the newest is closest to the screen edge. Previously each
  toast self-positioned to the same fixed spot and rendered exactly on top of the
  others (CopyPaste-8ebg.38).
- Default duration **3000 ms** (History uses 2500 ms for its own calls).
- **Auto-dismiss pauses on hover and on focus within the toast**, so a toast
  can't vanish mid-read or while the user is tabbing to its dismiss button
  (CopyPaste-8ebg.55).
- Each toast: `role="status" aria-live="polite"`, a leading severity dot
  (`info | success | warning | error`), the text, and a `Dismiss` icon button.
- Used by History, Devices, and Settings; each mounts its own provider.
- Toasts must never occlude an open dialog (z-order: scrim 40 < toast 60 <
  popover 80; the History undo toast sits at 40, below the details modal).

### 3.8 Sync status chip (sidebar footer)

*Old: `src/components/SyncStatusChip.tsx`.*

- State comes from the daemon's canonical `badge_state` when present
  (`synced | syncing | idle | misconfigured | offline | error`); a deprecated
  fallback derivation exists only for daemons predating it.
- Status poll **2000 ms**; peer count on its own **10 000 ms** poll.
- A sync-status IPC rejection means the socket is down → state `offline` (as
  distinct from `error`, which is a daemon-reported backend failure).
- Tooltip is a `·`-joined list: last sync (or `Background service unreachable`
  for offline/error, or `No sync yet`), paired device count, account email, and
  `N peers not syncing` when any peer is stalled.
- Accessible name: `Sync: <state>. <tooltip>`.
- **Per-peer stall pill** (the thing the global badge cannot express — the badge
  can read "synced" while one peer silently receives nothing): a peer is stalled
  when `rekey_failures > 0` (immediate, regardless of recency), or it has synced
  before and `last_sync_at` is older than **30 min**, or it has never synced and
  was paired more than 30 min ago. Freshly-paired peers are never flagged on the
  time checks.
- A one-shot pulse animation plays on transition **into** any green state
  (`synced` or `syncing`).
- Separate `Cloud sync misconfigured` indicator when the Supabase URL is set but
  the configuration is incomplete.

---

## 4. Accessibility requirements (concrete)

**A11Y-1 — List semantics.** The virtual list is `role="list"` with
`aria-label="Clipboard history"`, `tabIndex={0}`, and rows are `role="listitem"`.
Selection is exposed via `aria-current="true"`. Do **not** use listbox/option
(INV-8). The roving active id is exposed as a `data-active-descendant` attribute
for tests/debugging, cleared when the row is not rendered.

**A11Y-2 — Live announcements.** A visually-hidden
`aria-live="polite" aria-atomic="true"` sibling of the list mirrors the active
row's `aria-label` on every selection change (INV-9). The popup's result counter
is also `aria-live="polite"`. The History display-limit hint is `aria-live="polite"`.

**A11Y-3 — Masked content.** Per INV-10. The reveal affordance is a real button
with `aria-label="Sensitive content hidden — activate to reveal"`; the blurred
text node itself is `aria-hidden`.

**A11Y-4 — Dialogs.** Portal to `document.body`;
`role="dialog"` + `aria-modal="true"` + `aria-labelledby` (and
`aria-describedby` when a body id is supplied). Focus moves to the first
focusable descendant, or the container with `tabindex="-1"` if none.
Tab/Shift+Tab cycle within the panel. Escape and backdrop click both dismiss by
default, each independently disableable. **Focus is restored to the previously
focused element on unmount.** Body scroll-lock is ref-counted (INV-19).

**A11Y-5 — Banners.** Severity-appropriate roles: `role="alert"` for warnings and
errors; `role="status"` for informational and success/confirmation banners. The
accessibility-permission warning is `role="alert" aria-live="assertive"` (urgent
enough to interrupt); its granted-confirmation counterpart is
`role="status" aria-live="polite"` with
`aria-label="Accessibility permission granted — closing shortly"` so AT users
know it is transient.

**A11Y-6 — Tablist.** `role="tablist"` + `role="tab"` + `aria-selected`, panes
`role="tabpanel"` with `id`/`aria-labelledby` pairing. Arrow keys move selection
(Left/Right horizontal, Up/Down vertical), Home/End jump to the bounds, and
navigation **wraps** at both ends. React state is the source of truth.

**A11Y-7 — Disclosures.** `aria-expanded` + `aria-controls` on the header button.

**A11Y-8 — Toggle buttons.** Any button acting as a toggle exposes `aria-pressed`
(select-all, bulk pin/unpin, theme segmented control).

**A11Y-9 — Every icon-only control needs a name.** Decorative icons are
`aria-hidden="true"`; the control carries `aria-label` **and** a matching `title`
for pointer users.

**A11Y-10 — Contrast.** WCAG AA (≥4.5:1) for all text. This forced a set of
`*-strong` foreground-only token variants (see §8.3) where the base hue is
correct as a fill/dot but fails as small text on a tint. `--faint` was
deliberately lifted (dark) / darkened (light) because it labels real meta text.
`--mute` is decorative-only and must never be the sole carrier of text.

**A11Y-11 — Reduced motion.** `prefers-reduced-motion: reduce` zeroes the motion
duration tokens and clamps all animations to ~0 ms, globally, at the token layer.

**A11Y-12 — Reduced transparency.** `prefers-reduced-transparency: reduce` forces
every chrome surface solid, overriding the translucency preference.

**A11Y-13 — Shortcut control.** The shortcut capture control announces the
**currently bound accelerator** in its accessible name
(`Current shortcut: CmdOrCtrl+Shift+V. Click and press a new key combination.`),
using the raw accelerator string, not the glyph rendering — screen readers handle
`CmdOrCtrl+Shift+V` far better than `⌘⇧V` (CopyPaste-8ebg.53). When capturing:
`Press a key combination`. When unset: `No shortcut set. Click and press a key combination.`

**A11Y-14 — Live regions must not break required-children rules.** A live region
inside `role="list"` counts as content and fails `aria-required-children`; keep
announcers as siblings (CopyPaste-wrfn).

**A11Y-15 — Responsive minimum.** Everything must remain reachable at the app's
**720 × 460** minimum. Tab rows, link rows, and toolbars **wrap**; they must not
hide overflow behind a scrollbar-less scroller. Long/localised text truncates or
wraps per its own overflow rule — never rely on a fixed English string width.

---

## 5. Constants & tunables

### 5.1 Polling / refresh

| Constant | Value | Where | Rationale |
|---|---|---|---|
| History active poll | **3000 ms** | `useHistoryData` | Slowed from 1200 ms: 50→20 IPC calls/min with no perceptible UX change. New captures still appear within one window. (s7ia B1) |
| History backoff poll | **5000 ms** | `useHistoryData` | Used for `offline`/`error`/`not_ready` — don't hammer a dead or initialising daemon. |
| Popup poll | **3000 ms** | `usePopupHistory` | Matches history; visibility-gated. |
| Logs live tail | **3000 ms** | `LogView` | |
| Peer presence (active) | **5000 ms** | `peerPresence` | Backed down from 1 s. Peers reconnect fast enough at 5 s. (s7ia B3) |
| Peer presence (idle) | **30 000 ms** | `peerPresence` | When no peers are known, 1 s fired 60 IPC calls/min for nothing. |
| Peer presence TTL | **15 000 ms** | `peerPresence` | 3× the active interval — one missed tick must not flip the dot, but a daemon restart flips within ~15 s. |
| `list_peers` | **10 000 ms** | `usePairedDevices` | Online dot refresh without user interaction. |
| Own-device info | **10 000 ms** | `useOwnDevice` | Public IP resolves async via STUN; local IP changes on network switch. |
| Discovered devices | **3000 ms** | `useDiscoveredDevices` | Fast enough to surface a new LAN peer within seconds; tied to the mDNS announcement cadence. |
| Devices "ago" clock | **1000 ms** | `DevicesView` | Display-only tick between 10 s polls. |
| SAS poll | **700 ms** | `SasPairingModal` | The SAS state machine transitions within a round-trip; responsive without hammering. |
| SAS watchdog | **60 000 ms** | `SasPairingModal` | Must match the daemon's watchdog. At 30 s the UI said "timed out" while the buttons were still live. |
| QR countdown tick | **1000 ms** | `useQrCode` | Visibility-gated. |
| Sync chip status poll | **2000 ms** | `SyncStatusChip` | Offline must be reflected within one cycle; 10 s showed a stale green chip too long (CopyPaste-f701). |
| Sync chip peer count | **10 000 ms** | `SyncStatusChip` | Decoupled from the 2 s poll — cut `list_peers` from 30/min to ≤6/min (CopyPaste-crh3.48). |
| Accessibility permission poll | **3000 ms** | `App` | Only runs while the banner would be shown; stops on grant. |
| Tray private-mode resync | **250 ms**, 30 s give-up, 2 confirmations | `tray.rs` | Avoids caching a stale reply from a dying old daemon. |
| Tray Recent rebuild | **5000 ms** (250 ms while waiting, 30 s give-up) | `tray.rs` | Cheap 1-item probe; also drives background-capture notifications. |

> **Cross-language drift warning:** several of these mirror constants in the
> daemon / p2p crates with no shared source of truth. Keep them in sync manually.
> (`CopyPaste-x09o` tracks this.)

### 5.2 Layout / virtualization

| Constant | Value | Rationale |
|---|---|---|
| `PAGE_SIZE` | 200 | Initial + subsequent page size; server clamps. |
| `LOAD_MORE_THRESHOLD_PX` | 300 | Distance from bottom that triggers load-more. |
| `OVERSCAN_PX` | 240 | Rows rendered above/below the viewport. |
| `ROW_PAD_V` | 18 | Constant row vertical padding; the floor every height must clear. |
| `TITLE_LINE_PX` | 21 | `--fs-base` 14px × `--lh-normal` 1.5. |
| `SINGLE_LINE_FLOOR` | 59 | 21 + 2 + 18 + 18; guarantees a strictly positive gap to the next row. |
| File row height | 44 | Fits the FileChip. |
| Image row min | 34 | |
| Popup `MAX_ITEMS` | 50 | Quick-access surface, not a full browser. |
| Popup logical size | 403 × 624 | v0.5.3; must match `tauri.conf.json`. |
| Main window | 980 × 640, min 720 × 460 | |
| Menu-bar offsets | 24 pt bar + 4 pt gap, 8 pt right inset, 8 px cursor offset | |

### 5.3 Timings

| Constant | Value | Rationale |
|---|---|---|
| Undo-delete window | **5000 ms** | Deferred delete; committed early on a second delete or on unmount. |
| Copy flash | **700 ms** | Comfortably longer than the 650 ms CSS flash. |
| Sensitive auto re-blur | **10 000 ms** | Unattended screens must not stay exposed. |
| FTS debounce | **250 ms** | |
| FTS limit | **500** | |
| Popup focus delay | **50 ms** | macOS activation vs. React render race. |
| Popup hover suppression | **250 ms** after keyboard nav | |
| Popup scroll-idle | **120 ms** | |
| Popup hide-guard reset | **100 ms** | |
| Accessibility "granted" confirmation | **3000 ms**, fade starting at 2500 ms | Visual ephemerality cue (CopyPaste-5917.103). |
| Toast default | **3000 ms** (History: 2500 ms) | |
| Settings feedback | 2500 ms success / 3500–5000 ms error | Errors linger longer. |
| Migration retry backoff | 250 / 500 / 1000 / 2000 / 2000 ms, max 5 | INV-34. |

### 5.4 Budgets & caps

| Constant | Value | Rationale |
|---|---|---|
| Image data-URI LRU budget | **16 MiB** | Trimmed from 24 MiB. The dominant cost is decoded bitmaps, now bounded by a 192 px thumbnail source + intrinsic decode hints; this is the secondary string-side bound. (HB-10) |
| Image cache eviction | LRU via Map insertion order, touch-on-read | Also de-duplicates in-flight fetches by item id. |
| `historyDisplayLimit` | default **1000**; steps 100/250/500/1000/2500/5000/10000/**100000**=Unlimited | UI-only render cap; the daemon may hold more. |
| Peer stall threshold | **30 min** | Much longer than the 5 min global badge threshold — brief blips must not spam a warning. A non-zero `rekey_failures` flags immediately regardless. |
| Tray Recent | 10 items, 40-char labels | |
| QR TTL / refresh margin | **120 s / 15 s** | Daemon `PAKE_SESSION_TTL`. |
| Protocol version | `CURRENT_PROTOCOL_VERSION = 1` | Any differing daemon `protocol_version` fires the mismatch handler. |

### 5.5 Token-burn rationale (why these numbers, not "just poll fast")

The old UI had **eight independent pollers** across two windows. Untuned, they
produced well over 150 IPC round-trips/minute at idle, each of which wakes the
daemon, touches SQLite, and on macOS keeps the CPU out of deep idle. The fixes,
in priority order, and all of which the rewrite must preserve:

1. **Visibility gating** — a hidden window polls **zero** times (INV-27).
2. **Adaptive idle backoff** — presence drops 5 s → 30 s with no peers.
3. **Decoupling cadences** — the sync chip's 2 s status poll must not drag
   `list_peers` along at 2 s (30/min → ≤6/min).
4. **Cheap dedup before re-render** — the signature check makes an idle poll cost
   one IPC and *zero* React work; the 1-slot signature cache makes even the hash
   O(1) on the idle path (CopyPaste-44rq.35).
5. **Error backoff** — never poll a dead daemon at the healthy rate.
6. **Single-use token protection** — the QR tick is gated so a background window
   never burns PAKE tokens (INV-28).

React Query gives most of this for free (`refetchInterval` +
`refetchIntervalInBackground: false`, `structuralSharing`, per-query intervals),
but the *numbers* and the *idle backoff* are earned knowledge and must be carried
over explicitly. The signature-dedup requirement (INV-2) maps onto structural
sharing + a stable `select`, but must be verified, not assumed — INV-1 depends on
reference identity changing **only** on real content change.

---

## 6. Acceptance tests to re-create (given/when/then)

### Virtual list & scroll

**AT-1 — Anchor across a prepend.**
*Given* 500 items and the user scrolled so item #120 is at the viewport top,
*when* a poll prepends 1 new item, *then* item #120 remains at the viewport top
(±1 px) and no visible jump occurs.

**AT-2 — Anchor across a load-more append.**
*Given* the user scrolled near the bottom, *when* a page of 200 appends,
*then* the currently top-most row stays put and the new rows extend below.

**AT-3 — Anchor item deleted.**
*Given* the anchor row is the row being deleted, *when* the list mutates,
*then* the scroll position stays in bounds and no exception is thrown.

**AT-4 — Shrink clamp.**
*Given* the user is scrolled to the end of a tall list, *when* `previewLines` is
reduced (or a filter shrinks the list) such that total height < current
`scrollTop`, *then* both the tracked and the DOM scroll position clamp
immediately and the rendered window is a full viewport of rows — **without** any
further user scroll.

**AT-5 — Idle poll causes no re-render.**
*Given* an idle clipboard, *when* three poll cycles elapse, *then* the item array
identity is unchanged, no row re-renders, and the signature cache hits the fast
path.

**AT-6 — Mutation invalidates dedup.**
*Given* an item was deleted optimistically, *when* the next poll returns the
pre-delete list, *then* the UI re-renders with server truth (the stale signature
must not suppress it).

**AT-7 — Load-more survives the next poll.**
*Given* 3 pages loaded (600 items), *when* the next 3 s poll returns page 1,
*then* all 600 items remain.

**AT-8 — Height reservation never under-reserves.**
*Given* `previewLines = 6` and a very long clip at the **narrowest supported
window width (720 px)**, *then* the row's rendered height ≤ its reserved height,
and no row overlaps its neighbour.

**AT-9 — Image height cap.**
*Given* `imageMaxHeight = 40` and a tall portrait source image, *then* the
rendered thumbnail height is exactly ≤ 40 px and the row does not balloon.

### Keyboard & a11y

**AT-10 — Arrow navigation scrolls off-screen rows into view** and does not wrap
at either end (History). In the **popup**, arrow navigation **does** wrap.

**AT-11 — Active-descendant validity.**
*Given* the selected row is scrolled out of the rendered window, *then* the
active pointer attribute is absent (never a dangling id).

**AT-12 — Live announcement.**
*When* the selection moves via ArrowDown, *then* the polite live region's text
equals the newly-active row's accessible label.

**AT-13 — Masked row never leaks.**
*Given* a sensitive item with masking on, *then* neither the row's accessible
name nor its checkbox's accessible name contains any substring of the plaintext
preview.

**AT-14 — Reveal expires two ways.**
*Given* a revealed sensitive row, *when* the window blurs → it re-blurs
immediately; *and* given no blur, *when* 10 s elapse → it re-blurs.

**AT-15 — Dialog focus contract.**
*When* a dialog opens, focus is inside it; Tab from the last focusable wraps to
the first; Shift+Tab from the first wraps to the last; Escape closes; backdrop
click closes; *and on close focus returns to the element that opened it*.

**AT-16 — Nested dialog scroll-lock.**
*Given* two stacked dialogs, *when* the inner closes, *then* body scroll is still
locked; *when* the outer closes, *then* the original overflow is restored.

**AT-17 — Axe clean.** No `nested-interactive`, `aria-required-children`,
`aria-allowed-attr`, or `color-contrast` violations on History, Devices,
Settings (all tabs), or the popup, in **both** themes and **all six** accents.

**AT-18 — Tablist keyboard.** ArrowRight from the last tab wraps to the first;
ArrowLeft from the first wraps to the last; Home/End jump to the bounds;
unrelated keys are ignored.

**AT-19 — 720 px reflow.** At 720 × 460, all seven Settings tabs are visible
(wrapped), the About links wrap, and the logs toolbar wraps — nothing is clipped
or hidden behind an invisible scroller.

### Polling & degradation

**AT-20 — Visibility gating.**
*When* the document is hidden, no poll fires; *when* it becomes visible, a
refresh fires **immediately** and the interval restarts.

**AT-21 — Error backoff.**
*Given* the daemon is offline, *then* the history poll interval is 5000 ms, not
3000 ms; *when* it recovers, *then* exactly one refresh runs (no double-fire on
the state transition).

**AT-22 — Stale-response ordering.**
*Given* a slow refresh A and a fast refresh B issued after it, *when* A resolves
last, *then* B's data is displayed.

**AT-23 — Migration retry.**
*Given* the daemon replies `migration_in_progress` four times then succeeds,
*then* the call succeeds after backoffs 250/500/1000/2000 ms; *given* six
failures, *then* the error propagates. Any other error code is **not** retried.

**AT-24 — No socket path in the DOM.**
*Given* any IPC failure whose raw message contains `/Users/<name>/…`, *then* no
rendered text (nor any accessible name) contains that substring.

**AT-25 — Banner priority.**
*Given* a daemon spawn error **and** a protocol mismatch **and** a stale daemon
**and** missing accessibility permission simultaneously, *then* exactly one
banner renders (the daemon spawn error); *when* it clears, the protocol-mismatch
banner appears without needing to be re-triggered.

**AT-26 — Presence fallback.**
*Given* a peer's presence entry has expired, *then* the dot reflects the daemon's
`list_peers` `online` value — not "offline".

**AT-27 — Daemon restart clears stale dots.**
*Given* the presence poll fails then succeeds, *then* all presence entries reset
to offline immediately (not after TTL).

### Pairing

**AT-28 — SAS digits are shown and are inert.** Six digits rendered; the region
has an accessible label `Security code: <digits>`; text selection and copy are
not possible; the raw QR payload string appears nowhere in the DOM.

**AT-29 — Watchdog hides the decision buttons.**
*Given* 60 s elapse with no terminal state, *then* the timeout message is shown
**and** the SAS digits and Match/Doesn't-match buttons are gone.

**AT-30 — Trailing idle after local accept = success.**
*Given* the user clicked Match and the daemon's next poll returns `idle` (not
`confirmed`), *then* the modal shows `Paired ✓` and fires the paired callback.

**AT-31 — Trailing idle without local accept = neutral end.**
*Then* the modal shows `Pairing ended — check the other device.`, not an error.

**AT-32 — Abort on every close.** Closing from any state (including `confirmed`)
issues `pair_abort`, and a subsequent pairing attempt is not blocked.

**AT-33 — No phantom responder modal.**
*Given* an inbound pairing completed and the user navigated away from Devices and
back, *then* no SAS modal reopens.

**AT-34 — Initiator is never seeded with responder state.**

**AT-35 — QR regenerate re-blurs.**
*Given* the QR was revealed, *when* the user clicks Regenerate, *then* it is
blurred again before the new code appears.

**AT-36 — QR does not regenerate while hidden.**
*Given* the window is hidden for 5 minutes, *then* zero regenerations occurred;
*when* it becomes visible with an expired token, *then* exactly one regeneration
fires.

**AT-37 — Only one confirm modal at a time.** Opening Revoke-all while a
single-device revoke prompt is open closes the latter.

### Popup / window

**AT-38 — Blur-to-close does not surface the main window.**
*Given* the popup is open over a third-party app, *when* the user clicks that
app, *then* the popup hides and focus goes to that app — the main window does not
come forward.

**AT-39 — First-ever open with no prior app.**
*Given* no prior app has been recorded, *when* the popup is dismissed, *then* the
main window does not get promoted, and the Dock icon / Cmd+Tab entry remain.

**AT-40 — Copy failure keeps the popup open** with the error visible, and the
next click works (the hide guard is reset).

**AT-41 — Concurrent blur + row click activates the prior app exactly once**
(no focus flicker).

**AT-42 — Positioning clamp.** In each of the three modes, on a secondary
monitor with negative coordinates, the popup is fully within that monitor's
frame.

**AT-43 — Prefs propagate to the warm popup.**
*Given* the popup has been shown once, *when* the theme is changed in the main
window and the popup is shown again, *then* the popup renders in the new theme.

**AT-44 — Memory freed on hide.** After hide, the popup's item list and image
cache are empty; after the next show, the list repopulates.

**AT-45 — ⌘1–9 only without a query.**

**AT-46 — Hover does not steal keyboard selection** within 250 ms of an arrow
key, nor during scroll momentum.

**AT-47 — No "Nothing copied yet" flash** on popup open.

**AT-48 — Close hides, Quit exits.** Closing the main window hides it and leaves
the daemon running; tray → Quit exits and stops the daemon.

### Theme & prefs

**AT-49 — No theme flash.**
*Given* persisted `theme: "light"`, *when* the window is opened, *then* the first
painted frame is light. (Assert the bootstrap ran before the app module via its
ordering marker.)

**AT-50 — Per-field corruption recovery.**
*Given* stored prefs `{theme: "chartreuse", accent: "teal", translucency: 5}`,
*then* theme → default, accent → `teal` (**preserved**), translucency → default,
and a warning is logged for each present-but-invalid field. An **absent** field
defaults **silently** (no warning).

**AT-51 — Malformed storage.** Malformed JSON, a non-object payload, and a
throwing `localStorage` each fall back to full defaults without throwing.

**AT-52 — Unknown keys dropped.** A stored blob with extra keys loads cleanly and
those keys are not re-persisted.

**AT-53 — System theme live.**
*Given* `theme: "system"`, *when* the OS appearance flips, *then* `data-theme`
updates live without a reload — in **both** windows — and exactly one matchMedia
listener exists (no accumulation across re-applies).

**AT-54 — Bootstrap/schema parity.** The pre-paint script's key, defaults,
allowed enums, and translucency→`on|off` mapping match the TS schema exactly.

**AT-55 — Token parity.** Every custom property defined in
`copypaste-design-reference.html` resolves to an identical value in the new
theme, for both themes and all six accents.

### Shortcuts

**AT-56 — Layout independence.**
*Given* a Cyrillic keyboard layout, *when* the user presses the physical Q key
with ⌘⇧, *then* the captured accelerator is `CmdOrCtrl+Shift+Q`.

**AT-57 — Bare modifiers ignored;** a combination with **no** modifier is
rejected (returns nothing, nothing is saved).

**AT-58 — Escape cancels capture** without changing the binding.

**AT-59 — Unregisterable accelerator does not crash startup**; the value is
preserved and a warning logged.

**AT-60 — Default comes from the backend**, and equals `CmdOrCtrl+Shift+V`.

### Settings

**AT-61 — Field-scoped revert.**
*Given* an unsaved change to slider A, *when* toggle B's save fails, *then* only
B reverts — A's unsaved value is untouched.

**AT-62 — Restart failure is loud.**
*Given* Supabase credentials save succeeds but the daemon restart fails, *then*
the "Saved" state is cleared and an error is shown.

**AT-63 — Test-connection aborts on save failure.**

**AT-64 — Blank credential fields preserve stored values** (the patch omits them).

**AT-65 — Import requires confirmation** and never touches the database before
it; on success History refreshes immediately without waiting for its poll.

**AT-66 — Excluded-app edit is not applied before load.**
*Given* `loadState !== "ready"`, *when* the user adds a bundle id, *then* nothing
appears in the list (the old code showed it, never saved it, and it vanished on
reload).

**AT-67 — Private mode converges.** Toggling from the tray updates the Settings
toggle without a manual refresh, and vice versa; on IPC failure the tray
checkmark reverts.

### History misc

**AT-68 — Badge reflects the filter.**
*Given* 14 total items and a query matching 0, *then* the badge reads `0 items`,
not `14 items`.

**AT-69 — Pinned copy does not jump.** Copying a pinned item leaves it in its
`pin_order` slot; copying an unpinned item moves it to the top of the unpinned
section (below the last pinned row).

**AT-70 — Reorder revert.** A failed `reorder_pinned` restores the server order
and shows an error.

**AT-71 — Undo restores.** Delete → Undo within 5 s → the item is present and
was never deleted server-side. Delete → wait 5 s → it is deleted. Delete → delete
another within 5 s → the first commits immediately. Delete → unmount → the delete
commits.

**AT-72 — Bulk busy always releases.** Force the refresh after a bulk delete to
throw; the bulk bar is still interactive.

**AT-73 — FTS beyond page 1.**
*Given* 1000 items with a match only at index 800, *when* the user searches for
it, *then* it appears (the view auto-loads all pages while searching).

---

## 7. Keyboard shortcuts & shortcut-capture rules

### 7.1 Global

| Shortcut | Action |
|---|---|
| `CmdOrCtrl+Shift+V` (default, user-rebindable) | Toggle the Quick-Paste popup |

Registered via the global-shortcut plugin; on macOS a CGEventTap is additionally
installed for the same accelerator. Registration failure is non-fatal (INV-24).

### 7.2 In-app

See §3.1.4 (History) and §3.5.3 (popup). Settings tabs follow A11Y-6.

### 7.3 Capture rules

1. **Ignore bare modifier keydowns** (`Meta`, `Control`, `Alt`, `Shift`) — nothing
   to bind yet.
2. **Require at least one modifier.** A modifier-less key returns nothing.
3. **Derive the key from `e.code`, never `e.key`** (INV-23):
   - `Key*` → strip the `Key` prefix (`KeyQ` → `Q`),
   - `Digit*` → strip the `Digit` prefix (`Digit1` → `1`),
   - otherwise use `e.code` (falling back to `e.key`).
4. Single characters are upper-cased. Multi-character names map through a fixed
   table: `ArrowUp/Down/Left/Right` → `Up/Down/Left/Right`, `" "`/`Space` →
   `Space`, `Enter`/`Return` → `Return`, plus pass-through for `Escape`,
   `Backspace`, `Delete`, `Tab`, `Home`, `End`, `PageUp`, `PageDown`, `F1`–`F12`.
5. **Modifier order is fixed**: `CmdOrCtrl`, `Alt`, `Shift`, then the key, joined
   by `+`. `metaKey || ctrlKey` both map to `CmdOrCtrl` (Tauri's cross-platform
   alias).
6. Escape during capture cancels and blurs without changing the binding.
7. **Display is separate from the value.** The stored value is always the
   accelerator string; the UI renders one keycap per `+`-separated token, mapping
   tokens to glyphs (`CmdOrCtrl|Cmd|Command|Meta|Super` → `⌘`, `Ctrl|Control` →
   `⌃`, `Alt|Option` → `⌥`, `Shift` → `⇧`, `Return|Enter` → `↩`, `Backspace` →
   `⌫`, `Delete` → `⌦`, `Escape` → `⎋`, `Space` → `␣`, `Tab` → `⇥`, arrows →
   `↑↓←→`). Splitting on `+` (not per character) keeps `F1` a single keycap.
   Unmapped tokens pass through.
8. The accessible name uses the raw accelerator string, not the glyphs (A11Y-13).

---

## 8. Design tokens that must survive

`tokens.css` is **value-for-value parity-locked** to the token block in
`copypaste-design-reference.html` (repo root), enforced by
`src/styles/tokens.parity.test.ts`. The parity is directional: the new theme may
add tokens, but every token the reference defines must resolve to the identical
value. **Recreate this test.**

Axes on `<html>`: `data-theme="dark|light"` (resolved) ·
`data-theme-pref="system|dark|light"` (raw choice) ·
`data-accent="indigo|blue|teal|green|amber|rose"` ·
`data-translucency="on|off"`. Every `:root` themed selector is duplicated on
`.theme-scope[...]` so the dev gallery can preview a different theme in a scoped
wrapper without mutating `<html>`.

### 8.1 Dark theme (default)

```
--bg:#0E0F14  --panel:#16181F  --elevated:#1E2027  --card:#1E2027
--raised:#282B33  --raised-2:#33373F  --border:#33363F  --divider:#24262D
--text:#E7E9EE  --dim:#9CA1AC  --faint:#8F94A0  --mute:#5C616B
--hover:rgba(255,255,255,.045)  --pressed:rgba(255,255,255,.075)
--selected:color-mix(in srgb,var(--accent) 16%,transparent)
--scrim:rgba(0,0,0,.55)
--ok:#4FB866  --warn:#E0A33F  --err:#E5645F  --info:#5B9DFF
--c-text:#8B93A5  --c-url:#34D1BF  --c-code:#A78BFA  --c-image:#E879C6
--c-mail:#4ED98A  --c-color:#F5A524  --c-num:#5CC1CE  --c-path:#5B9DFF
--c-file:#5B9DFF  --c-json:#FB7B53  --c-secret:#F2616B
--sh1:0 1px 2px rgba(0,0,0,.30)
--sh2:0 8px 24px -6px rgba(0,0,0,.45)
--sh3:0 24px 64px -12px rgba(0,0,0,.60)
```

### 8.2 Light theme

```
--bg:#F5F6F8  --panel:#FFFFFF  --elevated:#FFFFFF  --card:#FFFFFF
--raised:#EFF1F4  --raised-2:#E2E5EA  --border:#E1E4E9  --divider:#ECEEF1
--text:#1A1C22  --dim:#565B66  --faint:#6E7380  --mute:#A2A7B1
--hover:rgba(15,18,26,.045)  --pressed:rgba(15,18,26,.075)
--selected:color-mix(in srgb,var(--accent) 12%,transparent)
--scrim:rgba(20,22,30,.28)
--ok:#1FA85B  --warn:#C77F1A  --err:#D64545  --info:#2563EB
--c-text:#6A7282  --c-url:#0E9E8C  --c-code:#7C5CE6  --c-image:#C44BA0
--c-mail:#1FA85B  --c-color:#C77F1A  --c-num:#1C8B9B  --c-path:#2F6FE0
--c-file:#2F6FE0  --c-json:#DC5A2E  --c-secret:#D64545
--sh1:0 1px 2px rgba(20,22,30,.06)
--sh2:0 8px 24px -8px rgba(20,22,30,.12)
--sh3:0 24px 64px -12px rgba(20,22,30,.18)
```

### 8.3 Contrast-corrected text variants (additive; **do not** merge into the base hues)

The base hue is correct as a fill / dot / syntax colour but fails AA as small
text on its own tint. These are foreground-only variants:

| Token | Dark | Light | Used for |
|---|---|---|---|
| `--err-strong` | `var(--err)` | `#B93434` | `.btn--danger` text on its 9%/16% tint |
| `--info-strong` | `var(--info)` | `#1D4ED8` | log level badge text on a 12% tint |
| `--ok-strong` | `var(--ok)` | `#157A42` | verified badge text on a 12% tint |
| `--warn-strong` | `var(--warn)` | `#96570A` | warn field-notes and warn banners |

### 8.4 Accent axis (theme-independent)

| Accent | `--accent` | `--accent-2` | `--on-accent` | Light `--accent` override |
|---|---|---|---|---|
| indigo (default) | `#6E5BFF` | `#9C8FFF` | `#fff` | `#5B49E0` |
| blue | `#3B82F6` | `#7CB0FF` | `#fff` | `#2563EB` |
| teal | `#13B8A6` | `#5FE0D2` | `#06302C` | `#0E9E8C` (`--on-accent:#fff`) |
| green | `#46C56A` | `#84E29A` | `#062A12` | `#1FA85B` (`--on-accent:#fff`) |
| amber | `#F5A524` | `#FFC56B` | `#2A1B05` | `#C77F1A` (`--on-accent:#fff`) |
| rose | `#F43F7E` | `#FF85AC` | `#fff` | `#E11D6B` |

### 8.5 Scale tokens

```
--f-ui:'Inter',-apple-system,BlinkMacSystemFont,'Segoe UI Variable','Segoe UI',system-ui,sans-serif
--f-mono:'JetBrains Mono',ui-monospace,'SF Mono','Cascadia Mono','Roboto Mono',Menlo,monospace

--r-chip:7px  --r-pill:999px  --r-ctl:8px  --r-input:9px
--r-card:13px --r-window:12px --r-xs:2px --r-sm:6px --r-row:10px
--r-chk:5px --r-empty-ic:16px

--s-1:2  --s-2:4  --s-3:6  --s-4:8  --s-5:11  --s-6:14  --s-7:16  --s-8:20  --s-9:24  (px)

--dur-fast:120ms  --dur:200ms  --dur-theme:300ms  --ease:cubic-bezier(.2,.8,.2,1)
  (all three durations → 0ms under prefers-reduced-motion)

--fs-3xs:9.5 --fs-2xs:10 --fs-xs:11 --fs-sm:11.5 --fs-smd:12 --fs-125:12.5
--fs-md:13 --fs-135:13.5 --fs-base:14 --fs-lg:15 --fs-xl:17 --fs-19:19
--fs-20:20 --fs-2xl:24 --fs-34:34  (px)
--fw-normal:450 --fw-medium:500 --fw-semibold:550 --fw-strong:600 --fw-bold:700
--lh-tight:1.2 --lh-normal:1.5
--ls-body:-.006em --ls-tight:-.01em --ls-none:0 --ls-wide:.05em --ls-wider:.06em

--focus-ring-width:2px  --focus-ring-offset:2px
--hairline:1px  --stroke-1:1px --stroke-2:2px --stroke-3:3px
--icon-sm:14px --icon-md:16px --icon-lg:20px
--ctl-h-sm:26px --ctl-h-md:30px --ctl-h-lg:34px

z-scale: --z-glide:0  --z-row:1  --z-grouphead:2  --z-dropzone:10
         --z-scrim:40 --z-toast:60 --z-popover:80

--main-min-w:640px --main-min-h:480px --popup-w:400px --content-max-width:640px

--pad-btn:7px 13px  --pad-btn-sm:5px 10px  --pad-field:8px 11px
--pad-chip:3px 9px  --pad-seg-btn:5px 12px --pad-tpill:2px 8px
--pad-badge:2px 7px --pad-kbd:0 5px        --gap-field:9px --gap-badge:5px

--sz-iconbtn:30px --sz-toggle-w:38px --sz-toggle-h:22px --sz-toggle-knob:18px
--sz-tile:34px --sz-dot:8px --sz-spinner:16px --sz-kbd:22px
--sz-chip-dot:7px --sz-badge-dot:5px

--modal-w:340px --modal-w-wide:560px --pad-modal:22px --gap-modal-sub:18px

--gap-row:12px --pad-row:9px 10px
--sel-bar-w:3px --sel-bar-r:3px --sel-bar-inset:6px
--chk-bw:1.5px --pad-empty:30px
--pad-grouphead:10px 10px 5px --pad-bulkbar:8px 10px 10px
--mask-blur:6px

--knob-fill:#fff  --sh-knob:0 1px 2px rgba(0,0,0,.3)
--ring-inset:rgba(255,255,255,.18)  --sheen:rgba(255,255,255,.22)
```

> **`--pad-row: 9px 10px` is load-bearing**: `ROW_PAD_V = 18` in the virtualizer
> is derived from it. If the row padding token changes, the height model must
> change with it — or rows will overlap.

### 8.6 Translucency axis

**Solid is the baseline.** Frosting is applied *additively* inside an
`@supports (backdrop-filter: blur(1px)) or (-webkit-backdrop-filter: …)` block, so
an engine without `backdrop-filter` automatically gets the solid fallback without
a separate rule — and no lower-specificity fallback can be defeated by the
higher-specificity on-state selector.

```
default / off / unsupported:
  --frost-filter: none;  --chrome-bg: var(--panel);  --scrim-blur: none;

[data-translucency="on"] + @supports:
  --frost-filter: saturate(180%) blur(20px);
  --chrome-bg: color-mix(in srgb, var(--panel) 72%, transparent);
  --scrim-blur: blur(2px);

@media (prefers-reduced-transparency: reduce)  → forced back to the solid values
  (same specificity, authored later, so it wins when it matches).
```

Only **chrome** surfaces frost (sidebar, popup container, modal scrim, toast, tab
bar). **Content** surfaces stay solid (`--panel` / `--card`).

### 8.7 Content-kind colour mapping

The normalized kind set is closed: `text | url | mail | num | color | json |
code | file | image | unknown`. Daemon aliases: `TEXT→text`, `URL→url`,
`EMAIL→mail`, `PHONE|NUMBER→num`, `COLOR→color`, `JSON→json`, `CODE→code`,
`PATH|FILE→file`, `IMAGE→image` (case-insensitive). An image MIME `content_type`
**wins over** `kind`; anything unrecognised → `unknown` (never a runtime error,
never a blank tile). Each kind maps to its `--c-<kind>` token; `code`, `json`,
`num`, and `color` also render monospace.

---

## 9. Known-unjustified complexity we should NOT port

Everything below existed because the old stack lacked a library, not because the
behaviour needed it. **Port the contract, not the mechanism.**

### 9.1 Hand-rolled infrastructure → use the libraries

| Old | Replace with | Keep from it |
|---|---|---|
| Bespoke virtualizer: `rowHeightFor` + prefix-sum `buildOffsets` + binary-search `computeVisibleWindow` + manual `padTop`/spacer | **TanStack Virtual** (`estimateSize` + `measureElement`) | The *height rules* as `estimateSize` (INV-5), `overscan` ≈ 240 px worth of rows, and **explicitly verify anchoring (INV-1) and clamp (INV-6)** — TanStack does not give you INV-1 for free with a mutating list. |
| Manual `setInterval` + `visibilitychange` per screen, ×8 | **React Query** `refetchInterval` + `refetchIntervalInBackground: false` + `refetchOnWindowFocus` | The *numbers* and the idle backoff (§5.1, §5.5). |
| `itemsSignature` + a 1-slot memo cache | React Query `structuralSharing` (+ a stable `select`) | INV-2's *guarantee*, and the INV-3 corollary: **every optimistic mutation must invalidate**, which in React Query is `invalidateQueries`/`setQueryData`, not a manual signature reset. |
| `requestSeqRef` sequence tagging | React Query's built-in request cancellation / last-write-wins | INV-33 as a test. |
| Hand-rolled `ipcCall` migration retry loop | React Query `retry` + `retryDelay` | INV-34's *policy*: retry **only** `migration_in_progress`. |
| `useFocusTrap` + `Dialog` + ref-counted `scrollLock` + portal | **Radix Dialog / AlertDialog** | A11Y-4 and INV-19 as tests; Radix already ref-counts. |
| Custom `ToastProvider` mounted **three separate times** (History, Devices, Settings each wrap themselves) | **sonner**, one app-level `<Toaster />` | Bottom-right placement, pause-on-hover/focus, durations, severity dot, z-order below dialogs. |
| `tabListKeyDown` factory + manual tab/tabpanel wiring | **Radix Tabs** | A11Y-6 wrap-around behaviour. |
| `InfoPopover` | **Radix Popover/Tooltip** | |
| `Toggle`, `SliderRow`, `AccentSwatch`, segmented control, `ConfirmModal` | **shadcn** Switch / Slider / ToggleGroup / AlertDialog | The copy, the value formatting (`N lines`, `Npx`), and the step arrays. |
| `loadPrefs` hand-validation + `validateTheme/Accent/Translucency` + whitelist merge | **zod** schema with `.catch()` per field | INV-21 *exactly*: per-field fallback, unknown-key stripping, warn on present-but-invalid, silent on absent. |
| `lib/fuzzy.ts` | any maintained fuzzy matcher (or keep it — it is small and tested) | Score-descending, stable-within-score ordering; match positions for highlighting. |
| Manual `React.memo` comparator listing 20 fields on `HistoryRow` | Stable props + React Compiler / narrow selectors | Nothing. This comparator is a maintenance hazard: adding an entry field silently breaks re-rendering. |

### 9.2 Dead or frozen abstractions — delete

- **The density axis.** `"comfortable" | "compact" | "spacious"` is threaded
  through `rowHeightFor`, `VirtualList`, `HistoryRow`, and the memo comparator —
  but production hardcodes `const density = "comfortable"`. Collapse to the
  single set of numbers (§5.2) and delete the parameter.
- **`previewSize` pref.** Documented as "kept for layout wiring; not exposed in
  UI". Either expose it or delete it; do not port a hidden pref that participates
  in height math.
- **`ViewTransitionWrapper` / `CrossfadeContainer`.** Two nested components whose
  animation was deliberately stripped; they exist only to preserve a `data-testid`
  and a `display: contents` hack. Delete both, keep `display: contents` on
  whatever wraps the view (the flex height chain does depend on it).
- **`HistoryView.tsx`'s re-export shims** (`itemsSignature`, `_itemsSigCache`,
  `rowHeightFor`, `buildOffsets`, `computeVisibleWindow` re-exported "so existing
  importers keep working"). Back-compat with itself. Delete.
- **`views/DevicesView.tsx`** — a 3-line file that re-exports
  `views/DevicesView/index.tsx`.
- **`selectionMode` as separate state.** It is fully derivable from
  `multiSelectedIds.size > 0`; the old code keeps it as independent state plus an
  effect to re-sync them. Derive it.
- **`SyncState` ← `SyncBadgeState` mapping function.** Explicitly documented as
  a 1:1 identity mapping. Use the daemon type directly.
- **`deriveSyncStateFallback`.** A deprecated fallback for daemons predating
  `badge_state`. Drop it if the rewrite ships with a minimum daemon version.
- **The `MOCK` / `?mock=1` / `?bridge=1` triple transport** with top-level
  `await import()` inside an `import.meta.env.DEV` branch. Replace with MSW or an
  injected transport at the composition root — the current arrangement makes the
  transport module's evaluation order load-bearing.
- **The dev-only Gallery routed through a URL param that bypasses the store**,
  plus a sidebar item that does a full page navigation to set that param. Use
  Storybook or a real route.
- **`INPUT_CLS` / `BTN_CLS` / `BTN_STYLE`** exported from a *hook* module and
  threaded as props through five tab components. This is Tailwind-class prop
  drilling; use the component library.
- **`useSettingsState`** — a single 1250-line hook returning **~90 fields**,
  including its own helper functions, which are then spread across five tab
  components as props. Split per tab; co-locate each tab's queries/mutations.
- **`buildConfigPatch` reading every live field on every save.** It exists solely
  because the daemon's `set_config` is a full-document write with no field-level
  patch. Keep the *invariant* (a toggle must never clobber unsaved fields) but
  solve it with a single form state object (react-hook-form + zod) rather than
  25 `useState`s reconciled by a closure — and note the deliberate
  non-memoisation of five handlers is a direct consequence of this design.
- **Four separate `role="alert"` mechanisms plus a priority queue** to
  de-conflict them. Model banners as one ordered list with a severity field
  (INV-17 is the requirement; the four-independent-states-plus-ternary-chain is
  not).
- **`historyBadge.ts`** — a whole module + test for a 3-line ternary. Inline it,
  keep the test case.

### 9.3 Things that look like complexity but are NOT — keep them

Called out explicitly so nobody "simplifies" a bug fix back into existence:

- Scroll anchoring (INV-1) and the shrink clamp (INV-6).
- The signature/dedup contract (INV-2/3/4) — however it is implemented.
- Full-`previewLines` height reservation (INV-5) — the "smarter" char-count
  estimate is the bug.
- `role="list"` instead of `role="listbox"` + the sibling live region
  (INV-8/9, A11Y-14).
- Selection tracked by id (INV-32).
- Popup hide-through-the-backend + prior-app activation + the blur/JS double-hide
  guard (INV-25, AT-38/39/41).
- Copy-before-hide (INV-26).
- Two-consecutive-identical-replies in the tray resync.
- Tri-state presence with `absent` meaning "fall back to `list_peers`"
  (§3.2.3) — the two-state version lies.
- QR blur independent of QR generation, and re-blur on regenerate (INV-13,
  AT-35).
- SAS trailing-`idle` interpretation (AT-30/31) and `pair_abort` on every close
  (INV-16).
- The 60 s SAS watchdog matching the daemon's (AT-29).
- `friendlyIpcError`'s code→copy mapping (INV-12).
- Per-field prefs validation (INV-21) and the pre-paint bootstrap (INV-22).
- `e.code`-based shortcut capture (INV-23).
- Visibility gating everywhere (INV-27/28).

---

## 10. Source index

| Concern | Old file(s) |
|---|---|
| Shell, routing, banner queue, global effects | `crates/copypaste-ui/src/App.tsx` |
| Nav + sync chip | `crates/copypaste-ui/src/components/Sidebar.tsx`, `components/SyncStatusChip.tsx` |
| Prefs store | `crates/copypaste-ui/src/store.ts`, `src/lib/theme/prefsSchema.ts` |
| Theme apply / pre-paint | `crates/copypaste-ui/src/lib/theme/applyTheme.ts`, `public/theme-bootstrap.js` |
| History view | `crates/copypaste-ui/src/views/HistoryView.tsx` |
| Virtualization | `crates/copypaste-ui/src/views/HistoryView/VirtualList.tsx`, `HistoryView/historyVirtualizer.ts` |
| Dedup | `crates/copypaste-ui/src/views/HistoryView/historySignature.ts` |
| History data/filter/selection/drop | `crates/copypaste-ui/src/views/HistoryView/hooks/*.ts` |
| Row + details + bulk bar | `crates/copypaste-ui/src/views/HistoryView/{HistoryRow,DetailsModal,BulkActionBar}.tsx` |
| Badge logic | `crates/copypaste-ui/src/views/HistoryView/historyBadge.ts` |
| Devices | `crates/copypaste-ui/src/views/DevicesView/index.tsx` + `DevicesView/hooks/*.ts` |
| Pairing modals | `crates/copypaste-ui/src/views/DevicesView/{SasPairingModal,RevokeConfirmDialog}.tsx`, `hooks/useQrCode.ts` |
| Settings | `crates/copypaste-ui/src/views/SettingsView.tsx`, `SettingsView/hooks/useSettingsState.ts`, `SettingsView/{tabs,components}/**` |
| Shortcut capture | `crates/copypaste-ui/src/views/SettingsView/components/ShortcutCapture.tsx` |
| Dialog / focus trap / scroll lock | `crates/copypaste-ui/src/lib/dialog/{Dialog.tsx,scrollLock.ts}`, `src/lib/useFocusTrap.ts` |
| Toasts | `crates/copypaste-ui/src/components/Toast.tsx` |
| a11y helpers | `crates/copypaste-ui/src/lib/a11y/{tabListKeyDown.ts,DisclosureHeader.tsx,a11y.test.tsx}` |
| IPC transport / errors | `crates/copypaste-ui/src/lib/ipc/{transport.ts,helpers.ts,api.ts,types.ts}` |
| Presence store | `crates/copypaste-ui/src/lib/peerPresence.ts` |
| Masking / kinds / image cache | `crates/copypaste-ui/src/lib/masking.ts`, `lib/clip/normalizeContentKind.ts`, `components/ImageThumb.tsx`, `hooks/useSensitiveReveal.ts` |
| Popup (web) | `crates/copypaste-ui/src/popup/{Popup.tsx,usePopupHistory.ts,PopupRow.tsx,GlideHighlight.tsx}` |
| Popup (native) | `crates/copypaste-ui/src-tauri/src/popup/{window.rs,position.rs,setup.rs,focus.rs,paste.rs,state.rs}` |
| Tray | `crates/copypaste-ui/src-tauri/src/tray.rs` |
| Global shortcut registration | `crates/copypaste-ui/src-tauri/src/lib.rs`, `src-tauri/src/config.rs`, `src-tauri/src/event_tap.rs` |
| Window config / CSP | `crates/copypaste-ui/src-tauri/tauri.conf.json` |
| Design tokens | `crates/copypaste-ui/src/styles/tokens.css` ⟷ `copypaste-design-reference.html` |
| Style layers | `crates/copypaste-ui/src/styles/{reset,base,primitives,patterns,shell,utilities}.css` |
| Tests worth re-creating | `src/styles/tokens.parity.test.ts`, `src/styles/shell.responsive.test.ts`, `src/lib/theme/themeBootstrap.test.ts`, `src/lib/theme/prefsSchema.test.ts`, `src/views/HistoryView/historyVirtualizer.test.ts`, `src/views/HistoryView/historyBadge.test.ts`, `src/lib/a11y/a11y.test.tsx`, `src/lib/dialog/Dialog.test.tsx`, `e2e/visual/layout-invariants.spec.ts` |

---

## Appendix A — Bug-fix ledger (bug → rule)

Every `CopyPaste-<id>` (and legacy audit-tag) comment found in the UI source,
with the rule it encodes. The rule is what the rewrite must satisfy.

### Virtualization & list

| Id | Bug | Rule |
|---|---|---|
| `8ebg.44` | Any row-count/height change above the viewport shifted everything below by the same delta; the viewport jumped out from under a mid-scroll user | Anchor by item id + intra-row offset (INV-1) |
| `f2ec #17` | Display-setting change shrank total height below a stale `scrollTop`; the window collapsed to the last row or two until the next manual scroll | Clamp tracked **and** DOM scroll on content/viewport change (INV-6) |
| `g27b.30` | Multi-line height estimated at ~120 chars/line; at 400 px a "2-line" clip wrapped to 6+ and overflowed into the next row | Reserve the full `previewLines` cap; never estimate from char count (INV-5) |
| `g27b.25` | `imageMaxHeight` only fed the height reservation and the `<img height>` decode hint, so tall images grew to the width cap and ballooned the row | Cap rendered thumbnail height in CSS via a per-row var |
| `8ebg.16` | Load-more kept the first-page signature; the next poll either skipped a needed render or replaced the merged list with page 1 | Signature tracks the merged list (INV-4) |
| `8ebg.18` | Delete/undo didn't invalidate the signature; the next poll matched a stale fingerprint and skipped the re-render | Every mutation invalidates (INV-3) |
| `44rq.35` | O(n) hash on every 3 s poll over 200 items | 1-slot envelope cache (len + first + last fingerprint); provably safe because a new item changes length and a pin re-orders to first |
| `5917.33` | `aria-activedescendant` referenced a row outside the render window | Clear the pointer when the target isn't rendered (INV-7) |
| `g27b.29` | `role="option"` flattened nested Pin/Preview/Delete buttons → axe `nested-interactive` | `role="list"`/`listitem` + `aria-current` (INV-8) |
| `8ebg.45` | Losing listbox semantics lost arrow-key announcements | Polite live region mirroring the active row's label (INV-9) |
| `wrfn` | The live region inside `role="list"` failed `aria-required-children` | Announcer must be a sibling (A11Y-14) |
| `5917.75` | Multi-select glide drew one rectangle first→last, visually selecting interleaved unselected rows | Hide the glide in multi-select; highlight per row |
| `8ebg.55` | `.row.copied` flash existed in CSS but nothing toggled it; bulk bar showed Pin **and** Unpin when one was always a no-op; toasts vanished mid-read | Flash on copy (~700 ms); single Pin/Unpin toggle driven by `allPinned`; pause toast timer on hover/focus |
| `f72f` | The whole `.row__right` was unmounted in selection mode, hiding the `too_large_to_sync` warning | Only the action buttons are selection-mode-only |
| `g27b.37` | Badge read "14 items" beside a zero-match search | Badge shows the filtered count whenever a filter is active |
| `crh3.106` | FTS hits past the first 200 were silently absent | Auto-load all pages while a search is active |
| `crh3.111` | Alt+Enter paste-as-plain-text failure only hit the console | Every keyboard action surfaces failures |
| `SCRH-9` | Users confused by fewer visible items than the badge total | `aria-live` display-limit hint |
| `SCRH-12` | The undo toast rendered above the details modal | Transient notices sit below dialogs |
| `5j9x`, `kayk`, `fjvz`, `vcnv`, `w6xc` | Destructive actions were inline Yes/No, misclickable | Confirm modal for reset-db, clear-all, bulk delete, import, clear history |
| `V-13` | A throw during bulk cleanup left the bulk bar permanently disabled | Release busy flags in `finally` (INV-30) |
| `F11` | Deferred delete could be silently abandoned on unmount | Commit pending deletes on unmount |
| `g27b.4` | Static CSS lived in inline styles | Only per-render computed values stay inline |

### Popup & window

| Id | Bug | Rule |
|---|---|---|
| `V-10` | `toggle_popup` called `popup.hide()` directly, skipping prior-app activation and surfacing the main window | One shared hide path (INV-25) |
| `V-11` | With no prior app recorded, hiding promoted the main window | Temporarily switch to Accessory policy, hide, restore Regular |
| `V-12` | Concurrent blur + JS hide activated the prior app twice → focus flicker | Guard on `is_visible()` |
| `D7` | macOS promoted the next same-policy window on hide | Activate the recorded prior bundle id first |
| `HW-M6` | Hiding before the copy resolved swallowed image-copy errors and produced every-other-click races | Copy first, hide second (INV-26) |
| `M1` | The popup WebView was built at launch (~84 MB idle RSS) | Lazy-create on first toggle; free only the JS heap on hide, keep the WebView warm |
| `8ebg.10` | The key handler lived on the search input, so clicking Pin killed ↑↓/⏎/Esc | Attach to the popup root |
| `8ebg.17` | The 3 s poll reordered the list between keydown and Enter, pasting the wrong item | Track selection by id (INV-32) |
| `8ebg.36` | `scrollIntoView` moved the list under a stationary cursor; the resulting mouseenter stole the keyboard selection | 250 ms hover suppression after keyboard nav |
| `zuzu` | Momentum scroll fired mouseenter for every passed row; the highlight jumped | Suppress hover while scrolling (120 ms idle) |
| `8ebg.37` | "Nothing copied yet" flashed on every open (items cleared on hide, refetched on show) | Render a blank list area while loading |
| `8ebg.56` | The popup silently showed 50 of 214; four refresh triggers could clobber each other | Show `N of total`; sequence-tag every request (INV-33) |
| `8ebg.64` | The result counter updated silently for screen readers | `aria-live="polite"` |
| `crh3.110` | Popup pin failure was console-only | Surface it |

### Pairing & security

| Id | Bug | Rule |
|---|---|---|
| `1jms.1` | The SAS code was selectable/copyable | Display-only, `user-select:none` (INV-14) |
| `1jms.2` / `v5a` / `crh3.21` | The QR was visible by default; regenerating cleared the privacy blur | Blur by default; blur state independent of generation; regenerate re-blurs first (INV-13) |
| `1jms.5` | The raw `CPPAIR2.*` payload could reach the DOM | Only the SVG is rendered (INV-13) |
| `1jms.3` / `1jms.12` | Not resetting the daemon state machine blocked the next LAN pairing | `pair_abort` on every close (INV-16) |
| `1jms.7` | The countdown showed 15 s while the token was already queued for replacement | Zero the countdown when the refresh fires |
| `8ebg.15` | The drain bar divided by a stale literal 300 vs. the real 120 s TTL — started at ~40%, never reached 100% | Use the token's actual TTL |
| `8ebg.51` | mDNS-advertised peer metadata was styled like verified data, right next to the SAS code | Label it unverified and de-emphasise (INV-15) |
| `8ebg.52` / `bdac.9` | A 30 s client watchdog said "timed out" while the daemon's own 30 s window kept the buttons live and functional | 60 s watchdog matching the daemon |
| `8ebg.30` | The timeout error could render while live SAS digits and decision buttons were still shown | Gate every non-terminal branch on `error === null` |
| `8ebg.28` | `incomingPairing` was never cleared, so returning to Devices re-opened a phantom SAS modal; and the responder payload was also being fed into the initiator modal | Clear on consume; initiator starts from its own default |
| `g27b.36a` | "Revoke all" and a single-device revoke prompt could both be open, stacking scrims | One confirm modal at a time (INV-18) |
| `tzzu` / `j5qg` / `ERR-1` / `ERR-2` | Raw errors containing the daemon socket path (with the local username) reached the DOM | `friendlyIpcError` everywhere (INV-12) |
| `PG-25` / `13a3` / `6uy9` | Screenshots captured clipboard history | Content protection on by default, opt-out only (INV-35) |
| `SCRH-7` / `5917.56` | Revealed secrets stayed visible on an unattended screen | Re-blur on window blur **and** after 10 s (INV-11) |
| `A11Y-1` | Masked rows announced their plaintext | Fixed placeholder label (INV-10) |
| `C-P0-4` | A plain revoke left the device still able to sync via cloud/relay | Offer "Revoke & rotate"; derive the new key before revoking |
| `x09o` | Poll cadences duplicated across the Rust/TS boundary with no shared source | Tracked drift risk — keep constants in one place if possible |

### Shell, state & a11y

| Id | Bug | Rule |
|---|---|---|
| `8ebg.12` | A single outer boundary meant any crash unmounted navigation too, and the fallback rendered against a bare body | Shell never inside a boundary; sibling boundaries for nav and main (INV-20) |
| `8ebg.39` | Four `role="alert"` banners could stack simultaneously | Severity-ranked queue, one winner (INV-17) |
| `8ebg.29` | Loading indicators were classless empty elements — blank screens indistinguishable from layout bugs | Loading states must be visibly rendered |
| `bdac.2` | `loading` fell through to the main layout, indistinguishable from "no devices" | Early-return a spinner |
| `bdac.6` | `ipc_not_ready` was treated as a hard error with an unfriendly message | Dedicated "Starting up…" state, checked **before** setting error detail |
| `tk2j` | A degraded daemon was mislabelled "not running" | Probe `status` to distinguish offline / degraded / error |
| `8ebg.19` | A failed daemon restart after saving credentials left "Saved" showing while sync silently broke | Restart failure is loud |
| `8ebg.20` | Excluded-app edits applied optimistically before config was loaded, then vanished on reload | Gate optimistic updates on readiness (INV-29) |
| `crh3.50` | Test-connection ran against the stale daemon config after a failed save | Abort with a clear message |
| `crh3.51` / `bdac.106` | Success was inferred from `msg === "Saved"`; any wording change turned success red | Typed `{ok, message}` signals |
| `V-9` | A blank key field overwrote the stored Supabase key with `null` | Omit unchanged credential fields |
| `3c72` | Untrimmed email/password caused silent auth failures | Trim credentials before sending |
| `V-21-A` | The tray checkmark defaulted false when the daemon wasn't up yet | Background resync, two consecutive identical replies |
| `V-21-B` | The tray checkmark stayed toggled after a failed IPC | Revert + broadcast the corrected value (INV-38) |
| `8ebg.23` | Blocking IPC on the main thread froze the menu bar on every click and stalled launch | Offload to background threads (INV-37) |
| `M4` | Private mode diverged between the tray and Settings | `private-mode-changed` broadcast + focus/visibility re-fetch (INV-39) |
| `h97m` | Imported items didn't appear until the next poll | `history-refresh` event |
| `8ebg.54` | The log filter was lost on every Settings tab switch | Lift the filter to the view |
| `8ebg.53` | The shortcut control never announced the currently bound accelerator | Announce the raw accelerator (A11Y-13) |
| `sqw0` | The TS default shortcut could drift from the Rust constant | Fetch the default from Rust |
| `g27b.31` | At the 720 px minimum, the Settings tab row overflowed behind a hidden scrollbar ("Logs" off-screen), About links spilled, the logs toolbar overflowed | Wrap; fixed-width level badge (A11Y-15) |
| `g27b.20` | Live OS-theme changes needed a reload; repeated applies accumulated listeners | Idempotent module-level matchMedia subscription; `data-theme-pref` carries the raw choice |
| `g27b.27` | `--faint` failed AA as meta text; `--err`/`--info`/`--ok`/`--warn` failed AA as small text on their own tints | Lift `--faint`; add `*-strong` text-only variants (A11Y-10) |
| `8ebg.63` | "System" didn't say what it resolved to; sliders showed bare numbers | Live-resolved hint; unit-formatted slider values |
| `8ebg.61` | Six raw z-index literals scattered across two stylesheets | Named z-scale tokens |
| `8ebg.38` / `7w060.2` | Toasts self-positioned to the same fixed spot and stacked on top of each other; bottom-centre bled into the sidebar | One positioned stack, `column-reverse`, bottom-right |
| `f701` | A 10 s poll left the sync chip green long after the daemon died | 2 s status poll |
| `crh3.48` | The 2 s chip poll dragged `list_peers` with it (30/min) | Decouple to a 10 s peer-count poll |
| `8ebg.26` | The global badge read "synced" while one peer had silently received nothing | Per-peer stall pill; 30 min threshold |
| `ptgcc` | A broken pairwise key produced no signal until the stall threshold elapsed | `rekey_failures > 0` flags a peer immediately |
| `k1jo` / `PG-44` | Supabase URL set but incomplete config gave no feedback | Dedicated "cloud sync misconfigured" indicator |
| `yw2k` | Peers signed into a different Supabase account synced nothing, silently | Compare peer `supabase_account_id` against the local one; show a mismatch banner |
| `5917.11` / `SCRD-3` / `SYNC-5` | Presence entries stayed green forever after a daemon outage; the naive fix lied "Offline" | Tri-state: expired entries are deleted so consumers fall back to `list_peers` |
| `s7ia B1/B3` | 8 pollers ≈ 150+ IPC calls/min at idle | 3 s history, 5 s/30 s adaptive presence |
| `HB-10` | 24 MiB data-URI cache plus unbounded decoded bitmaps | 16 MiB LRU + 192 px thumbnail source + decode hints |
| `HB-9` | Manual rescan unreachable when passive discovery found nothing | Always render the Refresh button |
| `44rq.27` | An empty discovered list gave no reason (P2P disabled, socket timeout) | Surface the reason inline |
| `ro0r` | Transient `migration_in_progress` surfaced as a hard failure | Retry that code only, with backoff (INV-34) |
| `audit P1-7` | `listen()` rejected on every mount outside Tauri, logging console errors | Feature-detect `__TAURI_INTERNALS__` |
| `audit P2` | The SAS modal body was blank before the first poll tick | Pre-handshake waiting placeholder |
| `xn95` / `5917.103` | Granting accessibility permission gave no confirmation; the confirmation vanished with no visual cue | `role="status"` confirmation, 3 s with a 500 ms fade, `aria-label` announcing transience |
| `5917.3` / `A11Y-2` | The permission warning wasn't announced until navigated to | `role="alert" aria-live="assertive"` |
| `A11Y-11` / `5917.30` | Every modal re-wired Escape itself | Escape handled once in the dialog primitive |
| `7set` | The sync kill-switch silently had no effect on daemons that ignore `sync_enabled` | Detect the missing field and warn |
| `am9w` | Absent `collect_public_ip` defaulted differently in UI and daemon | Mirror the daemon's `serde(default)` (opt-out, false) |
| `bdac.34/36/45/84` | "Daemon" jargon and British spelling in user-facing copy | "clipboard service" / "background service"; American English |
| `7w060.6` | A permanently-visible shortcut hint crowded the header and read as disabled text | Hover tooltip instead |
| `MOT-18` / `MOT-21` | Spinners/pulses ignored reduced-motion | Respect `prefers-reduced-motion`; static muted text for loading copy |
| `n9gp` / `PG-34` | The sensitive-reveal warning was mandatory | User-toggleable `showSensitiveWarnings` (default on, Android parity) |
| `bdac.91` | The History sort toggle and the Settings "Group by device" pref could disagree | The toolbar toggle persists to the pref |
