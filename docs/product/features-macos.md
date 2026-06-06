# CopyPaste macOS — Complete Feature Inventory

> **Scope:** macOS desktop app (Tauri shell + React/TS frontend + copypaste-daemon).
> **Sources inspected:** `crates/copypaste-ui/src/`, `crates/copypaste-ui/src-tauri/src/lib.rs`,
> `crates/copypaste-ui/src-tauri/src/ipc.rs`, `crates/copypaste-ui/src-tauri/src/daemon_lifecycle.rs`,
> `crates/copypaste-ui/src-tauri/src/event_tap.rs`.
> **Branch:** `feat/android-parity-v0.5.3` (as of 2026-06-04).

---

## Table of Contents

1. [Clipboard History — Main View](#1-clipboard-history--main-view)
2. [Quick-Paste Popup](#2-quick-paste-popup)
3. [Image Clips](#3-image-clips)
4. [File Clips](#4-file-clips)
5. [Sensitive / Private Masking](#5-sensitive--private-masking)
6. [Pin & Reorder](#6-pin--reorder)
7. [Multi-Select & Bulk Actions](#7-multi-select--bulk-actions)
8. [Details Modal](#8-details-modal)
9. [Menu-Bar Tray Icon](#9-menu-bar-tray-icon)
10. [Global Hotkey](#10-global-hotkey)
11. [macOS Notifications](#11-macos-notifications)
12. [Copy Sound](#12-copy-sound)
13. [Devices & Pairing](#13-devices--pairing)
    - 13a. This-Device Card
    - 13b. QR Code Pairing
    - 13c. LAN Discovery + SAS Pairing
    - 13d. Incoming (Responder) Pairing
    - 13e. Paired-Peer Management
14. [Settings — General Tab](#14-settings--general-tab)
15. [Settings — Display Tab](#15-settings--display-tab)
16. [Settings — Sync Tab](#16-settings--sync-tab)
17. [Settings — Shortcuts Tab](#17-settings--shortcuts-tab)
18. [Settings — Storage Tab](#18-settings--storage-tab)
19. [Settings — Advanced Tab](#19-settings--advanced-tab)
20. [Sync Status Indicators](#20-sync-status-indicators)
21. [Degraded / Recovery States](#21-degraded--recovery-states)
22. [App Lifecycle & Daemon Management](#22-app-lifecycle--daemon-management)
23. [Logs View](#23-logs-view)
24. [About View](#24-about-view)
25. [Notable Gaps Observed](#25-notable-gaps-observed)

---

## 1. Clipboard History — Main View

**Purpose:** Full browsable history of all captured clipboard items.

### What it does

- Loads an initial page of **200 items** from the daemon (`history_page` IPC, server-side cap at 1,000); items arrive sorted **pinned-first, then newest-first** within each group.
- **Total count badge** in the toolbar shows the true DB count across all pages (not just the loaded slice); hidden until the first page resolves.
- **Infinite-scroll load-more**: when the user scrolls to within 300 px of the bottom, the next 200-item page is fetched and appended (de-duplicated by ID). Load-more is disabled while a search filter is active.
- **Virtualised list** with a prefix-sum height model — only the rows in the viewport (plus ±240 px overscan) are rendered to the DOM. Row heights are mixed: text rows scale to `previewSize` (min 22 px), file rows are 44 px, image rows are `imageMaxHeight + 10` px (min 34 px).
- **Auto-refresh** at 1,200 ms when the daemon is healthy; backs off to 5,000 ms when offline or in error state. Pauses when the window is backgrounded (visibility API).
- **Optimistic reorder on copy**: when a non-pinned item is copied it is immediately moved to the first unpinned position in the local list; the next poll reconciles with server state.

### Per-row content

Each row shows (left to right):
- **Drag handle** (pinned rows only; visible on hover): six-dot grid icon.
- **Checkbox** (always in flow; invisible at rest; fades in on hover or in selection mode): enters multi-select.
- **Pin indicator** (amber bookmark ribbon): shown on pinned rows.
- **"Too large to sync" indicator** (amber warning triangle): shown when the daemon flagged the item as exceeding the sync size cap — kept locally but not synced.
- **Type chip / glyph**: image and file items show a coloured SVG icon; text items show a full-word `KindChip` (see below).
- **Content area**:
  - *Text/URL*: multiline preview clamped by `previewLinesApp` (1–6 lines), with ellipsis or `webkit-line-clamp`. Sensitive items show `•••••• (sensitive)` in italic; span-masked items show the non-sensitive ranges with `•` in sensitive positions.
  - *Image*: thumbnail only (no text label) — Maccy parity.
  - *File*: `FileChip` component with filename, MIME, and Save As / Copy buttons.
- **Right slot** (fixed width, never shifts layout):
  - Origin-device badge: "This device" (accent) or 8-char UUID prefix (faint) for synced items.
  - Source-app chip: 32×32 PNG app icon + short name derived from bundle ID (e.g. `com.google.Chrome` → "Chrome").
  - Relative timestamp (e.g. "2m ago").
  - Hover action buttons (opacity 0 at rest, 100 on hover): **Eye** (preview), **Pin/Unpin**, **Delete** (red).

### KindChip — text content-type classifier

The daemon's `kind` field classifies text items into: `TEXT` | `URL` | `EMAIL` | `PHONE` | `COLOR` | `JSON` | `CODE` | `NUMBER` | `PATH`. Each maps to a distinct colour chip. Falls back to a content_type-derived label on older daemon builds.

### Filter & sort toolbar

- **Search input** (right of toolbar): client-side substring match across `preview` text; case-insensitive. No-results empty state shown for non-matching queries.
- **Device-filter dropdown** (shown only when more than one origin device is present in the loaded slice): "All devices" (default) or per-device UUID (labelled "This device" for own device, 8-char prefix for others).
- **Sort-mode toggle** (shown only when multiple devices present): "By time" (default, daemon recency order) or "By device" (own device first, then alphabetical by UUID, preserving recency within each group).
- **Attach button** (paperclip): opens the browser file picker to ingest one or more files directly into clipboard history via `add_file_item`.
- **OS file drag-drop**: dragging files from Finder onto the History window shows a dashed drop-zone overlay and ingests all dropped files via `add_file_item`.

### Keyboard navigation

Focus must be on the list (`tabIndex=0`). Supported keys:
- `ArrowDown` / `ArrowUp`: move single-selection highlight; scrolls the selected row into view.
- `Enter`: copy and move the selected item to the pasteboard (fires sound/notification via same gates as click).
- `Backspace` / `Delete`: delete the selected item.
- `Cmd+A` / `Ctrl+A`: select all visible (filtered) items (enters multi-select mode).
- `Escape`: clears multi-selection if active, otherwise clears single-selection.

### How to reach

Sidebar tab "History" (default view on launch). Keyboard shortcut: navigate the sidebar with mouse or Tab.

### Limitations

- Search is client-side substring over the loaded page (max 1,000 items); not a full FTS query.
- Load-more is disabled during active search — items beyond the first page are not searched.
- Row-click always copies and moves to top; there is no "select only" action on mouse without entering multi-select mode.
- The device-filter and sort-mode controls only appear when items from multiple devices are present in the loaded slice.

---

## 2. Quick-Paste Popup

**Purpose:** A lightweight floating overlay for instant access to recent clips, activated by a global hotkey.

### What it does

- Loads up to **50 items** from the daemon on each show (refresh on focus).
- **Search bar** (44 px header): always auto-focused on show; placeholder "Search clipboard…"; result count badge ("N of M" when searching, total count otherwise).
- **Fuzzy search**: scored via `fuzzyMatch` (`lib/fuzzy.ts`); matched characters highlighted in accent blue without weight change (prevents layout shift). Items are ranked by fuzzy score when a query is active.
- **Row list**: each row shows a type chip (`T` / `↗` / image frame / `</>` / dot), preview text, source-app chip (icon + label), relative timestamp, and a pin/unpin button on hover.
  - Image rows display a thumbnail only.
  - Sensitive items display `••••••••`.
  - Span-masked items display redacted spans.
  - Pinned rows have an amber bookmark badge (at rest) that transforms to a filled pin button on hover.
- **Click a row** or press `Enter`: copies the item to the pasteboard via `copy_item`, hides the popup, activates the previously-frontmost app, and synthesises `Cmd+V` to paste (`paste_to_frontmost`).
- **`⌘1`–`⌘9` keycaps**: when no search query is active, the first 9 rows show a `⌘N` keycap; pressing `⌘N` pastes the Nth item directly.
- **Footer hint bar**: shows `↑↓ navigate`, `⏎ paste · Esc close`, and a gear button that opens the main window Settings view.
- **Settings gear**: hides popup, surfaces main window, emits `"open-settings"` event so `App.tsx` navigates to the Settings view.
- **Blur-to-dismiss**: the popup auto-hides when it loses focus (blur event), guarded against double-activation.
- **Memory management**: after hiding, JS heap (image LRU cache + item list) is freed via `window.__copypasteFreeMemory()` to reclaim idle RSS. The WebView itself stays alive (warm start on next show).
- **Polling while visible**: refreshes items every 3 s while the popup is in the foreground so newly-copied content appears without re-opening.

### Positioning

Three modes (configurable in Settings — Shortcuts):
- **Cursor** (default): appears 8 px offset from the current cursor position, clamped to the monitor frame.
- **Center**: centered on the primary monitor.
- **Menubar**: below the macOS menu bar, right-aligned to the primary monitor's right edge (8 px inset).

### Dimensions

403 × 624 logical pixels (set in `tauri.conf.json`; no resize). Always-on-top, no window decorations, transparent background, `NSVisualEffectMaterial.HudWindow` vibrancy with 12 pt corner radius.

### How to reach

Global hotkey (default `Cmd+Shift+V`; configurable). Pressing the hotkey again hides the popup (toggle).

### Limitations

- Maximum 50 items shown; no load-more.
- No per-item delete from the popup.
- No keyboard shortcut to navigate to the full history from the popup (only a gear icon to Settings).
- `⌘1–9` keycaps are disabled while a search query is active.

---

## 3. Image Clips

**Purpose:** Capture, store, and display image clipboard items.

### What it does

- Images are captured by the daemon's NSPasteboard poller; stored encrypted via XChaCha20-Poly1305 in SQLCipher.
- In the history list, image rows show a **thumbnail** fetched via `get_item_thumbnail` (returns a pre-computed WebP thumbnail, base64-encoded). Falls back to `get_item_image` (full-resolution data URI) when no thumbnail is available.
- Thumbnail display respects the `imageMaxHeight` preference (1–200 px); the image is aspect-preserving, never upscaled, max width 340 px.
- In the popup, the same thumbnail logic applies with the same `imageMaxHeight`.
- **Details modal** (Eye button in history): shows the full-resolution image via `get_item_image` (max height 600 px in the modal), content type, source app bundle ID, and timestamp.
- **Copy-back**: clicking an image row copies the full-resolution image back to the macOS pasteboard via `copy_item`.
- **Sync size gate**: items exceeding the configured `max_image_size_bytes` cap are stored locally but flagged `too_large_to_sync` and skipped for P2P/cloud sync; the amber warning triangle indicator appears on the row.

### How to reach

Automatically captured when any app writes an image to the pasteboard. Visible in History and Popup.

### Limitations

- No native image preview on hover — only the thumbnail in the row.
- Thumbnail quality fixed at capture time by `image_quality` config (1–100).

---

## 4. File Clips

**Purpose:** Capture and manage file clipboard items.

### What it does

- Files are stored as binary blobs in the daemon DB (up to `max_file_size_bytes`, hard cap 100 MiB).
- In the history list, file rows show a `FileChip` component: amber document icon + filename parsed from the `[file: <name>]` preview placeholder + "Save As" and "Copy" buttons.
- **Save As**: downloads the file binary via `get_item_file` (returns `{ filename, mime, data_b64 }`) through the browser download API.
- **Copy-back**: copies the file back to the macOS pasteboard via `copy_item`.
- **File ingestion — two paths**:
  1. **File picker** (paperclip button in History toolbar): `<input type="file" multiple>` → reads bytes via `File.arrayBuffer()` → `add_file_item` IPC.
  2. **OS drag-drop** (drag files from Finder onto the History window): `getCurrentWebview().onDragDropEvent` → reads bytes via `fetch("file://…")` → `add_file_item` IPC.
- **Details modal**: shows `FileChip` + metadata table (name, type, copied timestamp, source bundle ID).
- **Sync gate**: files > ~8 MB are kept locally but not synced over P2P/relay (skipped with a warning); the amber `SyncBlockedIndicator` is shown on those rows.

### How to reach

Automatically captured from pasteboard, or manually ingested via the paperclip button or Finder drag-drop onto the History view.

### Limitations

- No inline preview of file content (binary or text); only filename + metadata.
- Drag-drop MIME is inferred from the `fetch` response `content-type` header (best-effort).

---

## 5. Sensitive / Private Masking

**Purpose:** Protect passwords and other secrets from casual shoulder-surfing.

### What it does

There are two independent masking mechanisms:

#### 5a. Full-item sensitive flag (`is_sensitive`)
- When `is_sensitive = true`, the history row displays `•••••• (sensitive)` in italic.
- In the popup, the item displays `••••••••` and cannot be fuzzy-searched (treated as `"••••••••"`).
- The item is still copyable by clicking the row.
- Sensitive items are auto-wiped after `sensitive_ttl_secs` (configurable, default 30 s).

#### 5b. Span-level masking (`sensitive_spans`, `maskSensitive` pref)
- The daemon can return `sensitive_spans: [[start, end], …]` — character offset ranges within the preview that contain sensitive text (e.g. a credit card number within a larger string).
- When `maskSensitive` is enabled (default on), `applySpanMasking()` (`lib/masking.ts`) replaces only the sensitive ranges with `•` characters while showing the rest of the preview.
- The full original text is still copied when the row is clicked.
- Span masking is applied in both the history list and the popup.

#### 5c. Private Mode
- Toggled via Settings → General → "Private mode" or the tray menu "Private Mode" check item.
- When enabled, the daemon stops capturing new clipboard items. Existing history is still accessible.
- The setting persists to `config.toml` via the daemon; the tray checkmark is synchronised bidirectionally.

### How to reach

- Masking preferences: Settings → General → "Mask sensitive data" toggle.
- Private mode: tray menu or Settings → General.
- Sensitive TTL: Settings → Storage → "Sensitive auto-wipe" slider.

### Limitations

- There is no visual distinction between "item is fully sensitive" and "item has sensitive spans" in the row's left column.
- The search/filter in History does not exclude sensitive items — they just show the masked preview as the search target.

---

## 6. Pin & Reorder

**Purpose:** Keep frequently-used clips at the top of the list.

### What it does

- **Pin / Unpin** any item via the hover Pin button (bookmark icon) in the history row or popup row.
- Pinned items are always returned first by the daemon; within the pinned section they are sorted by `pin_order`.
- **Drag-to-reorder** (history list only, not popup): pinned rows have a six-dot drag handle that appears on hover. Dragging shows an accent-blue inset border above or below the drop target. On drop, `reorder_pinned` is called with the complete new ordered ID list; the UI applies an optimistic reorder immediately and reconciles on the next poll.
- Bulk pin/unpin is available via multi-select (see §7).

### How to reach

Hover over any history row → click the bookmark icon. Or checkbox-select items → Bulk action bar → Pin / Unpin.

### Limitations

- Drag-to-reorder is only available in the main history window, not in the popup.
- The pin button in the popup does not support drag reorder.

---

## 7. Multi-Select & Bulk Actions

**Purpose:** Operate on multiple clipboard items at once.

### What it does

- **Enter selection mode** by clicking any row's checkbox (revealed on hover) or pressing `Cmd+A`.
- A **Bulk Action Bar** appears above the list showing: item count, Select All / Deselect All, and action buttons: Copy, Pin, Unpin, Delete, Clear.
- **Bulk Copy**: copies the first selected item to the pasteboard via `copy_item`; if multiple non-sensitive text items are selected, their previews are concatenated with newlines and written to the browser clipboard API (best-effort). Fires the copy sound/notification.
- **Bulk Pin / Unpin**: calls `pin_item` sequentially for each selected item.
- **Bulk Delete**: calls `delete_item` sequentially; shows partial failure count ("Deleted N/M (K failed)").
- **Escape**: exits selection mode and clears all selections.
- The selection auto-clears when the last checkbox is unchecked.

### How to reach

Click any row's checkbox (visible on hover) in the History view.

### Limitations

- Bulk operations are sequential per-item IPC calls, not a single batched daemon request.
- Bulk copy of images copies the first selected item only (no multi-image copy).

---

## 8. Details Modal

**Purpose:** Full-detail preview of a single clipboard item.

### What it does

- Triggered by the **Eye button** (hover action) on any history row.
- **Text items**: a scrollable `<pre>` block with the full `preview` text; monospace, selectable, word-wrapped. Footer shows content type, source bundle ID, timestamp.
- **Image items**: full-resolution image via `get_item_image` (max 600 px tall, aspect-preserving), loading state, fallback "Image unavailable".
- **File items**: `FileChip` with Save As / Copy + metadata table (name, type, copied time, source bundle ID).
- Closes on `Escape`, backdrop click, or the × button.
- Not available in the popup.

### How to reach

History view → hover any row → click Eye icon.

---

## 9. Menu-Bar Tray Icon

**Purpose:** Always-accessible macOS menu-bar entry point.

### What it does

Menu structure (left-click or right-click):
1. **Open CopyPaste** — shows and focuses the main window.
2. **Recent** submenu — up to **10 most-recent items** truncated to 40 chars. Clicking any item calls `copy_item` on it and fires the copy sound + notification. Falls back to a disabled "No recent items" placeholder when the daemon is offline or history is empty.
3. **Private Mode** — a `CheckMenuItem` that toggles private mode via `set_private_mode` IPC. The check state is bidirectionally synchronised: tray changes broadcast to the Settings window; Settings changes update the tray via the `"private-mode-changed"` Tauri event. Startup race is handled by a background poller (`spawn_tray_private_mode_resync`) that waits for the daemon to be stable before committing the checkmark.
4. **Separator**
5. **Quit CopyPaste** — calls `app.exit(0)`, which stops the daemon.

The **Recent submenu** is rebuilt every 5 s by a background thread (`spawn_tray_recent_resync`). The same thread checks for newly-captured items and fires a `UNUserNotificationCenter` banner (if `notify_on_copy` is enabled).

The tray icon is a 32×32 PNG template image (`assets/tray-icon-32.png`), shown in macOS template mode (adapts to light/dark menu bar).

### How to reach

Always visible in the macOS menu bar while the app is running.

### Limitations

- Recent submenu shows text preview only; images and files are truncated to their `[file: ...]` or similar preview string.
- Clicking a Recent item does not paste — it only puts the item on the pasteboard.

---

## 10. Global Hotkey

**Purpose:** Show/hide the Quick-Paste popup from any app without switching windows.

### What it does

- Default accelerator: `CmdOrCtrl+Shift+V`.
- Registered via two complementary mechanisms:
  1. **`tauri-plugin-global-shortcut`**: OS-level hotkey registration (cannot override OS-reserved keys like `Cmd+Space`).
  2. **CGEventTap** (macOS only, requires Accessibility permission): `kCGHIDEventTapLocation`-level event interception that can override any key combination including OS-reserved ones; activated when the user grants Accessibility permission.
- The hotkey **toggles** the popup: pressing it while the popup is visible hides it.
- On hide, the previously-frontmost app is re-activated and `Cmd+V` is synthesised (`paste_to_frontmost`).
- The shortcut value persists to `ui-config.json` in the Tauri app config directory.
- **Shortcut recording**: in Settings → Shortcuts, clicking the accelerator field and pressing a key chord captures it via CGEventTap recording (macOS) or keyboard event parsing (fallback). Physical key code (`e.code`) is used — layout-independent.
- **Reset to default** button resets to `CmdOrCtrl+Shift+V`.

### Popup position modes

Configured in Settings → Shortcuts (`get_popup_position` / `set_popup_position`):
- `cursor` (default): near the cursor with 8 px offset, clamped to monitor.
- `center`: centered on the primary monitor.
- `menubar`: below the macOS menu bar, right edge of primary monitor.

### How to reach

Press `Cmd+Shift+V` (default) from any app. Configure in Settings → Shortcuts.

### Limitations

- OS-reserved shortcuts (`Cmd+Space`, `Cmd+Tab`, etc.) cannot be registered via the plugin path; CGEventTap can override them but requires Accessibility permission.
- Input Monitoring permission may also be required on macOS 10.15+ for the tap to install.

---

## 11. macOS Notifications

**Purpose:** System-level feedback when a clipboard item is copied.

### What it does

- Posted via `UNUserNotificationCenter` from inside the `CopyPaste.app` bundle so the banner shows the app icon (replaces the old `osascript` path which showed a generic Script Editor icon).
- **Notification content**:
  - *Text copied*: title "Text Copied", body = first ~160 chars at a word boundary, `…` if truncated.
  - *Image copied*: title "Image Copied", body "Image".
  - *File copied*: title "File Copied", body = filename extracted from `[file: <name>]` preview.
  - *Unknown type*: title "Copied", body = truncated preview.
- Fired from **three sources**:
  1. Row-click copy in History view (when `notifyOnCopy` pref is on).
  2. Row-click copy in the popup (when `notifyOnCopy` pref is on).
  3. Tray-Recent-item click.
  4. Background capture detected by `spawn_tray_recent_resync` (polls every 5 s, fires only if `notify_on_copy` config is on).
- First call triggers the macOS system permission prompt (`.alert` + `.badge` authorization).
- Each notification has a unique ID so banners don't coalesce.

### How to reach

Enabled/disabled via Settings → General → "Show notification on copy" toggle.

---

## 12. Copy Sound

**Purpose:** Auditory feedback when an item is copied — Maccy parity.

### What it does

- Plays `NSSound "Tink"` (a short soft system sound) via `objc2-app-kit`.
- Fired on the same events as notifications (row-click copy, popup copy, tray copy).
- Non-blocking and failure-safe: missing sound or unavailable audio device are silently ignored.
- Default: **enabled** (default `true` in store.ts; daemon config default may differ).

### How to reach

Enabled/disabled via Settings → General → "Play sound on copy" toggle.

---

## 13. Devices & Pairing

**Purpose:** Manage sync peers, initiate and confirm device pairing.

The Devices view has four functional areas:

### 13a. This-Device Card

Displayed at the top of the Devices view. Shows:
- Green "online" status dot + "This Mac" badge.
- Device name (e.g. "Dmytro's MacBook Air").
- Model (e.g. "MacBook Air").
- OS version (e.g. "macOS 15.5").
- App/daemon version.
- Local IP address.
- Public (WAN) IP address (fetched async via STUN; null until resolved).

Polled every 10 s so Local IP and Public IP stay fresh.

**Source:** `get_own_device_info` IPC.

### 13b. QR Code Pairing

- A **scannable QR code** is auto-generated on Devices tab mount via `pair_generate_qr` / `pairing_qr_svg` (Tauri-direct; daemon generates a `CPPAIR1.…` PAKE token and the Tauri layer renders it as inline SVG).
- The token TTL is 120 s; the UI auto-refreshes 15 s before expiry (1 s countdown tick while visible; visibility-gated to avoid burning single-use tokens when backgrounded).
- **Blur-reveal**: the QR starts visually blurred (`blur(12px)`); first click reveals it. Subsequent clicks regenerate a fresh code.
- **Payload**: the raw `CPPAIR1.…` string is passed to the SVG renderer; the other device scans it to pair automatically without manual entry.

**Limitation:** only P2P pairing material is embedded; full Supabase/relay provisioning is not yet included.

### 13c. LAN Discovery + SAS Pairing

- **Discovered-devices list**: polls `list_discovered` every 3 s; shows unpaired LAN peers visible via mDNS-SD. Each row shows device name, resolved IP addresses, certificate fingerprint, and a "Pair" button.
  - "Pair" is disabled for v1 peers that don't advertise a bootstrap port (`bport === null`).
- **Manual rescan** button: calls `rescan_discovered` (forces a daemon mDNS re-browse) and refreshes the list.
- **SAS pairing flow** (initiator path):
  1. Click "Pair" → `pair_with_discovered` is called; rate-limited (error shown if another pairing is in flight).
  2. **SAS modal** opens polling `pair_get_sas` every 700 ms.
  3. When state = `awaiting_sas`, a **6-digit numeric SAS code** is displayed prominently (monospace, 28 px, clickable to copy).
  4. Peer metadata available at SAS time is shown (name, IPs, fingerprint).
  5. Two buttons: **"Match"** (accept) and **"Doesn't match"** (reject). Confirming calls `pair_confirm_sas`.
  6. Terminal states: `confirmed` → "Paired ✓", `rejected` / `aborted` / `timed_out` → error message.
  7. Closing the modal mid-flow calls `pair_abort`.

### 13d. Incoming (Responder) Pairing

- A background Tauri thread (`spawn_incoming_pairing_poller`) polls `pair_get_sas` every 1 s regardless of which tab is active.
- When `state = "awaiting_sas"` and `role = "responder"`:
  1. The main window is brought to the foreground (`show_main`).
  2. A system notification is posted: "CopyPaste — Pairing request" / "A device wants to pair…".
  3. A `"incoming-pairing"` Tauri event is emitted; `App.tsx` switches to the Devices tab and passes the SAS status to `DevicesView` via `incomingPairing` prop.
  4. The SAS modal opens pre-seeded with the inbound status (no "Connecting…" flash).
- De-duplicated: the notification fires only once per distinct SAS code to avoid banner spam.

### 13e. Paired-Peer Management

Each paired peer shows:
- Online/offline status dot with "last seen Xm ago" tooltip (1 s live tick).
- Device name (fallback: `Device <8-char fingerprint>`).
- Model, OS version, app version, Local IP, Public IP, "Paired" timestamp, "Last sync" timestamp.
- **Unpair** button: removes the peer from the P2P allowlist (`unpair_peer`); does not rotate the sync key.
- **Revoke** button: opens a confirm dialog with a passphrase field. Two actions:
  - *Revoke only* (P2P): blocks the peer from future P2P connections (`revoke_peer`); does NOT cut off cloud/relay sync.
  - *Revoke & Rotate* (P2P + cloud): calls `revoke_and_rotate` — revokes from P2P AND rotates the shared sync key to a new passphrase (the revoked device's old key becomes useless; remaining devices must re-provision).
- **Revoke All** button: calls `revoke_all_peers` with a confirmation dialog; revokes all paired devices from P2P.
- Peers list polled every 10 s to refresh the online dot.
- Self is filtered from the peers list (own fingerprint excluded).
- Duplicate fingerprints are de-duplicated client-side.

---

## 14. Settings — General Tab

**Purpose:** Core behavioural toggles and daemon management.

### Toggles (all persist to daemon `config.toml` via `set_config` when "ready")

| Setting | Default | What it does |
|---|---|---|
| Private mode | off | Stops clipboard capture while enabled; bidirectional sync with tray |
| Play sound on copy | on | Fires `NSSound "Tink"` on copy events |
| Show notification on copy | on | Posts `UNUserNotificationCenter` banner on copy events |
| Mask sensitive data | on | Applies `applySpanMasking` to `sensitive_spans` in history + popup |

### Privacy & capture section

| Setting | Default | What it does |
|---|---|---|
| Discover public IP | on | Allows a one-off STUN request to learn this device's WAN IP for the device-info card |
| Paste as plain text | off | Strips RTF/HTML when pasting — writes plain text only |
| Excluded apps | (empty list) | Bundle IDs (e.g. `com.1password.1password`) whose clipboard is never captured; add via text input + Add button or Enter key; remove via × button |

### Daemon section

- **Version**: shows `build_version` from `status` IPC ("Not running" if offline).
- **Restart** button: calls `restart_daemon` (SIGTERM + respawn); reloads Settings on completion.

---

## 15. Settings — Display Tab

**Purpose:** Visual preferences for the history list and popup.

### History list

| Setting | Default | Range | What it does |
|---|---|---|---|
| Preview lines | 1 | 1–6 | Number of text preview lines per clip in the main History window |
| Image preview height | 40 px | 1–200 px | Max height of image thumbnails in the history list and popup |

### Popup appearance

| Setting | Default | Range | What it does |
|---|---|---|---|
| Preview lines | 1 | 1–6 | Number of text preview lines per clip in the Quick-Paste popup (independent from main window) |

### Window

| Setting | Default | What it does |
|---|---|---|
| Translucency / vibrancy | on | Native macOS `NSVisualEffectMaterial.Sidebar` vibrancy + CSS `backdrop-filter` on the main window; turning off switches all surfaces to solid opaque backgrounds |

All display preferences persist to `localStorage` (`copypaste-ui-prefs-v1`); they do not require the daemon.

---

## 16. Settings — Sync Tab

**Purpose:** Configure P2P LAN sync and cloud (Supabase) sync.

### Sync status banner

If Supabase is configured, a green "Connected ✓ — signed in as <email>" banner appears at the top.

### Local sync (P2P)

| Setting | Default | What it does |
|---|---|---|
| Enable P2P (LAN) sync | on | Toggles `p2p_enabled` in `AppConfig`; restarts daemon to take effect |
| Sync on Wi-Fi only | off | Restricts P2P sync to Wi-Fi interfaces only |

Toggling P2P triggers an in-place daemon restart; a transient "Restarting sync service…" message is shown while the restart is in flight. The control is disabled during the restart to prevent double-toggles.

### Cloud sync (Supabase)

| Field | What it does |
|---|---|
| Supabase URL | Project URL (e.g. `https://xyz.supabase.co`); saved to `AppConfig` |
| Supabase anon key | Project anon key; stored as a password field; shows "set ✓" when already configured and left blank |
| Relay URL | Optional HTTP relay URL for store-and-forward sync; saved with cloud settings |
| Sync passphrase | Shared encryption passphrase; auto-saved on Enter or focus-out; must match on all devices |

**Save** button persists URL + key + relay + P2P flag then restarts the daemon so new credentials take effect.

**Test connection** button saves config first, then calls `cloud_test_connection` which runs a staged probe (config → url → auth → network → schema → rls → done) and reports the result.

### Sync status detail

If the daemon returns sync status, a read-only panel shows:
- Passphrase set: ✓ / —
- Supabase configured: ✓ / —
- Signed in: ✓ / —
- Last sync: relative time (e.g. "3m ago") or "Never".

---

## 17. Settings — Shortcuts Tab

**Purpose:** Configure the global popup hotkey and its display position.

| Setting | Default | What it does |
|---|---|---|
| Open popup shortcut | `Cmd+Shift+V` | Global hotkey to show/hide the Quick-Paste popup |
| Popup position | Cursor | Where the popup appears: Cursor / Center / Menubar |

**Shortcut capture**: click the accelerator field → field shows "Press a shortcut…" → press the desired combo → field updates; click Save to persist. A reset-to-default button (circular arrow icon) is available.

On macOS, shortcut capture uses physical `e.code` (layout-independent). OS-reserved keys cannot be bound via the plugin path; CGEventTap handles them if Accessibility is granted.

---

## 18. Settings — Storage Tab

**Purpose:** Configure per-type size caps, local storage quota, and data management.

### Storage limits (stepped sliders; save on mouse-up / touch-end / key-up to avoid IPC spam)

| Setting | Default | Steps | What it does |
|---|---|---|---|
| Max clip text size | 10 MB | 1, 2, 5, 10, 15, 25, 50, 100 MB | Items larger than this cap are not stored |
| Max clip image size | 64 MB | 5, 10, 25, 64, 128, 256, 512 MB | Images larger than this cap are not stored |
| Max clip file size | 100 MB | 8, 16, 25, 50, 100 MB | Files larger than this cap are not stored; files > 8 MB are kept locally but not synced |
| Local storage limit | 10 GB | 1, 2, 5, 10, 25, 50 GB | Overall SQLCipher DB size cap; older items are pruned when this is exceeded |
| Sensitive auto-wipe | 30 s | 10 s, 30 s, 1 min, 5 min, 15 min, 1 hour | Time after capture before a sensitive item is automatically deleted |
| Image quality (1–100) | 100 | continuous | JPEG/WebP quality for image storage |

Each slider shows inline per-field feedback ("Saved" or error message) after committing.

### Data section

**Clear clipboard history**: two-step confirmation ("Clear history…" → "Delete all history? Yes / No"). Calls `delete_all`; reports count deleted.

---

## 19. Settings — Advanced Tab

**Purpose:** Placeholder for future advanced settings.

Currently shows: "Advanced daemon and storage limits will appear here in a future release."

No functional controls.

---

## 20. Sync Status Indicators

**Purpose:** Surface sync health without requiring the user to open Settings.

### In the history list

- **"Too large to sync" indicator** (amber triangle on row): item exceeds the sync size cap and was not sent to peers.
- **Origin-device badge**: "This device" (accent) or compact UUID prefix for synced items from other devices.

### In Settings → Sync

- Green connection banner with account email.
- Status panel: passphrase set, Supabase configured, signed in, last sync time.
- Test connection result (staged diagnostic message).

### In Settings (global banners)

- **Stale-daemon banner** (amber): shown when the running daemon's semver is strictly older than the app's semver (i.e., survives an upgrade). Shows daemon build version. Offers "Restart" button and "Dismiss".
- **Offline banner** (neutral): daemon not running. Shows "Daemon not running — clipboard sync paused." with Restart and Retry buttons.
- **Degraded banner** (amber): daemon up but DB unavailable (key mismatch). Shows reason string. Directs user to History to reset the database.

### In the main window (App.tsx banners)

- **Daemon spawn error banner** (red, non-dismissible): shown when `ensure_daemon_running_async` fails to start the daemon binary. Shows the raw error string.
- **Stale-daemon banner** (amber, dismissible): same as in Settings; also shown in the main window header on launch.
- **Accessibility permission banner** (amber, dismissible): shown when `check_accessibility_permission()` returns false. Offers "Open Settings" (opens System Settings → Accessibility) and "Dismiss". Polled every 3 s; auto-dismisses when permission is granted.

---

## 21. Degraded / Recovery States

**Purpose:** Guide the user through failure recovery.

### DB unavailable (degraded mode)

Condition: daemon up, DB cannot be opened/decrypted (`degraded: true` / `ready: false` from `status` IPC).

- History view: shows "Clipboard database can't be opened" with the degraded reason, an explanation, and a **"Reset database (erases local history)"** button with a two-step confirm ("Erase and reset?" + "Yes, erase" / "Cancel"). On success, the DB is wiped and the daemon recovers in-place.
- Settings view: amber degraded banner.
- About view: shows "Degraded" state with reason.

### Daemon offline

- History view: EmptyState with "Clipboard service offline — The daemon is not running." + `RestartDaemonButton`.
- Settings view: orange offline banner + Restart + Retry.
- About view: shows "Offline".
- Popup: EmptyState "Clipboard service offline — Restart it from Settings."

### Daemon stale (survived an upgrade)

- Main window header and Settings view: amber banner with daemon build version + "Restart" button.

---

## 22. App Lifecycle & Daemon Management

**Purpose:** Ensure the daemon is always running and cleanly shut down.

### Launch

- On app launch, `ensure_daemon_running_async` is called on a background thread; the tray and main window render immediately (non-blocking).
- The bundled `copypaste-daemon` binary is spawned as an app-owned child process (no launchctl). Spawn errors are stored in `DaemonSpawnError` state and emitted as a `"daemon-spawn-result"` Tauri event.
- The persisted `launch_at_login` preference is applied idempotently via `tauri-plugin-autostart` (macOS LaunchAgent).
- Default: **launch at login enabled**.

### Quit

- Closing the main window **hides it to the tray** (standard macOS pattern) and does NOT stop the daemon.
- Tray "Quit CopyPaste" → `app.exit(0)` stops the background threads (pairing poller, tray resync), uninstalls the CGEventTap, then calls `stop_daemon` (SIGTERM + reap).

### Restart

- `restart_daemon` Tauri command: SIGTERM the child, wait for socket release, respawn the bundled binary. Called by: Settings Restart button, stale-daemon banner, P2P toggle, Save cloud config.
- `RestartDaemonButton` component handles in-flight state, success/error feedback.

### Launch at login

- Persisted to `ui-config.json`. Controlled via Settings → General → Daemon section (via the Restart row — the toggle itself is exposed programmatically but not yet as a dedicated UI row; it is applied on launch).

> **Note**: the explicit "Launch at login" toggle is wired in `lib.rs` (`get_launch_at_login` / `set_launch_at_login` commands) but there is no dedicated Settings row in `SettingsView.tsx` exposing it to the user. The behavior defaults to `true` (enabled) and can only be changed programmatically.

---

## 23. Logs View

**Purpose:** Developer/diagnostic access to daemon log output.

### What it does

- Reads the last **500 lines** from `~/Library/Logs/CopyPaste/` via `read_logs` Tauri command.
- Displays the log path below the title.
- Shows the log content in a scrollable `<textarea>` (auto-scrolled to the bottom on load).
- **Refresh** button: re-reads the last 500 lines.
- **Export** button: triggers a browser download of the log content as `copypaste-daemon.log`.

### How to reach

Sidebar → "Logs" tab.

---

## 24. About View

**Purpose:** App metadata and daemon health at a glance.

### What it does

- Displays the app version pulled at runtime from the Tauri bundle (`@tauri-apps/api/app` `getVersion()`).
- Shows three feature highlights: "End-to-end encrypted local history", "Peer-to-peer device sync", "Automatic sensitive-data redaction".
- Daemon status badge: "Connected ✓" (green), "Degraded" (with reason), or "Offline" (neutral).

### How to reach

Sidebar → "About" tab.

---

## 25. Notable Gaps Observed

The following behaviours are absent from the code and would plausibly be expected by users:

1. **No "Launch at login" toggle in the Settings UI.** The underlying Tauri commands (`get_launch_at_login` / `set_launch_at_login`) and the autostart plugin are wired, but no `SettingsRow` in the General tab exposes this to the user. It defaults to `true`.

2. **No full-text search in History.** Search is a client-side substring match over the loaded page (max 1,000 items). The daemon has FTS5 infrastructure but it is not called from the UI; items beyond page 1,000 are unsearchable.

3. **No keyboard navigation in the popup pin/unpin.** The popup supports keyboard navigation for copy (`Enter`), dismiss (`Esc`), and `⌘1–9`; but there is no keyboard shortcut to pin/unpin the highlighted item.

4. **No cross-device account-mismatch warning in Sync settings.** The `DaemonStatus` type has a TODO comment (`// TODO(task-7): expose supabase_account_id`) noting that the daemon does not yet emit the signed-in account ID; the UI cannot warn when two paired devices use different Supabase accounts.

5. **QR pairing does not provision cloud/relay credentials.** Scanning the QR on another device establishes the P2P mTLS identity only. Supabase URL, anon key, relay URL, and passphrase must still be entered manually on each device.

6. **No "Revoke & Rotate" without a passphrase.** Cutting a device off from cloud/relay sync requires entering a new passphrase (key rotation). There is no "revoke cloud/relay only without a new passphrase" affordance.

7. **No per-item "copy as plain text" option.** There is a daemon-level `paste_as_plain_text` config that applies globally, but no per-item override in the UI.

8. **No bulk export of history.** There is no way to export the full clipboard history as a file from the UI (only the daemon logs can be exported from the Logs view).

9. **Popup has no keyboard shortcut to open full History.** The popup footer gear opens Settings; there is no direct shortcut to open the main History view from the popup.

10. **"Advanced" settings tab is empty.** The tab renders a placeholder message; no controls are present.
