# macOS Desktop App — Code Review

**Branch:** `feat/android-parity-v0.5.3` (reviewed at HEAD)  
**Scope:** `crates/copypaste-ui/src/` (React/TS) + `crates/copypaste-ui/src-tauri/src/` (Rust/Tauri)  
**Date:** 2026-06-04  

---

## 1. Code Duplication

### D-1 — Duplicated "listen with cancelled guard" pattern (P2)

Identical `cancelled`/`unlisten` boilerplate appears in three separate `useEffect` blocks in `App.tsx` (lines 33–47, 61–81, 116–134). Each repeats the same async listen + cancellation ceremony.

```tsx
// App.tsx:33 — "open-settings"
let cancelled = false;
let unlisten: (() => void) | null = null;
void listen("open-settings", () => { if (!cancelled) setView("settings"); })
  .then((fn) => { if (cancelled) fn(); else unlisten = fn; })
  .catch(() => {});
return () => { cancelled = true; unlisten?.(); };
```

The same pattern repeats verbatim at lines 61 and 116. Recommendation: extract a `useTauriEvent<T>(event, handler)` hook that encapsulates this cancel-guard/unlisten logic.

### D-2 — Duplicated notification title/body logic in Rust AND TypeScript (P2)

`lib.rs::notification_title_body()` (Rust, lines 1686–1723) and `lib/ipc.ts::buildNotificationContent()` (TS, lines 602–619) implement the same content-type → title/body mapping with the same match arms ("Text Copied", "Image Copied", "File Copied"). The file-preview stripping regex in TypeScript (`/^\[file:\s*/` / `/\]$/`) differs from the Rust `strip_prefix("[file: ")` / `strip_suffix(']')`. Recommendation: keep the mapping in one place; the TypeScript path is only needed for UI-initiated copies, while the Rust path covers background captures — but the logic should at minimum be tested against the same expected strings.

### D-3 — Duplicated `truncatePreviewBody` / `build_text_preview_body` (P3)

`lib/ipc.ts::truncatePreviewBody()` (lines 625–633) and `lib.rs::build_text_preview_body()` (lines 1728–1748) both implement 160-char truncation at a word boundary. They are slightly different: the TS version uses `lastIndexOf(" " | "\n")`, the Rust version uses `rfind(char::is_whitespace)`. Minor semantic mismatch. They should at least reference a shared specification or be extracted into a shared utility.

### D-4 — Duplicated offline-state EmptyState JSX (P3)

The "Clipboard service offline" `EmptyState` block (with the lightning SVG, title, RestartDaemonButton) is copied verbatim in `HistoryView.tsx` (line 2113–2127) and in `DevicesView.tsx` (lines 1240–1254). The `DevicesView` additionally duplicates the warning-circle SVG in both the `degraded` state (line 1264) and the `error` state (line 1285) using identical SVG markup and class lists. Recommendation: extract into shared `DaemonOfflineState`, `DaemonDegradedState`, and `DaemonErrorState` components.

### D-5 — Duplicated "sound + notification on copy" gates (P2)

The `playSoundOnCopy` / `notifyOnCopy` check-and-fire block appears three times in `HistoryView.tsx`:

- `handleCopy` (lines 1573–1582)
- `handleBulkCopy` (lines 1857–1867)
- `handleKeyDown` → delegates to `handleCopy` (OK, not a dupe)

And again in `Popup.tsx::copyAndPaste` (lines 360–367). The notification bodies are assembled differently in each site. Recommendation: extract a `fireCopyFeedback(entry: HistoryEntry)` helper that gates both actions.

### D-6 — Duplicated global-message-timer pattern in `DevicesView` (P3)

`globalMsgTimer.current` is used identically in `handleRevokeAndRotate` (lines 1159–1164) and `handleRevokeAllConfirmed` (lines 1183–1190):

```tsx
if (globalMsgTimer.current !== null) clearTimeout(globalMsgTimer.current);
setGlobalMsg({ text: "...", isError: false });
globalMsgTimer.current = setTimeout(() => setGlobalMsg(null), N);
```

Recommendation: extract a `showGlobalMsg(text, isError, durationMs)` helper.

### D-7 — Duplicated `history_page` call pattern in Rust tray code (P3)

`setup_tray` and `rebuild_recent_submenu` in `lib.rs` contain identical item-extraction logic from a `history_page` reply (lines 1302–1320 and 1607–1629). The `items_opt: Option<Vec<(String, String)>>` extraction chain is copied exactly. Recommendation: extract a `fetch_recent_items(limit: usize) -> Option<Vec<(String, String)>>` helper.

---

## 2. Dead / Unused Code

### DC-1 — `api.generatePairingQr` is dead (P2)

`lib/ipc.ts` line 464 exports `api.generatePairingQr()` which calls `"pair_generate_qr"` over the daemon IPC socket. However, the codebase actually uses `pairingQrSvg()` (line 674, a Tauri-direct command that also renders the SVG) everywhere that QR generation is needed. `api.generatePairingQr` is never called in any view or test. It is a second path to the same daemon method that was superseded when the SVG rendering was moved into the Tauri backend.

### DC-2 — `api.getOwnFingerprint` is dead (P3)

`lib/ipc.ts` line 454 exports `api.getOwnFingerprint()`. Searches show it is never called by any view or component — `DevicesView` uses `api.getOwnDeviceInfo()` which returns the fingerprint as part of the richer `OwnDeviceInfo` response. Dead wrapper.

### DC-3 — `detectStaleDaemon()` async function is dead (P2)

`lib/ipc.ts` lines 818–827 exports `detectStaleDaemon()`, an async function that fetches both `appVersion` and `api.status()` itself. `App.tsx` and `SettingsView.tsx` instead call `detectStaleDaemonFromStatus(status, myVer)` with pre-fetched data. The standalone async variant is never called.

### DC-4 — `SyncStatus` deprecated fields confuse consumers (P2)

`lib/ipc.ts` lines 217–223 declares three fields on `SyncStatus` as `@deprecated` ("Never emitted by daemon; kept for SettingsView compat"):

```ts
/** @deprecated Never emitted by daemon; kept for SettingsView compat. */
keychain_locked?: boolean;
/** @deprecated Never emitted by daemon; kept for SettingsView compat. */
db_unavailable?: boolean;
/** @deprecated Never emitted by daemon; kept for SettingsView compat. */
degraded_reason?: string | null;
```

These are type-level dead code; no consumer reads them, but they inflate the type and create confusion. They should be removed.

### DC-5 — `ipc.ts::formatWallTime` is unused (P3)

`lib/ipc.ts` line 531 exports `formatWallTime(ms)`. HistoryView uses `formatRelativeTime` from `lib/time.ts`. `DevicesView` uses `formatWallTime` (line 8 import, used for `revokedAt` in line 196). However, `formatWallTime` is also imported but not used in any recent code path in some views — confirmed: it IS used in DevicesView for `revokedAt` format. Keep as-is. Mark for audit.

### DC-6 — `renderAdvanced()` is a placeholder (P3)

`SettingsView.tsx` lines 1851–1858: `renderAdvanced()` returns only a static "Advanced daemon and storage limits will appear here in a future release" div. The tab itself is always shown to users. Either implement the tab or remove it from the tab list to avoid user confusion.

### DC-7 — `PrivacyPatch` local type in `SettingsView` is undocumented workaround (P2)

Lines 808–813 define a local `PrivacyPatch` type for fields that exist in the daemon's `set_config` but not in the `AppSettings` interface in `lib/ipc.ts`. This is explicitly named as a workaround: "Privacy & capture fields that are not (yet) in the AppSettings interface". The type is used as `& PrivacyPatch` in `buildConfigPatch` and then cast twice with `as unknown as Parameters<typeof api.setConfig>[0]`. This is a type-safety hole — it bypasses the `AppSettings` contract. Recommendation: add these fields to the `AppSettings` interface in `ipc.ts`.

---

## 3. Competing / Duplicate State

### CS-1 — `incomingPairing` prop vs `responderPairing` state in `DevicesView` (P1) — KNOWN PATTERN, NOT FIXED

`DevicesView` receives `incomingPairing: PairSasStatus | null` as a prop from `App.tsx` and also maintains a local `responderPairing` state. The two are synced by an effect:

```tsx
// DevicesView.tsx:757–761
useEffect(() => {
  if (incomingPairing != null) {
    setResponderPairing(incomingPairing);
  }
}, [incomingPairing]);
```

But `responderPairing` is initialized from the prop:

```tsx
const [responderPairing, setResponderPairing] = useState<PairSasStatus | null>(
  incomingPairing ?? null
);
```

There are now TWO sources of truth for the same concept. If `App.tsx` sets `incomingPairing` to a new value while the modal is already open (because a SECOND pairing request arrives), the effect fires and `setResponderPairing` overwrites the in-progress modal state silently. There is no guard for the "modal already open" case in the effect. Additionally, `handleClosePairing` clears `responderPairing` (line 1066) but NOT `incomingPairing` in App — so a re-mount of DevicesView would re-open the modal from the stale prop. Recommendation: lift the responder pairing modal state entirely to `App.tsx` and pass open/closed/payload as a single discriminated prop, or accept the limitation and add a guard in the effect.

### CS-2 — `config.p2p_enabled` vs `SettingsView`'s local form inputs (P2)

`SettingsView` maintains both a `config: AppSettings` state (line 506, sync'd from daemon on load) AND separate `supabaseUrl`, `supabaseKey`, `relayUrl` string states (lines 511–513). `buildConfigPatch` then re-assembles everything from these individual states. This means:

- `config.supabase_url` and `supabaseUrl` are two state variables for the same datum.
- `config.supabase_anon_key` and `supabaseKey` likewise.
- `config.relay_url` and `relayUrl` likewise.

They can desync: if `handleSaveConfig` updates `config` (line 953) but a subsequent load failure updates `config` from daemon but not the text inputs, the form shows stale strings. Recommendation: use a single form-state object or control each field from `config` directly without the parallel strings.

### CS-3 — `staleDaemon` state in both `App.tsx` and `SettingsView.tsx` (P2)

Both components independently fetch `detectStaleDaemonFromStatus` from `api.status()` on mount:

- `App.tsx` lines 147–166: fetches `appVersion` + `api.status()` and calls `detectStaleDaemonFromStatus`, storing result in `staleDaemon` state.
- `SettingsView.tsx` lines 632–647: also fetches `api.status()` independently (in the same `Promise.all`) and calls `detectStaleDaemonFromStatus`, storing result in its own `staleDaemon` state.

This means two independent concurrent IPC calls for the same data, and two independent version-comparison calculations that can produce different results (if a status changes between the two calls). Recommendation: store daemon status in the Zustand store, or at least share it via a context so it is fetched once.

### CS-4 — `privateMode` in Settings vs tray vs daemon (P2)

`SettingsView` has `privateMode` local state (line 503). There are three ways it gets set: (a) the load effect (line 656), (b) the window focus/visibility resync effect (lines 747–783), and (c) the Tauri event listener for `"private-mode-changed"` (lines 771–776). The tray has its own CheckMenuItem. The two-step confirm in `spawn_tray_private_mode_resync` is a workaround for the race where these three mechanisms deliver different values. This is a three-source-of-truth problem. Recommendation: treat `private_mode` as a daemon-owned fact and store it in Zustand with a single IPC fetch path.

---

## 4. Weird / Buggy Behavior

### B-1 — Race condition: `SasPairingModal` `onPaired` callback is stale-closure risk (P1)

`SasPairingModal` takes `onPaired: () => void` as a prop (line 261). The polling effect closes over `onPaired` via the dep array:

```tsx
// DevicesView.tsx:394
}, [onPaired, initialStatus]);
```

In `DevicesView`, `onPaired={loadPeers}` where `loadPeers` is a `useCallback` with no deps (line 914). This is stable, so no bug here currently. However, the effect also closes over `confirmedRef` and `localAcceptedRef` (both refs, safe). The actual race: if `onPaired` were ever recreated (e.g. if `loadPeers` gained a dep), the polling effect would restart, resetting `sawActive` to `false`, even mid-handshake. The `sawActive` local variable being inside the effect closure means a restart re-initializes it from `initialStatus.state` only — ignoring any active progress already observed. This is a latent bug if `loadPeers` deps ever change.

### B-2 — `handlePairDiscovered` stale guard: `pairStarting || pairingDevice !== null` (P1)

`handlePairDiscovered` (DevicesView.tsx line 1041) guards against a concurrent start with:

```tsx
if (pairStarting || pairingDevice !== null) return;
```

But `pairStarting` and `pairingDevice` are read from the closure at the time `useCallback` was memoized:

```tsx
}, [pairStarting, pairingDevice]);
```

Since the callback IS in the dep array, it is recreated when either changes. However there is a subtle race: if the user clicks "Pair" twice very rapidly before the first `setPairStarting(true)` commit causes a re-render, the second click sees `pairStarting === false` (the old closure value) and sends a second `pairWithDiscovered` IPC call. The guard is correct for normal use but not for React batching edge cases. Recommendation: use a `useRef` guard that is set synchronously before the async call.

### B-3 — Polling effect restart on `loadPeers` identity change wipes loading state (P2)

`DevicesView` calls `setLoadState("loading")` at the top of `loadPeers` (line 915). The 10s interval effect and the initial load effect both call `void loadPeers()`. Because `loadPeers` is a `useCallback` with empty deps, it is stable. But if it were ever given deps, the interval effect (which depends on `loadPeers`) would be cleared and restarted, causing an instant `setLoadState("loading")` which would blank the peer list during the normal 10s poll cycle — a flash that would be visible to users.

### B-4 — `handleTestConnection` calls `handleSaveConfig` as a side effect (P2)

`SettingsView.tsx` line 978:

```tsx
const handleTestConnection = useCallback(async () => {
  setTesting(true);
  setTestMsg(null);
  try {
    await handleSaveConfig();  // SAVES config as a side-effect of testing
    const result = await api.testCloudConnection();
    ...
```

Clicking "Test connection" silently saves the current config (including potentially unintended partial edits) to the daemon and restarts the daemon. This is a side-effect the user did not request. The "Test" button should test the current input values without saving.

### B-5 — `load` in `HistoryView` is called inside the interval without checking if already loading (P2)

`HistoryView`'s visibility effect (line 1402–1438) calls `void load(true)` immediately on becoming visible AND then starts an interval that calls `void load(true)` every `ACTIVE_MS` (1200ms). If `load(true)` takes longer than 1200ms (slow daemon, large history), multiple concurrent loads can be in-flight. There is no in-flight guard in `load` for the silent path (`load(true)` where `silent=true` skips `setLoadState("loading")`). Multiple concurrent mutations to `items`, `totalCount`, etc. will be committed in an undefined order (whichever IPC call resolves last wins). Recommendation: add a `loadingRef` in-flight guard or use `useReducer` to serialize updates.

### B-6 — `check_and_notify_new_capture` makes two IPC calls per 5s tick (P3)

`spawn_tray_recent_resync` (lib.rs line 1840) calls `rebuild_recent_submenu` then `check_and_notify_new_capture` on every 5s tick. `rebuild_recent_submenu` already calls `ipc::call("history_page", { limit: 10, ... })`. Then `check_and_notify_new_capture` calls `ipc::call("history_page", { limit: 1, ... })` PLUS `ipc::call("get_config", ...)`. This is 3 blocking IPC calls on a background thread every 5 seconds. They should be coalesced into a single `history_page(limit: 10)` call whose first item is used for the notification check.

### B-7 — `DevicesView` SAS modal opened with wrong `initialStatus` prop (P1 bug)

At line 1492–1498 in `DevicesView.tsx`:

```tsx
{pairingDevice !== null && (
  <SasPairingModal
    device={pairingDevice ?? undefined}
    initialStatus={incomingPairing ?? undefined}  // BUG: should be undefined here
    onClose={handleClosePairing}
    onPaired={loadPeers}
  />
)}
```

The **initiator** modal (user clicked "Pair") is passed `initialStatus={incomingPairing ?? undefined}`. `initialStatus` seeds `sawActive` in the poll effect:

```tsx
let sawActive =
  initialStatus?.state === "initiating" ||
  initialStatus?.state === "awaiting_sas";
```

If an `incomingPairing` is in flight at the time the user initiates a pairing, the initiator modal starts with `sawActive=true` (from the responder payload). This causes a trailing `idle` after the initiator's `pairWithDiscovered` to be misinterpreted as a completed handshake, calling `onPaired()` spuriously. The initiator modal should never receive `incomingPairing` as `initialStatus`; it should always start with `initialStatus={{ state: "initiating" }}` or `undefined`.

### B-8 — `FullResImage` `mountedRef` is not reset on `id` change (P2)

`HistoryView.tsx` lines 854–870:

```tsx
useEffect(() => {
  mountedRef.current = true;  // reset to true on each id change
  setSrc(null);
  setFailed(false);
  api.getItemImage(id).then(...)
  return () => { mountedRef.current = false; };
}, [id]);
```

Actually this IS correctly reset (line 858). On closer inspection the `mountedRef` is defined outside the effect (`const mountedRef = useRef(true)`) and the cleanup sets it to `false`. The problem: if two rapid `id` changes fire before the first fetch resolves, the cleanup for the first effect sets `mountedRef.current = false`, then the second effect sets it back to `true`. The first fetch then resolves after the cleanup and checks `if (!mountedRef.current) return` — but `mountedRef.current` is `true` (the second effect reset it). The first fetch's result would then overwrite `src` even though the component is on a different `id`. A ref alone is not sufficient to guard concurrent fetches; an AbortController or effect-local cancelled flag is needed.

---

## 5. Type / Contract Smells

### T-1 — `as unknown as Record<string, unknown>` cast on every `api.setConfig` call (P1)

`api.setConfig` is defined as:
```ts
setConfig: (settings: AppSettings) =>
  ipcCall("set_config", settings as unknown as Record<string, unknown>),
```

The internal cast is unavoidable since `ipcCall` takes `Record<string, unknown>`. However, every call site in `SettingsView` then also casts:

```ts
await api.setConfig(
  buildConfigPatch({ p2p_enabled: val }) as unknown as Parameters<typeof api.setConfig>[0],
)
```

`buildConfigPatch` returns `AppSettings & PrivacyPatch` which is not assignable to `AppSettings` because of the extra `PrivacyPatch` fields. The correct fix is to widen `AppSettings` to include those fields (see DC-7 above) so the cast disappears.

### T-2 — `Partial<DaemonStatus>` cast in `probeStatus` (P2)

`lib/ipc.ts` line 192:
```ts
const s = (await api.status()) as Partial<DaemonStatus>;
```

`api.status()` returns `Promise<DaemonStatus>`, so the cast to `Partial<DaemonStatus>` is a downcast that hides the fact that fields like `degraded` and `ready` are always present in the return type. This is a type lie — it forces defensive `?.` checks that should not be needed. The same pattern appears in `SettingsView.tsx` line 633:
```ts
api.status().catch(() => null) as Promise<DaemonStatus | null>,
```
which is fine, but then line 639 accesses `daemonSt.degraded === true` without the cast — the cast is applied at the wrong layer.

### T-3 — `copyAndPaste` in `Popup.tsx` casts `copied` to a custom object shape (P2)

`Popup.tsx` lines 349–355:
```ts
const copied = await api.copyItem(id);
const preview =
  typeof copied === "object" && copied !== null && "preview" in copied
    ? String((copied as { preview: string }).preview)
    : "";
const contentType =
  typeof copied === "object" && copied !== null && "content_type" in copied
    ? String((copied as { content_type: string }).content_type)
    : "";
```

`api.copyItem` returns `Promise<unknown>` (the generic `ipcCall<unknown>` default). The runtime shape check is correct but the type system provides zero safety. Recommendation: type the `copyItem` reply as `ipcCall<{ preview: string; content_type: string } | null>`.

### T-4 — `any`-equivalent cast `as unknown as readonly number[]` repeated 5 times (P3)

`SettingsView.tsx` lines 43–60 define step arrays cast as:
```ts
const TEXT_SIZE_STEPS_BYTES = [...].map((n) => n * 1024 * 1024) as unknown as readonly number[];
```

This cast is repeated for every step array (TEXT, IMAGE, FILE, QUOTA, SENSITIVE_TTL). The underlying issue is that `.map()` produces `number[]` not the literal-tuple type the `snapToNearest<T extends number>` generic expects. Using `as const` on the pre-multiplied tuple or accepting `readonly number[]` in `snapToNearest` would eliminate these casts.

### T-5 — `privacyCfg` typed inline in SettingsView load (P2)

`SettingsView.tsx` lines 703–710:
```ts
const privacyCfg = rawCfg as {
  collect_public_ip?: boolean | null;
  paste_as_plain_text?: boolean | null;
  excluded_app_bundle_ids?: string[] | null;
};
```

This is the same workaround as DC-7/T-1 — these fields exist in the daemon's `AppConfig` but are not in the TS `AppSettings` interface. The cast silently succeeds; if the daemon renames a field, no compiler error fires. Fix: add these fields to `AppSettings`.

---

## 6. Architecture Smells

### A-1 — `HistoryView.tsx` is 2348 lines — severely oversized (P1)

This single file contains: `Toast`, `ContentIcon`, `KindChip`, `PinIndicator`, `SyncBlockedIndicator`, `DeviceBadge`, `HistoryRow`, `IconActionBtn`, `BulkActionBar`, `FullResImage`, `DetailsModal`, `VirtualList`, the main `HistoryView`, and all drag-reorder logic. The file is a monolith. It is difficult to test, review, or modify in isolation. Recommendation: extract at minimum `DetailsModal`, `BulkActionBar`, `VirtualList`, and `HistoryRow` into their own files under `src/components/history/`.

### A-2 — `DevicesView.tsx` is 1593 lines — severely oversized (P1)

Contains `StatusDot`, `MetaRow`, `DeviceMetaGrid`, `ThisDeviceCard`, `PeerRow`, `SasPairingModal`, `DiscoveredRow`, and the main `DevicesView` with all pairing state machines and IPC handlers. The `SasPairingModal` alone is ~370 lines. Recommendation: extract `SasPairingModal`, `PeerRow`, and discovery-related components into separate files.

### A-3 — `SettingsView.tsx` is 1944 lines with 6 tab renderers as nested functions (P2)

`renderGeneral`, `renderDisplay`, `renderSync`, `renderShortcuts`, `renderStorage`, `renderAdvanced` are functions defined inside the component body (e.g. `renderStorage` contains a further nested `LimitSliderRow` component at line 1666). Nested function components are re-created every render, which means `LimitSliderRow` has no stable identity, preventing React from reconciling/diffing it — it remounts on every parent render. Additionally, all 6 tab contents share the same 30+ state variables in one scope. Recommendation: split each tab into its own file-level component, sharing state via props/context.

### A-4 — Polling everywhere instead of event-driven updates (P2)

The app uses four independent polling loops for data that could be pushed:

1. `HistoryView`: 1200ms interval for history updates (line 1402).
2. `DevicesView`: 10s interval for peer list (line 978), 3s interval for discovered devices (line 1006), 1s clock tick for "last seen" labels (lines 710–715).
3. `App.tsx`: 3s interval for accessibility permission status (lines 195–200).
4. Rust `spawn_tray_recent_resync`: 5s interval for tray Recent submenu (line 1781).
5. Rust `spawn_incoming_pairing_poller`: 1s poll for pairing state (line 1946).

The daemon already has an event system (Tauri `emit`). The history view's 1200ms poll is the most impactful: it runs 50 IPC calls/minute continuously while the window is visible. A daemon-side push event on clipboard change would allow the UI to wake on demand rather than polling.

### A-5 — `addFileItem` in `lib/ipc.ts` uses O(n) `btoa(String.fromCharCode(...Array.from(bytes)))` (P2)

`lib/ipc.ts` lines 523–526:
```ts
const data_b64 = btoa(String.fromCharCode(...Array.from(bytes)));
```

This spreads all bytes into a function argument, which hits the call-stack argument limit for files > ~65KB and is O(n) memory even for smaller files. For a 100MB file (the configured cap) this will throw `RangeError: Maximum call stack size exceeded`. Recommendation: use a loop-based chunked approach or `TextDecoder`/`Uint8Array` base64 encoding.

### A-6 — Two separate popup-hide paths can still cause double-hide (P2)

The popup is hidden via:
1. `invoke("hide_popup")` from JS (Popup.tsx line 289)
2. `WindowEvent::Focused(false)` in `wire_popup` (lib.rs line 1230)
3. `toggle_popup` close branch calling `hide_popup_internal` (lib.rs line 1041)

The `isHidingRef` in the JS layer (Popup.tsx line 282) is a client-side guard. The Rust `is_visible()` check in `wire_popup` (line 1233) is a server-side guard. But between the JS `invoke("hide_popup")` return and the OS-level window hide completing, a `Focused(false)` event can fire. The `is_visible()` call on line 1233 checks the window's state at the time the event handler fires — but on macOS, `is_visible()` may still return `true` during the brief window between the JS call and the actual hide. The guard is best-effort and the comment acknowledges it: "Skip if already hidden — avoids double hide_popup_internal call". This is correct as documented but worth noting as a timing-dependent guard.

---

## Top 10 Issues to Fix First

| Rank | ID | Severity | Summary |
|------|----|----------|---------|
| 1 | B-7 | P1 bug | Initiator SAS modal receives `incomingPairing` as `initialStatus` — can spuriously call `onPaired()` if a responder request is in flight simultaneously |
| 2 | CS-1 | P1 | `incomingPairing` prop vs `responderPairing` state: two sources of truth, no guard for concurrent second pairing request overwriting an in-progress modal |
| 3 | T-1 | P1 | `as unknown as Parameters<typeof api.setConfig>[0]` cast at every call site — fix by adding `collect_public_ip`, `paste_as_plain_text`, `excluded_app_bundle_ids` to `AppSettings` in `ipc.ts` |
| 4 | A-5 | P2 | `addFileItem` base64 encoding via spread (`...Array.from(bytes)`) will throw `RangeError` for files > ~65KB; use chunked encoding |
| 5 | B-4 | P2 | "Test connection" button silently saves config and restarts daemon as a side effect; these should be independent actions |
| 6 | B-8 | P2 | `FullResImage` uses a shared `mountedRef` that gets reset by concurrent `id` changes — first fetch's result can overwrite the second's; use an effect-local cancelled flag |
| 7 | DC-1 | P2 | `api.generatePairingQr` is dead code — superseded by `pairingQrSvg()` Tauri command; remove to avoid confusion about which QR path to use |
| 8 | CS-3 | P2 | Both `App.tsx` and `SettingsView.tsx` independently fetch daemon status + stale-daemon check on mount — two concurrent IPC calls for identical data; store in Zustand |
| 9 | A-1/A-2 | P1 | `HistoryView.tsx` (2348 lines) and `DevicesView.tsx` (1593 lines) are unmaintainable monoliths; extract `SasPairingModal`, `VirtualList`, `DetailsModal`, and `BulkActionBar` as a minimum |
| 10 | D-2 | P2 | Notification title/body logic duplicated in Rust (`lib.rs::notification_title_body`) and TypeScript (`ipc.ts::buildNotificationContent`) with minor behavioral differences in file-preview stripping |

---

## Summary Notes

- **File reviewed:** `src/`, `src-tauri/src/` of `crates/copypaste-ui`
- **No code was modified** — read-only review.
- The codebase is well-commented and the IPC contract is clearly typed; the main debt is component size and split state management.
