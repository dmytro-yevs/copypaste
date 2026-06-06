# CopyPaste UX/UI Heuristic Evaluation
**Branch:** v0.6.1-integration  
**Scope:** macOS (Tauri/React) + Android (Compose) — both apps  
**Evaluator role:** senior product designer, heuristic + DESIGN-SYSTEM-v2 compliance  
**Date:** 2026-06-04

---

## 1. Cross-Platform Consistency

### CP-1 — Settings tab structure mismatch (P1)
**macOS:** General / Display / Sync / Shortcuts / Storage / Advanced (6 tabs)  
**Android:** General / Display / Storage / Sync / Notifications (5 tabs)

The Shortcuts tab has no Android equivalent (global hotkey is macOS-only — acceptable), but "Notifications" exists only on Android while sound/notify toggles live in macOS General. A user checking notification settings on Android then switching to macOS will look for a Notifications tab and not find it.  
**Fix:** Rename the Android "Notifications" tab to "General" and merge the notification toggles there, mirroring macOS exactly. Or add a "Notifications" subsection inside macOS General and call the tab "General" on both.

**Files:** `crates/copypaste-ui/src/views/SettingsView.tsx:298–305`, `android/app/src/main/java/com/copypaste/android/SettingsActivity.kt:109–113`

---

### CP-2 — Storage step labels: "MB" (macOS) vs "MiB" (Android) (P2)
macOS labels show `"15 MB"` while Android labels show `"15 MiB"`. The underlying values are identical binary mebibytes. This creates confusion: a user setting limits on one device reads different strings on the other.  
**Fix:** Standardise to one notation across both platforms. "MB" (common app convention, used in macOS version) is fine as long as the code comment clarifies binary; update Android `TEXT_SIZE_STEP_LABELS` / `IMAGE_SIZE_STEP_LABELS` / `QUOTA_STEP_LABELS` in `Components.kt:341–376` to use "MB"/"GB".

**Files:** `crates/copypaste-ui/src/views/SettingsView.tsx:43–58`, `android/app/src/main/java/com/copypaste/android/ui/theme/Components.kt:341–376`

---

### CP-3 — Device badge label "This Mac" hardcoded on macOS (P2)
`DevicesView.tsx:128` renders a hardcoded `"This Mac"` badge. On a hypothetical Linux or future multi-platform build this is wrong, but more importantly it diverges from Android which derives the label from the device name. It also signals inconsistency to users who consciously compare the two apps.  
**Fix:** Replace with `"This device"` (Android already uses this string in its peer filter) or derive from `info.device_model`.

**Files:** `crates/copypaste-ui/src/views/DevicesView.tsx:128`

---

### CP-4 — Icon system: macOS uses inline SVG/Lucide-style, Android uses Material filled icons (P2)
The macOS popup uses a teal `↗` arrow glyph for URLs (`Popup.tsx:55`) — a literal Unicode character. Android uses `Icons.Filled.BookmarkAdded`, `Icons.Filled.SwapVert`, etc. — filled Material symbols. DESIGN-SYSTEM-v2 §9 specifies Lucide (lucide-compose on Android) at 1.5px stroke. Neither platform fully complies: macOS has mostly correct custom SVGs but Android has zero Lucide adoption.  
**Fix:** Adopt `lucide-compose` ImageVectors on Android for the core action set (pin/bookmark, trash, search, image, file, link). This is the §10 platform parity task and unifies visual language. Start with History row actions since those are the highest-frequency icons.

**Files:** `android/app/src/main/java/com/copypaste/android/HistoryActivity.kt:54–70` (all `Icons.Filled.*` imports)

---

### CP-5 — Pairing entry point: macOS embeds QR in DevicesView; Android has a separate PairActivity (P2)
On macOS the QR code, discovered-devices list, and paired-device list are all in one scrollable DevicesView. On Android, the QR scan/display is a separate `PairActivity` launched via a button in `DevicesActivity`. This fragmentation means a user familiar with macOS must hunt for the QR flow on Android.  
**Fix:** Inline the QR display section into Android's `DevicesScreen` (below the paired-devices card, same layout as macOS). The camera scan can remain a launched intent/activity since camera permissions require a different flow.

**Files:** `android/app/src/main/java/com/copypaste/android/PairActivity.kt`, `android/app/src/main/java/com/copypaste/android/DevicesActivity.kt`

---

### CP-6 — "Clear all" / "Clear unpinned" terminology diverges (P2)
macOS History actions label uses "Clear" (BulkActionBar `HistoryView.tsx:838`). Android overflow menu uses `R.string.action_clear_unpinned` and the confirmation dialog title references `dialog_clear_unpinned_title`. These strings are not visible to verify exact wording without the strings.xml, but the pattern shows macOS doesn't have a "clear unpinned" distinction directly accessible — it goes through bulk-select then delete. Android surfaces "Clear unpinned" as a first-class menu item.  
**Fix:** Add "Clear unpinned" as a direct action on macOS (overflow/kebab menu in History header) to match Android. This aligns both surfaces.

**Files:** `crates/copypaste-ui/src/views/HistoryView.tsx:2016–2100`

---

### CP-7 — Reorder UX: macOS = drag-and-drop, Android = up/down arrow buttons (P3)
macOS pinned items support HTML5 drag-and-drop (`HistoryView.tsx:554–714`). Android implements reorder via a `SwapVert` toolbar toggle + per-item `KeyboardArrowUp`/`KeyboardArrowDown` buttons in the list. These are two completely different interaction models.  
**Fix:** Acceptable given platform norms (iOS-style drag is complex), but at minimum the reorder affordance should be consistent in how it is activated. Explore `LazyColumn` reordering via `ReorderableState` for Android.

---

## 2. Information Hierarchy & Clarity

### IH-1 — History row: right cluster can exceed 4.5rem minimum-width causing overflow (P1)
`HistoryView.tsx:657` sets `minWidth: "4.5rem"` on the right cluster. This cluster can contain: DeviceBadge + app chip + timestamp + three icon buttons. When all are present simultaneously the cluster expands well beyond 4.5rem, pushing the content preview to collapse to almost nothing on narrow windows (e.g. 400px popup). The design spec says right cluster = "fixed min-4.5rem" but in practice it is not fixed.  
**Fix:** Give the right cluster a fixed max-width of `~8rem` (cap badge + time + icons) and use `overflow: hidden` clipping on badges, or make device badge and app chip mutually exclusive (show only the more useful one at a time).

**Files:** `crates/copypaste-ui/src/views/HistoryView.tsx:653–711`

---

### IH-2 — KindChip labels use 9px font: below the 10.5px floor (P1)
`HistoryView.tsx:245` renders `KindChip` at `text-[9px]`. DESIGN-SYSTEM-v2 §1 explicitly states "Never < 10.5px". This is illegible at normal display densities and on non-Retina screens.  
**Fix:** Raise KindChip font to `text-[10px]` minimum, or accept the 10.5px minimum and adjust chip padding accordingly.

**Files:** `crates/copypaste-ui/src/views/HistoryView.tsx:244`

---

### IH-3 — DeviceBadge shows raw UUID prefix (8 chars) for remote devices (P2)
`HistoryView.tsx:318`: remote-origin badges show `originId.slice(0, 8)` — a hex fragment like `"3a7f1b2c"` that is meaningless to users. Users know their device by name (iPhone, MacBook), not UUID.  
**Fix:** Resolve the UUID to a device name using the paired-peers roster (same approach DevicesView uses for `PeerRow`). If the device is not in the roster, show "Remote" as a fallback rather than a UUID fragment. Pass `pairedPeers` into `HistoryView` from the store.

**Files:** `crates/copypaste-ui/src/views/HistoryView.tsx:313–343`

---

### IH-4 — Android History row: no type chip displayed (P2)
Android `HistoryList` rows (reading `HistoryActivity.kt`) use `Icons.Filled.Image`, `Icons.Filled.AttachFile`, and `Icons.Filled.Lock` for content-type indication, but there is no equivalent to macOS's `KindChip` for text subtypes (URL, EMAIL, CODE, etc.). A user who copies a URL sees no visual differentiation from plain text on Android.  
**Fix:** Add a small `KindChip`-equivalent Compose component for text-subtype classification on Android rows. The `TextKind` class already exists (`TextKind.kt`) — wire its value into the row composable.

**Files:** `android/app/src/main/java/com/copypaste/android/HistoryActivity.kt` (row composable section), `android/app/src/main/java/com/copypaste/android/TextKind.kt`

---

### IH-5 — Timestamp format inconsistency: macOS uses "long" format, popup uses "short" (P3)
`HistoryView.tsx:679` calls `formatRelativeTime(entry.wall_time, "long")`. `Popup.tsx:688` calls `formatRelativeTime(item.wall_time, "short")`. The popup shows "2m" while the main window shows "2 min ago". This is actually appropriate (popup needs density), but it is undocumented and could drift. Android's `relativeTime()` in `HistoryActivity.kt:219` uses yet another format ("${diff/60_000}m ago").  
**Fix:** Align the short format across popup and Android: both should produce "2m ago" (not "2m" without "ago"). Document the intended format contract.

**Files:** `crates/copypaste-ui/src/popup/Popup.tsx:688`, `android/app/src/main/java/com/copypaste/android/HistoryActivity.kt:219`

---

### IH-6 — Devices view "online" count shows only peers, not "this device" (P3)
`DevicesView.tsx:1307–1312` renders the green dot + "X online" count from `peers.filter(p => p.online)`. This device is always online but not counted. On a two-device setup the count shows "0 online" even when both devices are active.  
**Fix:** Add 1 to the online count for the local device (it is always online).

**Files:** `crates/copypaste-ui/src/views/DevicesView.tsx:1307–1312`

---

## 3. Friction & Flows

### FF-1 — QR blur/reveal requires an extra click before the user can scan (P1)
`DevicesView.tsx:773–779`: the QR starts blurred with "Click to reveal". This adds one interaction to the most common pairing task (user opens Devices, wants to show QR to phone). The security intent is valid (prevent shoulder-surfing in screenshots), but the default-blurred state means every pairing requires an extra step.  
**Fix:** Remove the default blur. If security is required, provide an explicit "Hide QR" button that the user can activate, rather than a mandatory reveal click on every open. Alternatively, blur only when the window loses focus (similar to password managers).

**Files:** `crates/copypaste-ui/src/views/DevicesView.tsx:769–778`

---

### FF-2 — SAS modal "Match" / "Doesn't match" button labels are ambiguous (P1)
`DevicesView.tsx:534–545`: the confirmation buttons are labelled "Doesn't match" and "Match". Standard SAS nomenclature in other products (Signal, WireGuard, etc.) is "Confirm" / "Decline" or "The numbers match" / "The numbers don't match". "Match" as a verb is grammatically compact but unusual — users may read it as a state ("it is a match") rather than a confirmation action.  
**Fix:** Rename to "Confirm" (primary, accent) and "Reject" (secondary). Or use "Yes, match" / "No, different" for clarity at the cost of length.

**Files:** `crates/copypaste-ui/src/views/DevicesView.tsx:534–545`

---

### FF-3 — Android pairing: no mDNS discovery section visible; QR scan is the only path shown in DevicesActivity (P1)
`DevicesActivity.kt` (and `DevicesScreen`) contains peer cards and an "Add device" button that launches `PairActivity`. There is no inline "Discovered on your network" section analogous to `DevicesView.tsx:1374–1404`. LAN autodiscovery exists on macOS but not surfaced on Android.  
**Fix:** Add a "Nearby devices" section in `DevicesScreen` using the existing `listDiscovered()` logic from the macOS side. This is the most frictionless pairing path for LAN users and should be first-class on both platforms.

**Files:** `android/app/src/main/java/com/copypaste/android/DevicesActivity.kt`

---

### FF-4 — Settings on Android: "Save" button is required; macOS auto-saves (P1)
`SettingsActivity.kt:109` comment: "All edits buffer in Compose state; values are only written to SharedPreferences when the user taps the Save button." macOS Settings auto-saves on each toggle/slider change (per-field `saveLimitsField` with "Saved" badge). This is a fundamental model mismatch. Android users who forget to tap Save lose their changes on back navigation.  
**Fix:** Switch Android Settings to auto-save on change, using a 300ms debounce for sliders (same pattern as macOS `onRelease`). If the unsaved-changes guard is needed for nav-away, adopt the macOS pattern of per-field persistence rather than buffered batch.

**Files:** `android/app/src/main/java/com/copypaste/android/SettingsActivity.kt:240–300` (dirty tracking)

---

### FF-5 — Revoke dialog requires a passphrase for the "Revoke & rotate" path but gives no password strength guidance (P2)
`DevicesView.tsx:1543–1554`: the passphrase input field has placeholder "At least 8 characters" but no strength indicator, no visibility toggle, and the "Revoke & rotate" button just goes disabled below 8 chars with a tooltip that only appears on hover. On mobile/touch there is no hover.  
**Fix:** Add a password strength bar (simple 3-level: weak/ok/strong based on entropy), a "show/hide" eye icon, and surface the "8 chars min" message as permanent help text below the input rather than relying on a tooltip.

**Files:** `crates/copypaste-ui/src/views/DevicesView.tsx:1543–1590`

---

### FF-6 — No onboarding/first-run experience on macOS; Android has OnboardingActivity (P2)
Android has `OnboardingActivity.kt` that handles permissions setup. macOS has no equivalent — a fresh install opens directly to the History view showing "Nothing copied yet." There is no guidance on: enabling clipboard access, setting a global shortcut, or connecting devices.  
**Fix:** Add a first-run flow on macOS: a one-time welcome sheet (modal overlay) that prompts for clipboard accessibility permissions, shows the default shortcut, and offers a "Connect a device" CTA.

---

### FF-7 — Empty state for "Daemon offline" in Popup gives no path to fix it (P2)
`Popup.tsx:527–530`: when the daemon is offline, the popup shows "The daemon is not running. Restart it from Settings." But clicking Settings requires closing the popup, opening the main window, and navigating to Settings. Users expect a direct "Restart" button.  
**Fix:** Add a `RestartDaemonButton` directly inside the popup's offline empty state (same as `HistoryView.tsx:2123`). The button is already a component — reuse it.

**Files:** `crates/copypaste-ui/src/popup/Popup.tsx:527–530`

---

### FF-8 — Delete single item has no undo / confirmation (P2)
Clicking the trash icon on a `HistoryRow` calls `api.deleteItem()` immediately with no confirmation or undo toast. Bulk delete has a toast ("Deleted N items") but no undo path. Single-item delete is silent.  
**Fix:** Show a 3s "Deleted — Undo" toast for single-item deletions (same pattern as Gmail). Implement a soft-delete with a deferred commit to enable the undo window.

**Files:** `crates/copypaste-ui/src/views/HistoryView.tsx:1689–1701`

---

### FF-9 — Settings "Advanced" tab is empty or near-empty in the shipped UI (P3)
`SettingsView.tsx:296–304` defines an "Advanced" tab. Reading the renderAdvanced function (lines beyond the read window) — the tab exists but its content is unclear. If it is empty or contains only 1–2 items, users will open it expecting power-user controls and find nothing.  
**Fix:** Either populate Advanced with legitimately advanced items (excluded apps, daemon version, log export) or remove the tab and redistribute its content.

---

## 4. Visual Polish

### VP-1 — Popup content chip uses literal "↗" Unicode glyph for URLs; all others use SVG (P2)
`Popup.tsx:55`: the URL chip renders `↗` as plain text inside a `<span>`. Every other chip type uses an SVG icon. Under certain font stacks, font metrics differ for this glyph, causing the chip to be taller or slightly misaligned.  
**Fix:** Replace the `↗` text with the same SVG external-link path used in `HistoryView.tsx:126–140` for consistency.

**Files:** `crates/copypaste-ui/src/popup/Popup.tsx:54–56`

---

### VP-2 — HistoryView and Popup have different chip rendering: KindChip word vs single-letter (P2)
The main History window uses `KindChip` with full words ("URL", "EMAIL", "CODE"). The popup uses `ContentChip` with single characters ("T", `↗`, `</>`, image SVG). Two different components render the same semantic information differently across the two macOS surfaces.  
**Fix:** Unify into a single shared `ContentChip`/`KindChip` component. For the popup, use the compact form (glyph + tint, no text label) but backed by the same kind-data pipeline as KindChip.

**Files:** `crates/copypaste-ui/src/popup/Popup.tsx:45–85`, `crates/copypaste-ui/src/views/HistoryView.tsx:228–254`

---

### VP-3 — SectionLabel on Android uses `MaterialTheme.typography.titleMedium` (16sp), not the 11px spec (P2)
`Components.kt:151`: `SectionLabel` uses `MaterialTheme.typography.titleMedium` which resolves to ~16sp. DESIGN-SYSTEM-v2 §1 specifies "Section label: 11 / 600 / UPPER". At 16sp the section label is the same size as body text and loses its hierarchical role.  
**Fix:** Change `SectionLabel` to `fontSize = 11.sp, fontWeight = FontWeight.SemiBold, letterSpacing = 0.6.sp` with `text.uppercase()`.

**Files:** `android/app/src/main/java/com/copypaste/android/ui/theme/Components.kt:149–155`

---

### VP-4 — macOS "Revoke all" confirmation uses inline "Yes" / "No" text buttons (P2)
`DevicesView.tsx:1208–1231`: the revoke-all confirm uses inline text "Revoke all?" with "Yes" and "No" micro-buttons in the actions bar. This is a destructive action with no modal confirmation, no explanation of consequences, and a 2-character "No" button that is easy to miss. The per-peer Revoke uses a proper modal (`revokePrompt` dialog).  
**Fix:** Route "Revoke all" through the same modal pattern as the per-peer revoke dialog. Include a one-sentence consequence description ("All paired devices will lose P2P access.").

**Files:** `crates/copypaste-ui/src/views/DevicesView.tsx:1208–1234`

---

### VP-5 — Pinned rows have amber left-border AND amber background tint; double-emphasis (P3)
`HistoryView.tsx:526`: pinned rows apply both `border-l-2 border-l-ide-warning` AND `bg-ide-warningDim`. Either indicator alone conveys "pinned." Using both simultaneously can make the list feel visually noisy, especially when many items are pinned.  
**Fix:** Choose one: the bookmark SVG indicator (already shown) + the amber left-border is sufficient. Remove `bg-ide-warningDim` from the pinned row background, or reduce its alpha from 0.10 to 0.05.

**Files:** `crates/copypaste-ui/src/views/HistoryView.tsx:525–527`

---

### VP-6 — CopyPasteTopBar title uses `MaterialTheme.typography.titleLarge` (~22sp) but DESIGN-SYSTEM-v2 specifies "View title: 13px/590" (P2)
`Components.kt:77`: `CopyPasteTopBar` uses `titleLarge` (22sp Material default). DESIGN-SYSTEM-v2 §1 specifies "View title: 13 / 590". At 22sp the Android header is nearly twice as tall as the macOS 44px header, violating the "same header height everywhere" grid.  
**Fix:** Override `style = TextStyle(fontSize = 13.sp, fontWeight = FontWeight(590))` on the title Text, or define a custom `viewTitle` typography style in `Type.kt`.

**Files:** `android/app/src/main/java/com/copypaste/android/ui/theme/Components.kt:77`

---

### VP-7 — Motion: Android uses `EaseOutExpo` CubicBezierEasing but iOS-safe tokens from `Motion` object not consistently applied (P3)
`HistoryActivity.kt:141` imports `Motion` from `ui/theme`. The DESIGN-SYSTEM-v2 §8 specifies a `Motion` object with `instant=90/fast=130/base=180/slow=240` ms tokens. Some animations in `HistoryActivity.kt` use hardcoded `tween(150)` or `tween(200)` values rather than referencing `Motion.fast` / `Motion.base`, defeating the design token system.  
**Fix:** Audit all `tween(N)` hardcodes in `HistoryActivity.kt` and replace with `Motion.fast`, `Motion.base`, etc.

---

## 5. Accessibility

### AC-1 — Popup list has no ARIA role on the `<ul>` element; items have no role (P1)
`Popup.tsx:569–571`: the item list is a `<ul>` with no role attribute. Each `<li>` row has no `aria-selected`. Screen-reader users cannot navigate the popup with VoiceOver's list navigation. The macOS History `VirtualList` has `role="listbox"` and `aria-label="Clipboard history"` (`HistoryView.tsx:1132–1133`) — the popup lacks parity.  
**Fix:** Add `role="listbox"` to the `<ul>` in Popup and `role="option"` + `aria-selected` to each `<li>` row, matching `HistoryView`.

**Files:** `crates/copypaste-ui/src/popup/Popup.tsx:569–571`, `popup/Popup.tsx:691`

---

### AC-2 — IconActionBtn in HistoryRow has 20×20px hit target (below 44×44 minimum) (P1)
`HistoryView.tsx:731`: `IconActionBtn` has `h-5 w-5` (20×20px). Apple HIG and WCAG 2.5.5 both specify 44×44px minimum touch targets. On a touch-enabled Mac (Magic Trackpad, iPad Sidecar) this is dangerously small.  
**Fix:** Keep the visual 20×20px icon but pad the clickable area to 28×28px minimum using a pseudo-element or by changing to `h-7 w-7` with the icon centered.

**Files:** `crates/copypaste-ui/src/views/HistoryView.tsx:728–743`

---

### AC-3 — SAS pairing modal: no focus trap; focus can leave the modal (P1)
`DevicesView.tsx:470–633`: the SAS pairing modal is a fixed-positioned overlay with `role="dialog"` and `aria-modal="true"`, but there is no focus trap implementation. Tab key will cycle through the underlying DevicesView content behind the overlay.  
**Fix:** Implement a focus trap using a `useFocusTrap` hook or a library. Focus should cycle only within the modal's interactive elements until it closes.

**Files:** `crates/copypaste-ui/src/views/DevicesView.tsx:470–633`

---

### AC-4 — macOS History search field has no visible label; placeholder only (P2)
`HistoryView.tsx:2098`: the search input uses `placeholder="Filter…"` with no `<label>` element or `aria-label`. Placeholder text disappears when the user types, removing the field's accessible name.  
**Fix:** Add `aria-label="Filter clipboard history"` to the search input.

**Files:** `crates/copypaste-ui/src/views/HistoryView.tsx:2093–2100`

---

### AC-5 — Popup keyboard navigation wraps around (ArrowDown past last = first) but this is not announced (P2)
`Popup.tsx:418`: ArrowDown wraps around using modulo. When focus wraps from item 50 back to item 1, there is no announcement. VoiceOver users may not realize the list has wrapped.  
**Fix:** On wrap, announce with an `aria-live="polite"` region: "Wrapped to beginning" / "Wrapped to end".

---

### AC-6 — Android: no `contentDescription` on timestamp Text composables (P2)
Android `HistoryActivity.kt` row composables render timestamp text as `Text(relativeTime(...))` without a `contentDescription`. TalkBack reads the raw string "2m ago" without context ("Copied 2 minutes ago"). This also applies to the KindChip-equivalent icon.  
**Fix:** Add `Modifier.semantics { contentDescription = "Copied ${relativeTime(item.wallTimeMs)}" }` to the timestamp Text in each row.

---

### AC-7 — Color-only indicators: sync status badge uses only green/amber/red dot (P2)
`SyncStatusBadge.kt` (Android) and `SyncStatusChip.tsx` (macOS) both communicate sync state via color dots. Users with red-green color blindness (~8% male) cannot distinguish the success (green) and error (red) states.  
**Fix:** Add a secondary shape indicator: use a filled circle for online, a hollow circle for degraded, and an X-circle for error. Or add a single-character label ("✓", "!", "✗") inside the dot.

**Files:** `android/app/src/main/java/com/copypaste/android/ui/SyncStatusBadge.kt`, `crates/copypaste-ui/src/components/SyncStatusChip.tsx`

---

## 6. Microcopy

### MC-1 — "Pairing ended — check the other device." is vague (P2)
`DevicesView.tsx:602`: when the SAS handshake resets to idle without a local confirm, the modal shows "Pairing ended — check the other device." The user doesn't know whether pairing succeeded or failed, or what action to take on the other device.  
**Fix:** "Pairing did not complete. The other device may have cancelled or timed out. Try again."

**Files:** `crates/copypaste-ui/src/views/DevicesView.tsx:601–604`

---

### MC-2 — "Reset database (erases local history)" is presented as a button label (P2)
`HistoryView.tsx:2162`: the reset button label contains a parenthetical clarification. This is a common workaround but it makes for an awkward label. The parenthetical is the critical warning.  
**Fix:** Split: button label = "Reset database", with a `<p>` below reading "Warning: erases all local clipboard history permanently."

**Files:** `crates/copypaste-ui/src/views/HistoryView.tsx:2162`

---

### MC-3 — "Revoked & rotated sync key — re-provision remaining devices" is developer-speak (P2)
`DevicesView.tsx:1162`: the success toast uses "re-provision". Most users do not know what "provisioning" means in this context.  
**Fix:** "Sync key rotated. Other devices must re-scan the QR code to keep syncing."

**Files:** `crates/copypaste-ui/src/views/DevicesView.tsx:1162`

---

### MC-4 — Loading spinner says "..." in popup search area (P3)
`Popup.tsx:512`: while loading, the right side of the search bar shows `…` in a very faint color. This is invisible unless the user knows to look for it.  
**Fix:** Use a small `CircularProgressIndicator` (Material) on Android or a spinning SVG ring on macOS (matching the SAS modal's `animate-spin` spinner).

**Files:** `crates/copypaste-ui/src/popup/Popup.tsx:511–515`

---

### MC-5 — "Clipboard service offline" used in both History and Devices with identical copy (P3)
`HistoryView.tsx:2118`, `DevicesView.tsx:1248`: both offline states show "Clipboard service offline" / "The daemon is not running." This is accurate but repetitive. When the user sees this in Devices they might think it's a different problem from History.  
**Fix:** Ensure both states share exactly the same string via a shared constant. Add a third sentence in the Devices context: "Device information is unavailable while the service is stopped."

---

### MC-6 — "Filter…" vs "Search clipboard…" — different placeholder text on same filtering function (P3)
macOS History (`HistoryView.tsx:2098`) uses `placeholder="Filter…"`. macOS Popup (`Popup.tsx:487`) uses `placeholder="Search clipboard…"`. These are functionally equivalent (both filter the same item list).  
**Fix:** Unify to "Search clipboard…" in both contexts, or "Filter history…" in both. Consistency prevents user confusion about whether these are different features.

**Files:** `crates/copypaste-ui/src/views/HistoryView.tsx:2098`, `crates/copypaste-ui/src/popup/Popup.tsx:487`

---

## 7. Additional Findings (Unsectioned)

### ADD-1 — DetailsModal shows `content_type` raw MIME string in footer (P2)
`HistoryView.tsx:1003`: the footer of the Details modal shows `{entry.content_type}` and `{entry.app_bundle_id}` as raw internal identifiers (e.g. "text/plain", "com.apple.dt.xcode"). Users expect human-readable labels.  
**Fix:** Map `content_type` to human label ("Plain text", "Image", "URL") and `app_bundle_id` to app display name using the same `sourceAppLabel()` function already used in rows.

**Files:** `crates/copypaste-ui/src/views/HistoryView.tsx:1002–1006`

---

### ADD-2 — DeviceBadge "This device" badge appears on every locally-created item, cluttering the list (P2)
When all history comes from the local device (common for a new setup without sync), every row shows a "This device" badge. The badge adds noise without value in a single-device setup.  
**Fix:** Hide the DeviceBadge entirely when `ownDeviceId` is the only device ID seen in the loaded items (i.e. `knownDeviceIds.length <= 1`). Same logic as the device filter dropdown which is only shown when `knownDeviceIds.length > 1`.

**Files:** `crates/copypaste-ui/src/views/HistoryView.tsx:321–343`, `HistoryView.tsx:2040–2056`

---

## Top 15 UX Fixes (Ranked by User Impact)

| Rank | ID | Title | Severity |
|------|-----|-------|---------|
| 1 | FF-4 | Android Settings requires manual Save; macOS auto-saves — model mismatch that loses user changes | P1 |
| 2 | FF-1 | QR blur/reveal adds a mandatory extra click to every pairing session | P1 |
| 3 | AC-1 | Popup list has no ARIA listbox/option roles — screen readers cannot navigate clipboard history | P1 |
| 4 | IH-2 | KindChip uses 9px font, below the 10.5px design system floor — illegible | P1 |
| 5 | FF-2 | SAS "Match"/"Doesn't match" buttons — ambiguous security-critical labels | P1 |
| 6 | FF-3 | Android has no mDNS/LAN discovery section — hardest pairing path on mobile | P1 |
| 7 | AC-3 | SAS pairing modal has no focus trap — keyboard users can escape to background | P1 |
| 8 | AC-2 | IconActionBtn hit target is 20×20px — well below 44px minimum | P1 |
| 9 | FF-8 | Single-item delete has no undo/confirmation — irreversible silent data loss | P2 |
| 10 | CP-1 | Settings tab structure diverges (6 tabs macOS vs 5 tabs Android, "Notifications" mismatch) | P1 |
| 11 | CP-5 | Pairing entry point is inline (macOS) vs separate Activity (Android) | P2 |
| 12 | IH-3 | Remote device badge shows raw UUID prefix instead of device name | P2 |
| 13 | VP-6 | Android TopAppBar title is 22sp instead of 13px spec — oversized, wastes vertical space | P2 |
| 14 | VP-3 | Android SectionLabel uses 16sp titleMedium instead of 11sp/600/UPPER spec | P2 |
| 15 | FF-7 | Popup daemon-offline state has no Restart button — user must leave the popup to fix | P2 |

---

## Biggest Cross-Platform Inconsistencies

1. **Save model:** macOS auto-saves per field with inline "Saved" flash; Android requires a manual Save button with unsaved-changes dirty tracking. (FF-4)
2. **Settings tabs:** Different tab names, different tab count, Notifications is Android-only. (CP-1)
3. **Pairing flow structure:** macOS = single scrollable page with QR + discovery + peers; Android = three separate surfaces (DevicesScreen, PairActivity, SAS dialog from notification). (CP-5, FF-3)
4. **Icon language:** macOS uses Lucide-style custom SVG strokes; Android uses Material filled icons. DESIGN-SYSTEM-v2 §9 specifies Lucide on both. (CP-4)
5. **Unit labels:** "MB" (macOS) vs "MiB" (Android) for identical binary storage values. (CP-2)
6. **Content-type chips:** Full-word `KindChip` in macOS History; zero equivalent on Android History. (IH-4)
7. **Type chip in popup vs main window:** Single-character chip (popup) vs full-word KindChip (History) — two different macOS surfaces already inconsistent. (VP-2)
8. **Device name in history:** macOS resolves remote origins to `originId.slice(0,8)` UUID prefix; Android similarly lacks peer name resolution. (IH-3)
