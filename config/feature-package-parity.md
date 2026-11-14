# Feature package: parity checklist and evidence status

This file covers the clipboard productivity package: Paste Queue, Snippets (with templates, clip editing and abbreviation search), Text Transforms, Content Rules and Android Quick Access (keyboard panel, tiles, widget and desktop tray parity). AI tools (the MCP server) are not part of the core app; see section 1.6. It records:

- which entry points exist on each platform;
- the evidence status of every acceptance criterion (AC) in the five core feature specs;
- the stated policies that ship with the package: backup compatibility, privileged operations, Linux limits and the design-system exception for native Android surfaces;
- the resource behavior that was implemented. Native resource qualification has not been established for any of it.

CopyPaste ships on macOS, Windows and Android. Linux is built from the desktop code and is listed only where its behavior differs.

## Status legend

| Code | Meaning |
|---|---|
| U | Automated Rust or Dart test. Runs in CI (`rust-and-supabase`, `macos-core`, `windows-core`, `flutter-foundation`). |
| UK | Kotlin Robolectric test (`apps/copypaste_flutter/android/app/src/test`). Every feature WP ran these locally. The `android-unit-tests` CI job added by this package runs them from now on (decision U8). Until that job has passed in CI, treat this evidence as local only. |
| N | Native verification in the test VM, an emulator or a physical device. **Pending for every row marked N.** None has been recorded yet. |
| X | Deferred by a recorded decision. Not in v1. |
| G | Gap: in scope, but not implemented in this package. Listed under follow-ups. |
| Dev | Accepted deviation from the spec. The reason is given in the row. |

A row marked "U + N" has its logic covered by tests, while its platform behavior still needs native evidence.

## 1. Entry-point parity (spec section 5.1 tables)

Every row below still needs native verification on macOS, Windows and Android (paste-queue AC30). GUI checks were not run on the host.

### 1.1 Paste Queue

| Entry point | macOS | Windows | Android |
|---|---|---|---|
| Start or stop Collecting | Global hotkey (⌃⌘C bound by default); queue drawer from the header action; Quick Paste Queue tab; menu bar row opens the drawer | Hotkey **unbound** by default, with a "Set shortcuts" prompt (decision P2); drawer; Quick Paste Queue tab; tray row opens the drawer | IME Queue tab (Collect); app queue drawer (header action). Tile long-press: not provided (decision A3) |
| Add specific items | History multi-select and row menu, keeping selection order; Quick Paste Mod+Shift+Q; snippet "Add to queue" | Same as macOS | History (inspector drawer and bulk selection); snippets in the IME; text-selection "Add to Paste Queue"; Share sheet |
| Paste next | Global hotkey ⌃⌘V; Quick Paste Queue tab. The main-window drawer only copies (see 7.4) | Hotkey unbound by default; Quick Paste Queue tab | IME "Insert next". **G**: widget "Copy next" (decision P4) is not implemented. Tile "Copy next": Dev (decision A3) |
| Paste all | Quick Paste Queue tab; queue drawer | Same | IME Queue tab; app queue drawer |
| Manage (reorder, edit, remove, Reverse, separator) | Queue drawer and Quick Paste Queue tab | Same | IME Queue tab (drag handles, move actions) and app queue drawer |
| Floating mini panel | X (decision P3) | X | — |

### 1.2 Snippets

| Flow | macOS (and Linux) | Windows | Android |
|---|---|---|---|
| Browse and manage | Settings → Snippets section | Same | Settings → Snippets section |
| New snippet | ⌘N in the Snippets section; menu bar "New snippet" | Ctrl+N; tray "New snippet" | "+" action in the Snippets section |
| New one-off clip | ⌘N in History | Ctrl+N in History | "+" toolbar action in History |
| Save clip as snippet | ⌘S on the selected History clip; History row menu; inspector More menu, also inside Quick Paste. Quick Paste binds no ⌘S | Same, with Ctrl+S | Inspector drawer More menu. IME panel: G (no "Save as snippet" action) |
| Edit | ⌘E on the selected clip; editor drawer from the snippet list | Ctrl+E | Tap the row, then Edit |
| Use snippet | Quick Paste: type the abbreviation, Return (exact match preselected); Snippets tab | Same, with Enter | IME Snippets tab: tap or search; widget Snippets filter: tap |
| Import / Export | Snippets section import and export row | Same | Same |

### 1.3 Text Transforms

| Flow | macOS | Windows | Android |
|---|---|---|---|
| Transform a history item | History and Quick Paste: Mod+T palette; row and inspector "Transform…" | Same, with Ctrl+T | Inspector drawer "Transform…"; IME "Transform…" item action and Transforms tab |
| Transform the clipboard or a selection | Desktop selection hotkey and Services: X (decision T2) | X | IME Transforms tab (Selection / Clipboard / History / Snippet sources); `PROCESS_TEXT` selection menu; Quick Settings tile through the transform sheet |
| Default paste pipeline | Quick Paste "Pasting with" chip; bypass with Mod+Shift+Enter. A per-app default follows the frontmost app's bundle ID | Same; the target's lowercase image name | IME history inserts, with an "Insert original" action; the editor's package name |
| Settings → Transforms | Yes | Yes | Yes |

### 1.4 Content Rules

| Entry point | macOS | Windows | Android |
|---|---|---|---|
| Settings → Rules | Yes | Yes | Yes |
| History "Create rule from this item…" | **G** | **G** | **G** |
| Inspector "Why is this here…" (rule trace) | Yes (History and Quick Paste inspector) | Yes | Yes (inspector drawer) |
| Recently skipped | Menu bar "Recently skipped (n)"; Rules section dialog | Tray; Rules section dialog | Rules section dialog |
| IME long-press "Create rule…" | — | — | **G** |

### 1.5 Android Quick Access and desktop tray parity

| Entry | Android | Desktop (macOS, Windows, Linux) |
|---|---|---|
| Insert from history or snippets | System IME switcher, then the CopyPaste panel | Global Quick Paste hotkey; tray/menu "Open Quick Paste (shortcut)" |
| Save current clipboard | Tile "Save clipboard"; widget header Save; Share to CopyPaste | Tray/menu "Capture now" |
| Pause and resume | Tile "Capture toggle"; app | Tray/menu toggle and "Pause for…" |
| Glance and copy recent | Widget (History, Pinned and Snippets filters; privacy level) | Tray/menu "Recent (N)" |
| Pull from another device | **G**: no app or widget action (decision A3 assigns it there). Tile T4: Dev (decision A3) | Tray/menu "Copy latest from other device" |

### 1.6 AI tools (moved to an optional module)

AI tools are not part of the core app. The MCP server (`copypaste mcp`), Settings → AI tools, first-use approval and "Hide from AI tools" move to the planned optional `ai-tools` module, delivered through the module marketplace. Their parity and acceptance criteria will be tracked with that module.

Core keeps only a neutral hook for such a module: the synced per-clip flag `hidden_from_integrations` and the `hide_from_integrations` rule action. The app shows neither while no integration module is installed. Built-in rules only mark detected credentials and payment cards secret; secret clips are excluded from any integration.

## 2. Acceptance-criteria status

The owning work package (WP) follows the plan's section 8.

### 2.1 Paste Queue (spec section 7)

| AC | Status | Evidence and notes |
|---|---|---|
| 1–4, 6–8, 10, 14–25 | U | Runtime queue semantics (Q1, `crates/copypaste-runtime/src/features/paste_queue/tests.rs`) and desktop UX (Q2, `test/features/paste_queue`) |
| 5 | U | I2: bulk "Add to Paste Queue" keeps the order of selection, and a range keeps display order |
| 9 | N | F7 modifier safety, ⌥-based hotkey in Finder |
| 11 | U + N | Fake host without Accessibility permission (Q2). Native banner and clipboard-only fallback on macOS still need checking |
| 12, 13 | N | Windows elevated target; exclusion from Win+V clipboard history |
| 26, 29 | UK | `PanelQueueTabTest` |
| 27 | UK + N | `QueueIntakeTest`; selection menu and Share on a device |
| 28 | Dev + G | The fixed tiles are Save clipboard and Capture toggle (decision A3). "Copy next" belongs in the widget header and the IME (decision P4); the widget action is missing |
| 30 | N | Section 1 of this file is the checklist. Native walkthrough pending |
| 31 | N | Idle trace pending; unqualified until `feature_budgets.paste_queue_idle_wakeups_per_hour` exists (RUN-111) |
| 32 | N | VoiceOver, Narrator, TalkBack. The IME has custom actions and "n of N" announcements (A2) |
| 33 | U + UK | Masking through the shared `ClipVisibility` helpers (Q2, A2, A4) |
| 34 | U | Q2 shortcut conflict handling |
| R35, R39 (floating panel) | X | Decision P3 |
| R13 (advance on native paste) | X | Decision P1 |

### 2.2 Snippets (spec section 7)

| AC | Status | Evidence and notes |
|---|---|---|
| 1, 7 | U | S6 editor tests |
| 2–4, 6 | U | S4 clip editing |
| 5, 21, 26–28, 30–35 | U | S2 storage, conflicts and search; S3 runtime |
| 8–13, 15, 17, 18, 20, 22 (lint), 24 | U | S1 template crate |
| 25 | U + N | Under-50 ms timing on reference hardware is native evidence (RUN-112) |
| 14, 16, 19, 44 | U | S3; 19 (UI) in S6 |
| 22 (Android caret) | UK + N | `PanelSnippetsTabTest`; caret placement on a device |
| 23 | N | Desktop caret with emoji and CRLF (F7, I1) |
| 27 | U | I1 Quick Paste exact match preselected; IME tier label (A2) |
| 29 | N | Code review: no global keystroke hook added on any platform |
| 36–40 | U | S5 import and export |
| 41 | U | S3, M1, A4, S5 |
| 42 | U | Log assertions in S3 and R4 |
| 43 | N | Native idle trace (RUN-112) |
| 45 | N | Widget template fill-in and literal copy (A3, A4) |
| 46 | N | "Copied — paste manually" where synthesized paste is blocked (F7, I1) |
| Rich-text editor | X | Decision S1 |
| Alfred and TextExpander import | X | Decision S7 |
| Date pattern builder, "Remember last values" (T-20), collection picker in the manual clip editor, Undo after deleting a collection | G | S6 deviations |

### 2.3 Text Transforms (spec section 7)

| AC | Status | Evidence and notes |
|---|---|---|
| 1 | U + N | Catalogue listing (T1–T6); palette on each platform |
| 2 | U + N | Golden corpus on the host. An Android run of the corpus (`cargo test --target`) is not in CI, so Android byte identity is N |
| 3–23 | U | T1–T4 |
| 24, 26–32, 34, 43 | U | T5 runtime; T6 UI states; 34 also I1 |
| 25 | U | T5 with F0 (P2P and fake cloud) |
| 33 | N | Windows elevated target |
| 35 | U | T5: a per-app default for another platform is inert |
| 36–38 | UK | `PanelTransformsTest` |
| 39, 41 | UK + N | `TransformIntakeTest`, `TileStateTest` |
| 40 | N | Android 13+ toast and `EXTRA_IS_SENSITIVE` |
| 42, 48 | U | T6 |
| 44 | N | Palette open and close 50 times (RUN-114) |
| 45, 46 | U | No network crate in the transform dependency tree; log assertions |
| 47 | N | Screen readers (T6, A2) |
| Desktop selection hotkey and Services | X | Decision T2 |
| CLI `copypaste transform` | X | Decision T4 |
| Tier 2 transforms | X | Decision T1 |

### 2.4 Content Rules (spec section 7)

| AC | Status | Evidence and notes |
|---|---|---|
| 1 | U + N | Existing privacy settings plus R2 |
| 2 | U | R5 Android source-app notice; existing excluded apps on the desktops |
| 3, 17, 18, 24 | U | R2 capture and receipt hook |
| 4–7, 9–12, 33 | U | R1 engine |
| 8 | X | "Move to collection" removed (decision R3) |
| 13 | U + N | R1 on the host; Android runs the same crate |
| 14 | U + Dev | Per-condition trace and transformed output (R4, R5). Regex match highlighting is missing: the IPC has no match ranges |
| 15, 16, 29–32 | U | R4 |
| 19, 22, 23, 25, 27 | U | R3 |
| 20 | U + N | Purge within 5 s of wake (RUN-107) |
| 21 | U + UK + N | R3, A1 |
| 26 | U + Dev | Masking is tested. Secrets sync end-to-end encrypted like any item (decision R2, capture policy), not "local unless enabled" |
| 28 | U | F0, F3, R4 |
| 34 | U | "Don't save" alone is enforced. Pin is removed as an action (decision R3) |
| 35, 36 | U + UK + N | R5 and A1. "Create rule from this item" is **G** (section 1.4) |
| 37 | N | Unqualified until `rules_capture_evaluation_ms` and `rules_evaluator_bytes` exist (RUN-109) |
| 38 | N | By design there is no alarm job (RUN-108). Confirm with `dumpsys` |
| Window-title condition | X | Decision R4 |

### 2.5 Android Quick Access (spec section 7)

| AC | Status | Evidence and notes |
|---|---|---|
| 1–3 | UK + N | `PanelCaptureTest`. The toast exemption and hidden-panel callbacks need device checks (decision A8) |
| 4, 5 | UK + N | `TileStateTest`; existing share path |
| 6–10 | UK | `PanelInsertTest`, `PanelPrivacyTest`. 10 also needs a device check of the caret |
| 11, 12 | UK + N | Return to the previous keyboard |
| 13, 14 | U + UK | S3, I1, A2 |
| 15–17 | UK | `PanelQueueTabTest` |
| 18–22 | UK + N | `PanelPrivacyTest`; lock screen UI dump; before first unlock (decision A5) |
| 23–25 | UK | `TileStateTest` |
| 26–30 | UK | `WidgetRefreshTest`, `WidgetPrivacyTest` |
| 31 | U | A6 tray (macOS, Windows, Linux) |
| 32, 33 | N | Concealed pasteboard marker and Win+V exclusion; macOS secure field opens masked mode |
| 34 | Dev, U | Follows snippets AC33 (a later edit restores; decision S2) |
| 35 | U | S2 |
| 36 | Dev | Usage counters are local only and are not summed across devices (decision A9) |
| 37 | N | TalkBack custom actions |
| 38 | N | Resource runs for the `feature_budgets.android` keys (RUN-116, RUN-117) |

## 3. Kotlin evidence

Kotlin unit tests run under Robolectric with JDK 25 and the Gradle 9.8.0 wrapper that the Flutter tool generates (the wrapper scripts are not committed). Before this package no CI workflow ran them (plan fact V23). The `android-unit-tests` job in `.github/workflows/ci.yml` now runs `:app:testDebugUnitTest`. It is listed in `config/ci-policy.json`, so `CI required` depends on it.

The package's Kotlin tests:

- `ime/PanelCaptureTest`, `PanelInsertTest`, `PanelPrivacyTest`, `PanelQueueTabTest`, `PanelSnippetsTabTest`, `PanelTransformsTest`
- `quickaccess/QueueIntakeTest`, `TileStateTest`, `TransformIntakeTest`
- `widget/WidgetPrivacyTest`, `WidgetRefreshTest`
- `DeviceAuthenticatorTest`, `QuickAccessChannelTest`, `RuntimeAccessTest`, `ClipboardFloatingActivityTest`, `AndroidCaptureFeedbackTest`

Robolectric does not lay out list views, so drag reordering in the IME Queue tab has no unit test (N).

On 2026-11-13 the CI job's exact command sequence ran locally on the integrated branch (`flutter build apk --debug --config-only`, then `./gradlew --no-daemon :app:testDebugUnitTest` with JDK 25). All 165 tests in 37 classes passed. This is local evidence; the first CI run of the job has not happened yet.

## 4. Stated policies

### 4.1 Backup compatibility

Restore accepts only a backup whose schema matches the current one exactly. There is no migration of a staged backup before it is verified.

- Migration `010_feature_package.sql` therefore makes backups created by earlier builds unrestorable by this build, as every earlier migration did.
- Such a restore is refused with `RestoreError::InvalidBackup`, and live history is left intact.
- The release notes must state this (decision U7).

### 4.2 Privileged operations

Un-hiding a clip from integrations (`ClipOp::SetHiddenFromIntegrations { hidden: false }`) is accepted only from the CopyPaste app (WP-F10), so another local client cannot widen what an integration module may see. This is not a boundary against a process that runs as the same OS user: such a process can already run `copypaste get`, read the clipboard or drive the UI. Any other IPC peer gets `ErrorCode::Forbidden`. Verification runs on the first privileged request of a connection and is then kept for that connection:

- macOS: audit token → `SecCodeCopyGuestWithAttributes` → `SecCodeCheckValidity`. The requirement is the daemon's own designated requirement with the identifier `com.copypaste.app`. Release builds are re-signed locally with a self-signed certificate, so the check cannot use an Apple team id.
- Windows: the pipe client process image must be `CopyPaste.exe` next to the daemon, signed by the same Authenticode subject as the daemon (`WinVerifyTrust`, cache only).
- Linux (weaker): the canonical `/proc/<pid>/exe` path must equal the installed app binary. It is path-based, has no signature, and pid reuse is possible between capture and check.
- Unsigned development builds: only in debug builds with an unsigned daemon, a peer whose path is the development app path is accepted.

Known limit:

- `SnippetOp::SetPreference` is not privileged, so any same-user IPC caller can turn off `snippets.require_auth`. That is consistent with this model but is open for review (follow-up).

### 4.3 Device authentication and peer identity on Linux

- Linux has no device-authentication adapter. `availability()` reports `unsupported`.
- Concealed snippets, the Quick Paste secret unlock and the tray copy of a secret fall back to an in-app confirmation.
- Quick Paste on Linux does not explain that password fields cannot be detected there (quick-access D3). Masked mode is never entered on Linux.
- Quick Paste applies a per-app default paste pipeline only on X11, where the target window's class names its desktop entry. Wayland companions do not report the target app, so the global default applies there.
- Peer identity on Linux is path-based (section 4.2).
- Linux is not in the parity triad.
- On Windows builds without the window interop (before 22000), Windows Hello falls back to the unparented `UserConsentVerifier` prompt.
- On Android API 24–28 the confirm-credential screen is used instead of `BiometricPrompt`.

### 4.4 Clients that predate organization records

Cloud rows for organization records carry the provider-safe id `org-v1-<64 hex>` (WP-F0). Builds older than organization records stored unknown rows as binary clips, so such a build would show a pulled `org-v1-` row as a junk binary clip that is never adopted. Those builds predate the `app_versions >=1.3.0` range of the shipped `copypaste.supabase` module 0.1.29 and cannot run it, so the case is documented here and not handled. Older builds that do know organization records decline the new kinds (plan fact V3). P2P peers without `organization_kinds` are not offered the new kinds at all (RUN-120).

### 4.5 Design-system exception for native Android surfaces

The keyboard panel, Quick Settings tiles, home widget and quick-access sheets use framework Android Views and RemoteViews, not Flutter and shadcn_flutter (decision A1). A Flutter engine in the IME would cost tens of MB while the keyboard is shown.

- They render the same design tokens: `res/values/design_tokens.xml` and `res/values-night/design_tokens.xml`.
- `test/app/theme/android_design_tokens_test.dart` fails when the tokens drift.
- Icons are Lucide vector drawables.

The decision also asks for a one-line exception in AGENTS.md. AGENTS.md is no longer tracked in this repository, so this package could not commit that line. The line to add is:

> Native Android surfaces that cannot host a Flutter engine (keyboard panel, Quick Settings tiles, home widget) use framework Views and RemoteViews rendering the mirrored design tokens, guarded by the token-parity test.

## 5. Resource behavior

Every entry is `implemented_pending_native_evidence`, and native resource qualification is `not_established` on every listed platform. No native measurement was taken. All feature budgets in `config/runtime-resource-budgets.json` (`feature_budgets`) are null, which means unresolved and never passing.

The plan numbered these entries RUN-104 to RUN-119. Those ids were taken on the branch by entries that landed after the plan's baseline, so the registry uses RUN-107 to RUN-122. RUN-115 (the MCP stdio process) moved with AI tools to the planned optional module:

| Plan id | Registry id | Behavior |
|---|---|---|
| RUN-104 | RUN-107 | One deadline task; timers only within 1 h; resume sweeps on every desktop OS |
| RUN-105 | RUN-108 | Read-time expiry filtering; no Android alarm job |
| RUN-106 | RUN-109 | Rules compiled once per revision; bounded prefixes |
| RUN-107 | RUN-110 | Receipt-side protection reuses the evaluator; local rule tags |
| RUN-108 | RUN-111 | Paste queue holds references; no idle timers |
| RUN-109 | RUN-112 | Snippet search projection; bounded template cache released on close |
| RUN-110 | RUN-113 | Template date formatters built on demand |
| RUN-111 | RUN-114 | Lazy transform catalogue; bounded, debounced, cancellable previews |
| RUN-113 | RUN-116 | Engine-free IME panel over the blocking facade |
| RUN-114 | RUN-117 | Event-driven widget; active-mode tiles |
| RUN-115 | RUN-118 | Cloud withholds items expiring within 1 h |
| RUN-116 | RUN-119 | Tray Recent built on menu open |
| RUN-117 | RUN-120 | P2P withholds new kinds from legacy peers |
| RUN-118 | RUN-121 | Coalesced library and progress events |
| RUN-119 | RUN-122 | One import plan and one rule-preview plan |

Integration-level behavior to measure with those entries:

- **Quick Paste (I1):**
  - No polling.
  - Each open adds one `secureFieldFocused` query and one default-pipeline read for the target app, plus a pipelines read when a default pipeline applies.
  - One 60 s one-shot timer runs only while secrets are unlocked.
  - The queue controller is created on first use.
  - Snippet rows are bounded (5 in History, 50 in the Snippets tab) and are dropped, with `SnippetOp::Release`, when the presentation ends.
  - Masked and locked rows load no secret or image content.
- **History (I2):**
  - Feature adapters are created lazily.
  - Each visible expiring clip has one timer, re-armed only when its countdown text changes.
  - The rule trace and its options load only when the "Why" dialog opens.
  - Keep patches the loaded row without re-reading it.
- **App wiring (I3):**
  - No new timers or polling.
  - Queue shortcuts share one `DesktopShortcutRegistrar`.
  - Settings controllers load only while their section is shown.
- **Settings sections (S6, T6, R5):**
  - Content loads on attach and is dropped on detach.
  - The only timers are editor pauses (snippet editor 300 ms, rule validation 300 ms, transform preview 120 ms), and they stop when the editor closes.
  - The rule-preview dialog sends Cancel on close to release the runtime plan.
- **IME tabs (A2):**
  - Tabs keep no state once the panel hides.
  - Every runtime call runs on the shared worker thread.
  - Template previews refresh 150 ms after a change, never on a timer.

Still to measure:

- P2P session bytes for a mixed-version pair (risk K2);
- the binary-size delta of ICU data, the Windows tzdb and the YAML parser (risk K3);
- IME, widget and tile resource runs on physical devices;
- the idle traces for paste-queue AC31, snippets AC43 and transforms AC44.

## 6. Native verification still to run

- **macOS (test VM):**
  - Touch ID and password prompts in the main window and the Quick Paste panel;
  - masked mode under Secure Event Input;
  - caret placement after a snippet paste;
  - the "Copied" notice when paste is blocked;
  - ⇧⌘Q in Quick Paste;
  - queue hotkeys and the modifier-safety check;
  - the concealed pasteboard marker;
  - the self-signed app accepted as a privileged peer while `copypaste-cli` is refused;
  - wake purge within 5 s.
- **Windows:**
  - a full daemon and runner build in CI (the peer-identity and device-authentication code was only cross-checked);
  - Windows Hello with the main window and Quick Paste HWND;
  - UI Automation masked mode;
  - an elevated target;
  - Win+V exclusion;
  - an Authenticode-signed `CopyPaste.exe` accepted while another binary from the same signer is refused.
- **Android (emulator and physical devices):**
  - toast exemption and callbacks while the panel is hidden (Pixel, Samsung, Xiaomi, GrapheneOS; decision A8);
  - `BiometricPrompt` on API 30+ and 29, and confirm-credential on API 26;
  - queue drag reordering;
  - template caret;
  - the image paste-via-clipboard path;
  - TalkBack custom actions;
  - lock-screen and before-first-unlock states;
  - tile and widget refresh counts.

## 7. Follow-ups

1. Gaps from section 1:
   - "Create rule from this item" (History and IME);
   - IME "Save as snippet" item action;
   - widget "Copy next" (decision P4);
   - Android app and widget "Copy latest from other device" (decision A3).
2. A native hook that hides the main window and reactivates the previous app, so the main-window queue drawer can paste instead of only copying (I3).
3. Quick Paste masks only authenticated secrets (`clip.secret`). History also masks clips that a rule marked secret (`HistoryClip.masked`). Aligning the two is a possible follow-up.
4. Report the target app from the Wayland companions so Quick Paste applies per-app default paste pipelines there (section 4.3).
5. Decide whether `SnippetOp::SetPreference` (`snippets.require_auth`) should be a privileged operation (section 4.2).
6. Add the AGENTS.md line in section 4.5.
7. Record native evidence for every N row, then the resource measurements in section 5, before claiming any qualification.
