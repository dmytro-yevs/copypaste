# macOS Desktop App — Performance Audit

**Scope:** `crates/copypaste-ui/src/` (React/TS) + `crates/copypaste-ui/src-tauri/src/` (Tauri/Rust)
**Branch:** `feat/android-parity-v0.5.3` (audited 2026-06-04)
**Nature:** Read-only findings — no code was changed.

---

## 1. Rendering

### R-1 `HistoryRow` is not memoized — re-renders on every poll (HIGH / M)

**File:** `src/views/HistoryView.tsx:466` (`HistoryRow` function component)

`HistoryRow` is an un-memoized function component. `HistoryView` runs a `load()` call every **1 200 ms** (ACTIVE_MS). Each successful poll calls `setItems(incoming)` only when the signature changes, but it also calls `setOwnDeviceId`, `setTotalCount`, `setLoadState`, and similar state setters unconditionally. Any of these triggers a `HistoryView` re-render which re-creates all callback lambdas passed to every visible `HistoryRow` as anonymous arrow functions (e.g. `onCopy={() => void handleCopy(entry.id)}`). Since props are new object references every render, even a `React.memo`-wrapped `HistoryRow` would re-render unless callbacks are stabilized too.

At 200 loaded items with ~15 rows in the viewport, every 1.2 s poll re-renders ~15 `HistoryRow` instances unnecessarily when nothing changed.

**Recommendation:** Wrap `HistoryRow` in `React.memo`. Stabilize all per-row callbacks by passing `entry.id` as a primitive and defining the handlers inside the memoized component (or use a row-index–based `useCallback` pattern). The `itemsSignature` diff guard in `load()` already stops `setItems` from firing when data is unchanged — but it does not stop the other state setters that still cause a parent re-render.

---

### R-2 `VirtualList` recomputes `offsets` (prefix-sum) on every render (MED / S)

**File:** `src/views/HistoryView.tsx:1093–1096`

```ts
const offsets = buildOffsets(
  items.map((it) => rowHeightFor(it, previewSize, imageMaxHeight))
);
```

`offsets` is re-computed inside the `VirtualList` render body with no memoization. For 200 items this allocates a 201-element array and runs 200 additions on every render, including the 1.2 s silent poll re-renders and every keystroke in the search box (which causes `filtered` to change). `buildOffsets` itself is O(n); for 1 000 items this is measurably wasteful.

**Recommendation:** Wrap in `useMemo` with `[items, previewSize, imageMaxHeight]` as deps.

---

### R-3 `filtered` recomputes for every poll even when search is empty (MED / S)

**File:** `src/views/HistoryView.tsx:1461–1490`

When `search` is empty and `sortMode` is `"recency"`, the `filtered` memo returns `items` unchanged after identity-checking — but the `useMemo` still runs and re-allocates `result` because `items` is a new array reference on every `setItems` call. The `.filter()` for the device filter runs even when `deviceFilter === "all"`.

Additionally, `search.trim()` is called twice inside the memo body (lines 1463 and 1465).

**Recommendation:** Add short-circuit: if `!search.trim() && deviceFilter === "all" && sortMode === "recency"`, return `items` directly (no allocation). Cache `search.trim()` in a local variable.

---

### R-4 Source-app chip uses an IIFE inside JSX on every row render (LOW / S)

**File:** `src/views/HistoryView.tsx:665–676` and `src/popup/Popup.tsx:744–760`

```tsx
{entry.app_bundle_id && (() => {
  const appLabel = sourceAppLabel(entry.app_bundle_id);
  return appLabel ? ( … ) : null;
})()}
```

The IIFE is re-executed on every render of each visible row. `sourceAppLabel` is a pure string function and cheap, but the IIFE pattern prevents React from diff-skipping and makes the intent opaque. The same pattern appears in `PopupRow`.

**Recommendation:** Hoist to a variable before the JSX return (or a small helper component). Minimal cost but cleans up hot-path render code.

---

### R-5 `DevicesView` runs a 1 s clock tick (`setInterval`) that re-renders the whole view (MED / M)

**File:** `src/views/DevicesView.tsx:706–715`

```ts
const [nowSecs, setNowSecs] = useState(() => Math.floor(Date.now() / 1000));
useEffect(() => {
  const id = setInterval(() => { setNowSecs(Math.floor(Date.now() / 1000)); }, 1000);
  …
}, []);
```

`nowSecs` is a top-level state in `DevicesView`. Every second it updates, which re-renders the entire `DevicesView` subtree — including the QR section, all `PeerRow` instances, and the discovered-devices list — solely to update the "Xm ago" tooltip string. This re-render is unnecessary for most of the tree.

**Recommendation:** Extract the live-clock tick into a dedicated child component (e.g. `LiveLastSeen`) that owns `nowSecs` internally and renders only the elapsed-time string. Or pass `liveLastSeenSecs` as a computed value to `PeerRow` using a context / separate atom so only the affected row re-renders.

---

### R-6 `KindChip` color string is computed with a long ternary chain on every render (LOW / S)

**File:** `src/views/HistoryView.tsx:233–241`

The nine-arm ternary runs every time a row renders. This is trivially cheap per call but runs on every visible row on every poll.

**Recommendation:** Replace with a static `Record<string, string>` lookup or a `Map`. Zero allocations, O(1) lookup.

---

## 2. Memory

### M-1 Full-res `getItemImage` is fetched uncached for the Details modal (HIGH / S)

**File:** `src/views/HistoryView.tsx:851–897` (`FullResImage` component)

Every time the user opens the Details modal for an image item, `getItemImage` is called, which returns the **full-resolution** base64 data URI via IPC. This is not cached anywhere — `FullResImage` uses local component state, discarding the bytes on modal close. On a retina display a 3 MB screenshot decoded to RGBA is ~40 MB of WebView bitmap memory. Re-opening the modal re-downloads and re-decodes.

The known 200–400 MB webview spike is driven by these uncached full-resolution images. The `ImageThumb` cache is thumbnail-only and does not help here.

**Recommendation:** Use the same `ImageThumb` / `imageCache` for the modal too, or add a separate full-res LRU cache with a tight budget (e.g. 2 entries). At minimum, reuse the already-fetched thumbnail as a low-res placeholder while the full-res loads.

---

### M-2 `btoa(String.fromCharCode(...Array.from(bytes)))` is O(n) and stack-unsafe for large files (HIGH / S)

**File:** `src/lib/ipc.ts:525`

```ts
const data_b64 = btoa(String.fromCharCode(...Array.from(bytes)));
```

`Array.from(bytes)` allocates a full JS array of numbers. The spread `...Array.from(bytes)` passes all elements as function arguments — for files larger than ~65 KB this will hit the call-stack limit (`RangeError: Maximum call stack size exceeded`). This is on the hot path for file upload via the file-picker and drag-drop ingest.

**Recommendation:** Use the chunked encoding pattern:
```ts
let binary = "";
const chunkSize = 8192;
for (let i = 0; i < bytes.length; i += chunkSize) {
  binary += String.fromCharCode(...bytes.subarray(i, i + chunkSize));
}
const data_b64 = btoa(binary);
```
Or use `Buffer.from(bytes).toString("base64")` if running in the Tauri Node-compatible environment, or the `FileReader.readAsDataURL` approach.

---

### M-3 `AppIcon` cache is never cleared on history clear (MED / S)

**File:** `src/components/AppIcon.tsx:15` and `src/views/HistoryView.tsx:1893`

When the user clicks "Clear All" (or the database is reset), `clearImageCache()` is called to drop thumbnail memory. However `iconCache` in `AppIcon.tsx` is a separate module-level map and is **never** cleared. Over a long session with many distinct source apps the 128-entry LRU cap is generous but the `inflight` promise map in `AppIcon.tsx` is also never cleared — an inflight fetch for a bundle ID whose item was deleted will still resolve and `cacheSet` its result.

**Recommendation:** Export a `clearIconCache()` from `AppIcon.tsx` (mirroring `clearImageCache`) and call it alongside `clearImageCache()` in the clear-all path (`HistoryView.tsx:1893`) and in the popup's `__copypasteFreeMemory` hook.

---

### M-4 `HighlightedText` allocates a `Set` and a new `nodes[]` array on every render (LOW / S)

**File:** `src/popup/Popup.tsx:93–123`

Every `PopupRow` render with an active query allocates `new Set(positions)` and a `nodes` array. With 50 items and a live-typing query this runs 50 times per keystroke.

**Recommendation:** Memoize `HighlightedText` with `React.memo` and stable prop references, or move the `Set` construction outside the render loop (pre-compute during the `filtered` `useMemo`).

---

### M-5 `imageCache` uses `data-URI string length` as the byte proxy (LOW / S)

**File:** `src/components/ImageThumb.tsx:54`

```ts
const bytes = uri !== null ? uri.length : 0;
```

A base64 data URI string length is ~1.37× the compressed image bytes, but the decoded RGBA bitmap held by the WebKit GPU layer is `width × height × 4` bytes — potentially 10–100× larger than the string. The 16 MiB string-budget cap does not bound the actual GPU/RAM cost. The comment at line 21 acknowledges this.

**Recommendation:** After thumbnail generation (pre-compute at capture, Plan B), store decoded px dimensions alongside the data URI so the cache can budget by `w × h × 4` rather than string length. Until then, consider halving `CACHE_BUDGET_BYTES` to 8 MiB to give a tighter bound relative to the GPU memory.

---

## 3. IPC Chattiness

### IPC-1 History poll at 1 200 ms is the highest-frequency IPC call in the app (HIGH / M)

**File:** `src/views/HistoryView.tsx:1403`

```
ACTIVE_MS = 1200   →  ~50 calls/min while HistoryView is visible
```

Each call:
1. Calls `invoke("ipc_call", { method: "history_page", params: { limit: 200, offset: 0 } })`
2. Tauri dispatches to `spawn_blocking`, which opens a **new** `UnixStream` socket connection
3. Serializes and writes the JSON request
4. Reads the newline-delimited response (up to 200 serialized `HistoryEntry` objects)
5. Deserializes the `serde_json::Value` in Rust, re-serializes to `IpcReply`, crosses the IPC bridge to JS, and is deserialized again in TS

Each poll transfers the full 200-item payload (~20–50 KB of JSON depending on preview lengths) even if nothing changed. The `itemsSignature` diff guard avoids a React re-render when data is identical, but it does not avoid the network+serialize cost.

**Recommendation (short-term):** Increase `ACTIVE_MS` to 2 000–3 000 ms. The popup already uses 3 000 ms with no UX complaint. History is a background view; users rarely need sub-second freshness there. This halves IPC load with zero feature cost.

**Recommendation (medium-term):** Add a lightweight `history_changed` push event from the daemon (Tauri event bridge) triggered only when the daemon actually captures a new item or processes a mutation. The UI subscribes and polls lazily; the 1.2 s interval becomes a backstop only.

---

### IPC-2 Every `ipc_call` opens a new Unix socket connection (MED / L)

**File:** `src-tauri/src/ipc.rs:71–110`

```rust
let stream = UnixStream::connect(&path)…
```

Every single IPC call — history poll, status check, get_config, thumbnail fetch — does a full `connect → write → read → close` cycle on a new Unix domain socket. This is cheap on macOS (microseconds for a local socket) but it means:
- `spawn_blocking` allocates a new OS thread-pool slot for every call
- The 1.2 s history + 10 s sync-status + 10 s peers polls + SAS 700 ms poll can overlap and each hold their own blocking thread

At peak (DevicesView open during pairing + HistoryView animating), up to 4 blocking threads are simultaneously waiting on socket IO.

**Recommendation:** Implement a persistent connection pool (even a single reusable connection protected by a `Mutex<Option<UnixStream>>` with reconnect-on-error) to eliminate the connect overhead. This also opens the door to pipelining multiple requests on one connection.

---

### IPC-3 Tray Recent-submenu background thread polls at 5 s with a full 10-item `history_page` (MED / S)

**File:** `src-tauri/src/lib.rs:1781–1844` (`spawn_tray_recent_resync`)

```rust
const REFRESH_INTERVAL: Duration = Duration::from_secs(5);
// … calls history_page(limit=10) + get_config every 5 s forever
```

This thread also calls `check_and_notify_new_capture` on the same 5 s tick, which issues **two additional IPC calls** (`history_page(limit=1)` + `get_config`). So every 5 s: 3 IPC calls just for the tray.

Additionally, `get_config` is fetched on every tick only to check `notify_on_copy`. This field changes rarely.

**Recommendation:** Cache `notify_on_copy` in a `std::sync::atomic::AtomicBool` updated only when the user saves Settings. For the tray poll, increase `REFRESH_INTERVAL` to 15–30 s (the tray Recent menu is a convenience, not a live indicator). Use the push-event approach (IPC-1) to drive tray updates on actual clipboard changes.

---

### IPC-4 `SyncStatusChip` polls `get_sync_status` + `list_peers` every 10 s (MED / S)

**File:** `src/components/SyncStatusChip.tsx:22,140`

```ts
const POLL_INTERVAL_MS = 10_000;
```

`SyncStatusChip` is mounted in the `Sidebar` (always visible when the main window is open). Every 10 s it issues two IPC calls: `get_sync_status` and `list_peers`. Combined with the DevicesView polls (OWN_INFO_POLL_MS = 10 s, PEERS_POLL_MS = 10 s) when the Devices tab is active, there are 4 IPC calls every 10 s just for device/sync metadata.

**Recommendation:** Share the peers list between `SyncStatusChip` and `DevicesView` via a lightweight context or Zustand slice, so only one poll is in flight at a time. Alternatively, have the SyncStatusChip subscribe to the same push events as the history view.

---

### IPC-5 Accessibility permission is polled every 3 s in App.tsx regardless of banner dismissal (LOW / S)

**File:** `src/App.tsx:195`

```ts
const interval = setInterval(() => { void check(); }, 3000);
```

This poll fires every 3 s for the entire app lifetime, even after permission is granted or the banner is dismissed. Each call invokes `check_accessibility_permission` which on macOS calls `AXIsProcessTrustedWithOptions` via the CGEventTap module — a CoreGraphics API.

**Recommendation:** Stop the interval once `axGranted` becomes `true` (use a `useEffect` cleanup or a flag). Also stop when `axDismissed` is true. The current code does stop the interval on unmount, but `App` never unmounts while the main window is open.

---

### IPC-6 Popup polls at 3 s even when the window has focus — items refresh on every re-open anyway (LOW / S)

**File:** `src/popup/Popup.tsx:199`

The popup already calls `refresh()` every time `onFocusChanged(focused=true)` fires (line 176). The 3 s interval is therefore redundant for the common case (user opens popup → picks item → closes popup). The interval only benefits the rare case where the popup stays open while the user copies something in a background app.

**Recommendation:** Consider increasing to 5–10 s, or dropping the interval entirely and relying on the focus-triggered refresh + a single background event (copy notification) to signal freshness.

---

## 4. Startup

### S-1 `history_page(200)` is the first IPC call on app launch — transfers up to ~200 items of JSON before the UI is painted (MED / M)

**File:** `src/views/HistoryView.tsx:1390–1393`

The `load()` callback fires in a `useEffect` on initial mount. With 200 items each containing a preview string, the JSON payload can be 20–100 KB. This IPC call is on the critical path to first paint of the history list.

**Recommendation:** Use a smaller initial page size (e.g. 50) and expand via load-more. Alternatively, start with an aggressive page size but render the first visible window (~10 rows) from the first partial response using streaming pagination.

---

### S-2 No code-splitting between popup and main window bundles (MED / M)

**File:** `vite.config.ts:36–43`

The Vite build produces two entry points (`main` and `popup`), but neither has explicit chunk-splitting configured. Both bundles currently include all React, Zustand, and @tauri-apps/api code. If either bundle imports a large shared module it is duplicated.

Check `dist/` bundle size: `ls -lh crates/copypaste-ui/dist/assets/`. If main bundle exceeds ~300 KB gzip there is room to split lazy views (DevicesView, SettingsView, LogView) from the eagerly loaded HistoryView.

**Recommendation:** Add `build.rollupOptions.output.manualChunks` to hoist `react`, `react-dom`, and `@tauri-apps/api` into a shared vendor chunk. Lazy-import DevicesView, SettingsView, AboutView, LogView via `React.lazy` (only HistoryView loads on startup).

---

### S-3 Popup WebView is lazy-created (good) but layout recalculates on every show (LOW / S)

**File:** `src-tauri/src/lib.rs:1005–1066` (`toggle_popup`)

The popup is lazy-created on first hotkey press (M1 — saves ~84 MB idle RSS). However, `position_popup` calls `win.available_monitors()` and `win.primary_monitor()` on every show, which are synchronous cross-process calls to the macOS window server.

**Recommendation:** Cache the monitor geometry between shows; invalidate only when a `NSScreenDidChangeNotification` equivalent fires (Tauri exposes `on_window_event` for `ScaleFactorChanged`).

---

## 5. Tauri/Rust Side

### RS-1 Every `ipc_call` allocates a `BufReader` wrapping a temporary `UnixStream` (MED / S)

**File:** `src-tauri/src/ipc.rs:90`

```rust
let mut reader = BufReader::new(&stream);
```

`BufReader` allocates an 8 KB read buffer by default. With ~50 IPC calls/min from the UI, this is 50 small allocations per minute. The stream itself also allocates. These are individually trivial but compound with the spawn_blocking overhead.

**Recommendation:** Part of the persistent-connection pool fix (IPC-2). With a pooled connection the `BufReader` allocation amortizes across all calls.

---

### RS-2 `spawn_tray_recent_resync` calls `ipc::call` (blocking) from a background thread that holds a `Mutex` guard (LOW / S)

**File:** `src-tauri/src/lib.rs:1600–1604`

```rust
let guard = state.0.lock()…
let submenu = guard.as_ref()…
// … then ipc::call which is also blocking, holding the guard
```

The `RecentSubmenu` Mutex is held while `ipc::call("history_page", …)` blocks on socket IO (up to 10 s timeout). Any other thread trying to touch the submenu (e.g. a menu-click handler dispatching `rebuild_recent_submenu` from `on_menu_event`) will deadlock until the IPC call returns.

Actually looking more carefully: `rebuild_recent_submenu` takes the lock at line 1601, calls `ipc::call` while holding it. The `on_menu_event` closure at line 1399 does not call `rebuild_recent_submenu` — but future callers must be careful.

**Recommendation:** Drop the guard before calling `ipc::call` by cloning the `Arc<Submenu>` out first:
```rust
let submenu = { state.0.lock()?.as_ref().ok_or(...)?.clone() };
// guard dropped; now call IPC
let items = ipc::call(…)?;
// re-acquire only to mutate
```

---

### RS-3 `check_and_notify_new_capture` issues `get_config` on every 5 s tick to read one bool (LOW / S)

**File:** `src-tauri/src/lib.rs:1884–1888`

```rust
let notify_enabled = ipc::call("get_config", serde_json::json!({}))
    .ok()
    .and_then(|r| r.data)
    .and_then(|d| d["notify_on_copy"].as_bool())
    .unwrap_or(false);
```

`get_config` returns the full app configuration. This is called **every 5 seconds** just to read the `notify_on_copy` boolean. The full config payload is deserialized and immediately discarded except for this one field.

**Recommendation:** Cache `notify_on_copy` in an `AtomicBool` on the Tauri state, updated only when `set_config` is called. Eliminates 12 IPC calls/min just for this check.

---

### RS-4 `post_un_notification` spawns a new OS thread on every copy (LOW / S)

**File:** `src-tauri/src/lib.rs:918`

```rust
fn post_un_notification(title: String, body: String) {
    std::thread::spawn(move || { … UNUserNotificationCenter … });
}
```

A new thread is spawned for every notification. `UNUserNotificationCenter` calls are async (completion blocks) so this thread blocks only for the synchronous parts. For high-frequency copies (bulk copy, tray paste) this spawns many threads in quick succession.

**Recommendation:** Use `tauri::async_runtime::spawn` (Tokio task) instead of `std::thread::spawn`, or send the notification request to a dedicated notification thread via a channel. Avoids OS thread creation overhead.

---

## 6. Quantified Polling Summary

| Poller | Interval | IPC calls/min | Scope |
|---|---|---|---|
| HistoryView history_page(200) | 1 200 ms | 50 | While main window visible |
| Popup history_page(50) | 3 000 ms | 20 | While popup open |
| SyncStatusChip get_sync_status+list_peers | 10 000 ms | 12 | While main window visible |
| DevicesView peers | 10 000 ms | 6 | While Devices tab open |
| DevicesView discovered | 3 000 ms | 20 | While Devices tab open |
| DevicesView own_info | 10 000 ms | 6 | While Devices tab open |
| DevicesView SAS poll | 700 ms | ~86 | During pairing only |
| App.tsx accessibility check | 3 000 ms | 20 | Until permission granted |
| Tray resync thread | 5 000 ms | 36 | Always (3 IPC calls × 12/min) |
| Tray pairing poller | 1 000 ms | 60 | Always |
| **Total at rest (main window)** | — | **~98 IPC calls/min** | |
| **Peak (Devices tab + pairing)** | — | **~250 IPC calls/min** | |

Each IPC call costs: `connect + write + read + close` on a Unix socket + `spawn_blocking` task allocation.

---

## Top 10 Performance Wins (ranked by impact/effort ratio)

| Rank | Finding | Impact | Effort | Expected Gain |
|---|---|---|---|---|
| 1 | **IPC-1** Increase HistoryView poll from 1 200 → 3 000 ms | High | S | Halves dominant IPC load (~25 calls/min saved) with a one-liner change |
| 2 | **M-1** Cache full-res image in Details modal; eliminate repeat full-res fetches | High | S | Eliminates the 200–400 MB webview memory spike on re-open |
| 3 | **M-2** Fix `btoa(String.fromCharCode(...Array.from(bytes)))` stack overflow for large files | High | S | Fixes a crash bug and removes O(n) array allocation on file upload |
| 4 | **IPC-2** Persistent Unix socket connection pool in Tauri Rust layer | Med | L | Eliminates connect/close overhead for all 98–250 IPC calls/min |
| 5 | **R-1** Wrap `HistoryRow` in `React.memo` + stabilize per-row callbacks | High | M | Stops ~15 unnecessary re-renders every 1.2 s while window is visible |
| 6 | **RS-3** Cache `notify_on_copy` in `AtomicBool`; remove `get_config` from 5 s tray loop | Med | S | Removes 12 IPC calls/min (get_config) from always-on background thread |
| 7 | **IPC-3** Increase tray resync interval 5 s → 30 s + cache notify setting | Med | S | Reduces always-on tray thread from 36 → 6 IPC calls/min |
| 8 | **S-2** Code-split DevicesView/SettingsView/LogView behind `React.lazy` | Med | M | Reduces startup parse/compile time; popup bundle stays small |
| 9 | **R-2** Memoize `offsets` array in `VirtualList` | Med | S | Eliminates O(n) allocation on every render; critical at 1 000+ items |
| 10 | **IPC-5** Stop accessibility permission poll once granted or dismissed | Low | S | Eliminates 20 IPC calls/min (CoreGraphics API calls) after first launch |

---

*All line references are as of the branch state at time of audit. File path prefix `src/` = `crates/copypaste-ui/src/`; `src-tauri/src/` = `crates/copypaste-ui/src-tauri/src/`.*
