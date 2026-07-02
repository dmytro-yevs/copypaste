# Behaviour & state coverage

Companion to `component-inventory.md` (view symbols). This proves **behaviour/state** coverage:
every non-composable behaviour owner, every manifest component, and every reachable interaction/state
has an owner, a disposition, an owning slice, and acceptance evidence.

**Disposition legend** — Visual: Restyle / Refactor / Preserve / New / Remove · Behaviour: Preserve /
Change / N-A. **State tag:** `E` existing→preserve · `EP` existing→new presentation · `N` new state
(needs plumbing) · `I` invisible/system-owned · `D` dead/remove.

---

## A. Behaviour-owner inventory (non-composable, user-visible)

| Owner | User-visible responsibility | Visual · Behaviour | Slice | Evidence |
|---|---|---|---|---|
| `HistoryItemActions.kt` | copy, bulk copy, delete, open/share/save file+image, action failures | Restyle feedback · Preserve | S5 | outcome unit + connected; toast localized |
| `HistoryUriHelper.kt` | URI grants, external open/share targets | N-A · Preserve | S5 | grant/intent unit; no-handler/error |
| `HistoryUrlUtils.kt` | URL host/path split for row/preview | N-A · Preserve | S5 | unit |
| `AppIconHelper.kt` | source-app icon load + fallback shown in History | N-A · Preserve | S5 | fallback rule + golden fixture (cached/missing) |
| `HistoryRowModel.kt` | masking/placeholder/row-state derivation | N-A · Preserve (security) | S5/S6 | existing `HistoryRowModelTest` + no-plaintext-semantics; links List↔Preview contract |
| `ErrorMessages.kt` | user-facing error mapping | N-A · Preserve classification | S13 | every message resourced/localized; classification unit |
| `LogExportHelper.kt` | log export/share ZIP + failure | Restyle feedback · Preserve IO/grants | S11 | success/failure toast localized; IO unit |
| `LogcatCaptureService.kt` | FGS notification + background-capture state | Restyle notif · Preserve lifecycle | S12 | channel localized; action tests |
| `NotificationPermissionHelper.kt` | request + permanently-denied routing | N-A · Preserve state machine | S10 | state→card/action mapping unit |
| `OemAutoStartHelper.kt` | OEM settings labels/intents/fallback | Restyle labels · Preserve resolver order | S10 | resolvable/unresolvable unit |
| `OnboardingPermissions.kt` | permission-state derivation + request routing | N-A · Preserve | S10 | state→card mapping (`OnboardingPermissionsTest`) |
| `DevicesRevokeActions.kt` | unpair/revoke ordering + error outcomes | N-A · Preserve (audit-first, local-only) | S7 | ordering unit; outcome→dialog binding |
| `PairController.kt` (+ `PairProvisioning`/`PairBootstrapSync`/`PairingApi`/`PairUtils`/`QrUtils`) | scan/SAS/progress/error transitions | N-A · Preserve protocol/IPC | S8 | each controller state → one UI state (`PairControllerTest`) |
| `ClipboardService.kt` | FGS status, pause/resume actions, copy-event | Restyle notif · Preserve service/actions | S12 | every notification state localized; action→state |
| `ServiceRestartWorker.kt` | restart + any posted notification | Restyle notif · Preserve scheduling | S12 | inventory posted notifications; behavioural regression |
| `SupabasePollWorker.kt` | background sync (surfaces sync-error banner state) | N-A · Preserve | S11 | error→banner mapping |
| `BootReceiver.kt` | post-boot restart → notification state | N-A · Preserve | S12 | behavioural regression, no restyle |
| `CaptureControlReceiver.kt` | Pause/Resume notification actions → state | N-A · Preserve | S12 | label→PendingIntent→state transition |
| `CaptureOverlayController.kt` | invisible 1×1 overlay lifecycle | N-A · Preserve (I) | S12 | suppress/restore idempotency unit |

## B. Manifest / app-component coverage

| Component | Type | Disposition | Slice |
|---|---|---|---|
| 13 Activities | Activity | see `component-inventory.md` | S4–S12 |
| `ClipboardService` | foreground Service | Preserve; localize/brand notifications | S12 |
| `LogcatCaptureService` | Service | Preserve; localize/brand | S12 |
| `BootReceiver` | BroadcastReceiver | Preserve | S12 |
| `CaptureControlReceiver` | BroadcastReceiver | Preserve; action tests | S12 |
| `ServiceRestartWorker`, `SupabasePollWorker` | WorkManager | Preserve; notif inventory | S11/S12 |
| `androidx.work.impl.WorkManagerInitializer` | startup provider (manual init) | Preserve; no UI | — |
| `androidx.core.content.FileProvider` (`${appId}.fileprovider`) | ContentProvider | **Preserve URI/grant contract** wherever a redesigned open/share/export action depends on it | S5/S11/S12 |

Providers need no redesign, but the FileProvider grant/path contract SHALL be preserved and tested
wherever History/Logs/Share actions rely on it.

---

## C. Complete interaction & state matrix

Every reachable action/state below has exactly one owning slice + evidence. `golden` = a Paparazzi
baseline exists (Y) / not applicable (N-A). Full color matrix is covered by token/contrast tests, not
goldens (see `android-visual-regression`).

### C1. App shell  (S4)
| Interaction / state | Result | Tag | golden · test |
|---|---|---|---|
| Tab select (Clips/Devices/Settings) | switch tab, accent-selected pill | E | Y · connected |
| Dirty-Settings nav guard | discard dialog intercepts tab switch | E | N-A · connected |
| System back | back per current tab / guard | E | N-A · connected |
| Selected-tab restore (config change / saved-state process death / cold=Clips) | restore or default | E | N-A · connected |
| Sync badge placement + sheet open | badge unobstructed; tap→sheet | E | Y · connected |
| System/gesture/IME/cutout inset change | pill repositions, no overlap | E | N-A · connected |
| Committed appearance publish vs draft isolation | app re-themes on Save only | EP | Y · unit+connected |

### C2. History  (S5)
| Interaction / state | Result | Tag | golden · test |
|---|---|---|---|
| tap-to-copy + echo suppression | item copied, no re-capture loop | E | N-A · unit |
| long-press → select mode; count; select-all/clear | multi-select UI | E | Y(selected) · connected |
| pin / unpin; pinned reorder | pinned group above Today | E | Y · unit |
| single / bulk delete + confirm | rows removed after confirm | E | N-A · connected |
| bulk copy excludes sensitive/non-text | only eligible content copied | E | N-A · unit (security) |
| search; device filter; clear filter | list filtered | E | N-A · connected |
| open/share/save file/image/URL + no-handler/error | action or explicit error | E | N-A · unit+connected |
| reveal guard → reveal → re-mask; partial-span mask | mask/unmask, no plaintext leak | E¹ | Y(masked synthetic) · security-semantics |
| load-more / pagination; concurrent refresh | append without dup/flicker | E | N-A · unit |
| source-app icon fallback | icon or fallback glyph | E | Y · unit |
| too-large / unavailable content actions | badge + disabled actions | E | Y · unit |
| loading / populated / empty / empty-private / no-results | correct state UI | E | Y · connected |
| **error/degraded list state** | persistent in-list error | **N (S5 owns presentation-state plumbing; no repo/IPC change)** | Y · connected |
| 12 kinds → 10 colors, glyph/swatch/thumbnail, SECRET lock | content-type tile | EP | Y · unit(token) |

¹ The overall reveal/mask user flow is preserved (E), but the pre-31 fallback **mechanism** itself
changes: today it is bullet substitution (`HistoryRow.kt`/`PreviewContent.kt` literally substitute
`"•••••••••••••"`); the redesign replaces it with a geometry-preserving opaque overlay over a
sanitized representation (§ List Masking Contract, `android-history/spec.md`; also `cross-platform-parity.md`).

### C3. Full-screen preview  (S6)
| Interaction / state | Result | Tag | golden · test |
|---|---|---|---|
| open / close / back / swipe gesture arbitration | overlay in/out; peek→pin state machine | E | N-A · `PreviewGestureTest` |
| copy/open/share/save availability by kind | actions vary by kind | E | N-A · unit |
| image loading / decode failure | spinner / explicit failure | E | Y · connected |
| file URI/grant failure | explicit error, stays on preview | E | N-A · unit |
| sensitive reveal guard + **Reveal action + semantics replacement (a11y fix)** | masked; Reveal unmasks; no plaintext in any semantics | **N (S6 introduces Reveal action + `revealed` state — no Reveal exists today; a11y fix bundled)** | Y(masked) · security-semantics |
| large content scroll/zoom | responsive, no OOM | E | N-A · unit |
| content-type color = shared source (no List/Preview divergence) | identical color | EP | N-A · unit |

### C4. Devices  (S7)
| Interaction / state | Result | Tag | golden · test |
|---|---|---|---|
| discovery start/stop/refresh | discovered list updates | E | N-A · connected |
| device card always-expanded (no collapse — see §F) | full §9.7 grid | E | Y · connected |
| fingerprint tap-to-copy + feedback | full 64-hex copied | **N (new for own-device AND paired-peer/roster cards — neither has tap-to-copy today; pattern reused from `PairedPeerList.kt`'s `onCopyFingerprint`)** | N-A · connected |
| pair discovered device | enters pairing | E | N-A · connected |
| unpair / revoke / rotate / revoke-all + cancel/retry/in-flight-dismiss rules | dialog states | E | Y(2) · connected + ordering unit |
| online/offline/reconnecting derivation | dot+label | E | Y(online+offline) · unit |
| QR refresh/countdown | progress/warning/regenerate | E | N-A · unit |
| auto-open SAS from notification | SAS modal opens | E | N-A · connected |
| cloud-account mismatch stays inert (gldr) | banner never shows | E | N-A · unit |
| own vs paired grids (§9.7), pills/badges, danger footer | field grids | EP | Y · unit |
| reduced-motion presence glow off | static dot | E | N-A · connected |

### C5. Pairing  (S8)
| Interaction / state | Result | Tag | golden · test |
|---|---|---|---|
| QR generate / reveal / expiry / regenerate | blur-at-rest, countdown | E | Y · unit |
| camera request / denial / permanent denial / settings recovery | camera or recovery UI | E | N-A · connected |
| scanner result / deep link / malformed / expired | proceed or error | E | N-A · unit |
| scan-review confirm / cancel | proceed to SAS or abort | E | Y · connected |
| six-digit SAS accept/reject (Match/Doesn't-match; fingerprint supplemental) | bootstrap or cancel | E | Y · connected |
| connecting/provisioning/bootstrap/sync phases | distinct phase UI | E | Y · connected |
| retry / cancel / success dismissal | recover or finish | E | N-A · connected |
| unconditional `FLAG_SECURE` (whole flow) | screenshots blocked | E | N-A · connected (window flag) |
| ZXing preview — FLAG_SECURE added (P0-1 security fix) | scanner UI unmodified visually, window now secure | N | Y · connected (window flag; local run per B4 policy) |

### C6. Settings  (S9)
| Interaction / state | Result | Tag | golden · test |
|---|---|---|---|
| tab switch with draft retention | drafts preserved | E | N-A · connected |
| dirty guard from tab nav / system back / top-bar back | discard dialog | E | N-A · connected |
| validation + Save enablement | Save gated on validity | E | Y(error) · connected |
| commit failure retains dirty | no false success | E | N-A · unit |
| Save success + appearance publish | persisted + app re-themes | EP | Y · unit+connected |
| Discard / Keep editing | revert or stay | E | N-A · connected |
| slider snapping / limits | snapped value | E | N-A · unit |
| excluded-app selection | list edit | E | N-A · connected |
| destructive storage/history actions | confirm + in-flight | E | N-A · connected |
| diagnostics/log/about/permissions nav | open target | E | N-A · connected |
| normal/focused/disabled/dirty/saved states | control states | EP | Y(dirty+error) · connected |

### C7. Onboarding / permissions / background capture  (S10)
| Interaction / state | Result | Tag | golden · test |
|---|---|---|---|
| first-launch routing + completion gate (notifications only) | Continue/Skip | E | N-A · connected |
| granted / denied / permanently-denied / not-applicable | card status (icon+text) | EP | Y(denied+granted) · connected |
| notification/camera/overlay/battery/OEM launches | correct Intent | E | N-A · intent unit |
| resume-time state refresh | re-eval on onResume | E | N-A · connected |
| OEM intent unavailable fallback | "not needed" card | E | N-A · unit |
| ADB command copy | copied + toast | E | N-A · connected |
| crash dialog export / dismiss | export ZIP or close | E | N-A · connected |
| bg-capture setup step complete/retry | step progresses | E | Y · connected |

### C8. Feedback / diagnostics / system surfaces  (S11/S12)
| Interaction / state | Result | Tag | golden · test |
|---|---|---|---|
| toast replacement / timeout / action | single-slot, action dismiss | E | Y(per-kind) · unit |
| banner retry / dismiss rules | actionable-only | E | Y · connected |
| sync badge 4 states + detail sheet | dot+label, sheet | E | Y · connected |
| log load / filter / clear / copy / export success+failure | log states + toast | E | Y(loading/empty/no-match) · connected |
| About links / licenses / no-handler | open or graceful fail | EP (build-id/licenses new content) | Y · connected |
| foreground/copy/pair/native-unavailable/restart notifications | localized/branded | EP | N-A · channel+action tests |
| notification actions Pause/Resume/Open/SAS | state transitions | E | N-A · action tests |
| share receiver ACTION_SEND / SEND_MULTIPLE, URI lifetime, failure logging | UI-less capture | I | N-A · behavioural (no UI) |
| invisible overlay permission/focus/suppress/restore lifecycle | UI-less | I | N-A · idempotency unit |

---

## D. Explicitly removed / dead

- `previewChipColor` (PreviewChrome.kt) — **Remove**: replaced by the single shared content-type
  color source (kills List↔Preview divergence).
- Legacy `NavIcons.kt` bespoke vectors + `contentIconFor` Material mappings — **Remove** after Lucide
  migration.

No `New` state is introduced without a plumbing task; the only new presentation-state is History
**error/degraded** (S5). New components (shared Banner, transport/Verified/This-device pills, SECRET
lock tile, About build-id/licenses) are listed in `component-inventory.md` with owning slices.

---

## E. Settings control-level matrix  (S9, with S3 appearance)

Persistence mode: **Draft** (Save-owned batch) · **Immediate** (persist on change) · **Ephemeral**
(session-only). Every control preserves its current mode unless a row says otherwise.

### E1. General
| Control | Mode | Side effect | Failure | Slice |
|---|---|---|---|---|
| Private mode | Draft | stops capture/History recording | — | S9 |
| Sync enable (global) | Draft | gates all transports | — | S9 |
| Public-IP discovery consent | Draft | STUN public_ip becomes visible | STUN fail → blank | S9 |
| Paste-as-plain-text | Draft | strips rich text on paste | — | S9 |
| Nav rows (Permissions/Devices/Background/Logs/About) | N-A | launch target | — | S9/S10/S11 |
| Log export | Action (A) | FileProvider share ZIP | toast on IO fail | S11 |
| Logcat capture enable | Draft | permission-dependent status | not-granted status | S9 |
| ADB command copy | Action (A) | clipboard + toast | — | S9 |

### E2. Display
| Control | Mode | Side effect | Slice |
|---|---|---|---|
| Sensitive warning toggle | Draft | capture-skip toast policy | S9 |
| Reveal guard toggle | Draft | reveal confirmation | S9 |
| Mask-sensitive toggle | Draft | list/preview masking | S9 |
| **allowScreenshots** | **Immediate** | **writes now + toggles `FLAG_SECURE` on current window** | S9 |
| Translucency | Draft (live preview) | frosted vs opaque | S3 |
| Image max height / preview delay / preview lines | Draft | list rendering | S9 |
| Theme / System / Accent (new) | Draft (live preview) | app-scoped publish on Save | S3 |

### E3. Sync
| Control | Mode | Side effect | Slice |
|---|---|---|---|
| P2P enable | Draft | LAN dialer gate | S9 |
| LAN visibility | Draft | mDNS register/unregister | S9 |
| Auto-apply synced clip | Draft | applies remote clips | S9 |
| Wi-Fi-only sync | Draft | network gate | S9 |
| **Relay enable / Supabase enable** | **Immediate** | **written in SyncTab now; additive transports (fan-out reads live)** | S9 |
| Supabase URL/key/passphrase/email/password | Draft | credentials; masked; validation | S9 |
| Relay URL | Draft | validation | S9 |
| Sign-in / sign-out / account state | Action (A) | GoTrue auth | S9 |
| Test connection | Action (A) | probes enabled+configured transports; unavailable/in-flight/OK/fail toast | S9 |
| Sync-error banner (401 vs generic) | read-only | distinct copy | S11 |
| Cloud-account mismatch | read-only | stays inert (gldr) | S7 |

### E4. Storage
| Control | Mode | Side effect | Slice |
|---|---|---|---|
| Stepped sliders (text/image/file size, quota, TTL, max-items) | Draft | step/sentinel via `SettingsUtils` | S9 |
| Lowering max-items | Draft + confirm | **destructive prune** confirmation on Save | S9 |
| Excluded-app add/remove | Draft | dedup/invalid input handling | S9 |
| Export history (SAF) | Action (A) | writes file | S9 |
| Include-sensitive export toggle | **Ephemeral** | default off; plaintext warning | S9 |
| Import history (SAF) | Action (A) | success/partial/failure/dedup | S9 |
| Clear history / Reset database | Action (A) + confirm | destructive | S9 |
| Vacuum/compact | Action (A) | unavailable/in-flight/OK/fail | S9 |

### E5. Notifications
| Control | Mode | Side effect | Slice |
|---|---|---|---|
| Notify-on-copy | Draft | posts copy-event notification | S9 |
| Sound-on-copy | Draft | click sound on capture; **independent of notify-on-copy** — both toggles are separate in all capture paths (`if (notifyOnCopy)…` / `if (soundOnCopy)…`), preserve independence | S9 |
| (relationship) POST_NOTIFICATIONS denial + FGS visibility | read-only | status note | S10/S12 |

## F. Resolved ambiguities (FOURTH_PASS §5)

- **Device card expand/collapse — RESOLVED: no collapse.** Cards are always-expanded, natural-height,
  showing the full §9.7 field grid; remove "if retained". (S7)
- **History error/degraded trigger.** Source = a `ClipboardRepository` read/IPC failure (e.g. DB open
  or daemon error) surfaced by `HistoryScreenState` as a persistent `Error` presentation-state; today
  such errors only toast. New presentation-state only — no repository/IPC behaviour change. (S5)
- **Preview zoom — exists, Preserve.** Image pinch-zoom/pan exists (`PreviewImageContent` +
  `Modifier.previewPeekGesture`) and is preserved. "Large text/code" = scroll (distinct behaviour); no
  new text zoom. (S6)
- **About links.** Enumerated destinations: repository URL (GitHub), license/attribution notices,
  version+build identifier. No-handler → graceful (no crash), owned by S11.
- **Notification variants** (S12), each with channel · importance · visibility · actions · PendingIntent · debounce/auto-cancel · l10n:
  - Foreground-service — `copypaste_service` · LOW · `VISIBILITY_SECRET` · ongoing · Open→`MainActivity` (getActivity), **Pause/Resume→`CaptureControlReceiver` (getBroadcast)** · localize.
  - Copy-event — `copypaste_copy_event` · MIN · silent · auto-cancel · debounced 500ms · localize.
  - Pair-request — `copypaste_pair_request` · HIGH · badged · Open (→DevicesActivity `EXTRA_AUTO_OPEN_SAS`) · localize.
  - Native/encryption-unavailable (`NotificationHelper`) — **4th channel `copypaste_sync`** · one-off · specify importance/category/visibility · localize (move off hardcoded strings).
  - Restart (`ServiceRestartWorker.getForegroundInfo()`) — notification ID 1010 on
    `ClipboardService.CHANNEL_ID` · PRIORITY_LOW · ongoing=true · launcher foreground icon ·
    `FOREGROUND_SERVICE_TYPE_SPECIAL_USE` where supported · preserve incl. the API 26–30
    expedited-worker path · localize the active title.
- **Share receiver log privacy.** Failure logging MUST NOT include the shared URI, file name, or
  content — only a non-identifying failure category. (S12, security)
- **System-owned intents**, each with action + fallback: overlay `ACTION_MANAGE_OVERLAY_PERMISSION`;
  battery `ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS`; notifications `ACTION_APP_NOTIFICATION_SETTINGS`
  → fallback `ACTION_APPLICATION_DETAILS_SETTINGS`; OEM via `OemAutoStartHelper` (resolvable/unresolvable);
  sharesheet `ACTION_SEND` chooser. (S10/S12)

## G. Additional behaviour owners (FOURTH_PASS §6)

| Owner | User-observable transition / boundary | Disposition · Slice |
|---|---|---|
| `HistoryFilePicker.kt` | SAF launcher/result/cancel/error | Preserve · S5 |
| `HistoryImageCache.kt` / thumbnail utils | thumbnail load/fallback/cache invalidation in rows | Preserve; define fallback + golden · S5 |
| `HistoryScreenState.kt` | search/filter/load-more/preview/selection orchestration | Preserve · S5 |
| `SettingsActivity.persistAll` | commit result, commit-failure, post-save nav | Preserve; failure→dirty · S9 |
| `SettingsUtils.kt` | slider snapping/formatting/sentinels | Preserve · S9 |
| `ServiceNotifications.kt` / `NotificationHelper.kt` | notification builders (separate owners) | Preserve; localize/brand · S12 |
| `NotificationPermissionHelper` × `MainActivity` | first-launch service start gating | Preserve · S12 |
| `PairActivity` | deep-link / `onNewIntent` routing | Preserve · S8 |
| `DevicesController` | dialog-dismiss / in-flight guards | Preserve · S7 |
| `LogViewerActivity` | log clear/filter/export state orchestration | Restyle + Preserve · S11 |

Rule applied: a file gets a row when it owns a user-observable state transition, security boundary,
external intent, or failure mapping — not for pure formatting helpers.

## H. Evidence → concrete test/command mapping (FOURTH_PASS §7)

Each evidence label in §C maps to a real target. Runner ∈ {JVM, Robolectric, Paparazzi, Connected,
Manual}. CI: `unit` = JVM/Robolectric job; `golden` = Paparazzi job; `instrumented` = emulator job
(API 34, Pixel profile); `manual` = milestone checklist.

| Evidence label | Test class to add | Runner · command | CI |
|---|---|---|---|
| token / contrast | `CpColorContrastTest` | JVM · `:app:testDebugUnitTest` | unit |
| migration | `ThemeMigrationTest` | Robolectric · `:app:testDebugUnitTest` | unit |
| persistence / commit-failure | `SettingsPersistenceTest` | Robolectric · `:app:testDebugUnitTest` | unit |
| revoke ordering | `DevicesRevokeOrderingTest` | JVM · `:app:testDebugUnitTest` | unit |
| pairing states | `PairControllerTest` (exists) | JVM · `:app:testDebugUnitTest` | unit |
| unit (actions/mapping) | `HistoryItemActionsTest`, `ErrorMessagesTest`, `OnboardingPermissionsTest` | JVM · `:app:testDebugUnitTest` | unit |
| golden (per screen) | `<Screen>PaparazziTest` | Paparazzi · `verifyPaparazziDebug` | golden |
| security-semantics (masking) | `HistoryMaskingSemanticsTest`, `PreviewMaskingSemanticsTest` | Connected · `connectedDebugAndroidTest` | instrumented |
| connected (roles/focus/48dp) | `<Screen>A11ySemanticsTest` | Connected · `connectedDebugAndroidTest` | instrumented |
| intents | `PermissionIntentTest` | Robolectric · `:app:testDebugUnitTest` | unit |
| channel + action tests | `NotificationChannelTest`, `NotificationActionTest` | Robolectric/Connected · resp. command | unit/instrumented |
| notify/sound independence | `CopyNotificationSoundTest` (4-combo truth table) | JVM/Robolectric · `:app:testDebugUnitTest` | unit |
| l10n completeness | `LocalizationCompletenessTest` + hardcoded-text lint/AST | JVM · `:app:testDebugUnitTest` + `lint` | unit |
| window flag (FLAG_SECURE) | `SecureWindowTest` | Connected · `connectedDebugAndroidTest` | instrumented |
| manual (TalkBack/gesture) | TalkBack checklist | Manual · milestone | manual |

The connected/instrumented emulator config (API 34, Pixel profile) and its CI availability are
finalized in S0.6; no §C row claims evidence without a row here. CI availability: `instrumented`
evidence is **CI advisory-only until CopyPaste-k1l0 is resolved** (see design.md Resolved decisions
and `android-localization-accessibility/spec.md`) — treated as a mandatory local run for
security-relevant slices (S4, S5/S6, S8, S9/S10, S12, S15) until then.

---

## I. Settings preservation inventory (every existing user setting keeps working)

Mode kinds: **D** DraftPreference (Save batch) · **I** ImmediatePreference · **E** EphemeralUiState ·
**A** Action · **R** ReadOnlyStatus (only D/I participate in preference-preservation tests). Status
**Preserve** = same key/default/effect · **New** = additive · **Legacy** = key retained, not an
effective control · **Repair** = persisted today but has NO runtime consumer (must wire a consumer or
reclassify). Each functional row's **consumer**, **activation timing**, **dependencies**, and **evidence** are
normative in the `android-settings` behaviour requirements (Functional-consumer · Activation-timing ·
Dependency/disabled) and verified per row by S9.4's 3-layer tests (persistence · consumer · UI). Every
tab below carries the `consumer · activation` columns inline; dependencies + failure feedback + named
test evidence live in those requirements + S9.4. Cross-platform parity per shared setting is in
`cross-platform-parity.md`. No existing key is renamed or dropped.

### General tab
| Setting | key | default | mode | consumer · activation | status |
|---|---|---|---|---|---|
| Private mode | `private_mode` | false | D | ClipboardCapturePipeline (skip persist/sync) · next capture | Preserve |
| Sync enabled (global) | `sync_enabled` | true | D | FgsSyncLoop/poller gate · next sync | Preserve |
| Collect public IP | `collect_public_ip` (ConfigKnobs) | false | D | STUN/public-ip resolver · next sync | Preserve |
| Paste as plain text | `paste_as_plain_text` (ConfigKnobs) | false | D | paste path · next paste | Preserve |
| Logcat capture enable | `logcat_capture_enabled` | false | D | `LogcatCaptureService.syncState` · immediate (start/stop) | Preserve |
| Permissions/Devices/Background/Logs/About nav · ADB copy | — | — | A | launch/clipboard · on tap | Preserve |

### Display tab
| Setting | key | default | mode | consumer · activation | status |
|---|---|---|---|---|---|
| Sensitive-skip warning | `notify_on_sensitive_skip` (+legacy migrate) | true | D | suppression-branch toast · next capture | **Repair** (no consumer today) |
| Reveal guard | `show_sensitive_warnings_reveal_guard` | true | D | reveal entry points (list/preview/search/copy) · immediate | Preserve |
| Mask sensitive | `mask_sensitive_content` | true | D | HistoryRow/Preview masking · immediate recompose | Preserve |
| Allow screenshots | `allow_screenshots` | false | **I** (+FLAG_SECURE) | SecureWindowChrome / app-scoped · immediate (current) + on resume | Preserve |
| Translucency | `translucency` | true | D | chrome surfaces · immediate recompose | Preserve |
| Image max height | `image_max_height` (1–200) | 40 | D | HistoryRow image bounds · immediate recompose | Preserve |
| Preview delay | `preview_delay_ms` (200–100000) | 1500 | D | preview auto-collapse timer · next interaction | Preserve |
| Preview lines | `preview_lines` (1–6) | 1 | D | HistoryRow `maxLines` · immediate recompose | Preserve |
| Theme | `theme_mode` | dark | D | CopyPasteTheme (committed) · Save→publish | **New** |
| Accent | `accent` | indigo | D | CopyPasteTheme (committed) · Save→publish | **New** |

### Sync tab (worked example with consumer · activation columns)
| Setting | key | default | mode | consumer · activation | status |
|---|---|---|---|---|---|
| Sync backend (legacy hint) | `sync_backend` | SUPABASE | D | none (legacy) · n/a | **Legacy** — retain key; not an effective control |
| Wi-Fi-only sync | `sync_on_wifi_only` | false | D | FgsSyncLoop/poller network gate · next sync | Preserve |
| P2P sync enable | `p2p_sync_enabled` | true | D | FgsSyncLoop P2P dialer · service reconfigure | Preserve |
| LAN visibility | `lan_visibility` | true | D | NSD register/unregister · immediate (listener) | Preserve |
| Auto-apply synced clip | `auto_apply_synced_clip` | true | D | inbound-transport seam · next inbound | **Repair** (no consumer today) |
| Relay enable | `relay_enabled` | true | I | fan-out gate (live) · immediate | Preserve |
| Supabase enable | `supabase_enabled` | true | I | fan-out gate (live) · immediate | Preserve |
| Supabase URL | `supabase_url` | "" | D | SupabaseClient · next sync | Preserve |
| Supabase anon key | `supabase_anon_key` | "" | D | SupabaseClient · next sync | Preserve |
| Cloud passphrase (secret) | keystore-wrapped | "" | D | cloud-key derivation · next sync | Preserve |
| Supabase email (secret) | keystore-wrapped | "" | D | GoTrue sign-in · on sign-in | Preserve |
| Supabase password (secret) | keystore-wrapped | "" | D | GoTrue sign-in · on sign-in | Preserve |
| Relay URL | `relay_url` | "" | D | RelayClient · next sync | Preserve |
| Test connection | — | — | A | RelayClient/SupabaseClient.health · on tap | Preserve |

### Storage tab
(defaults = FFI `defaultConfig()` values, pinned exactly in S1)
| Setting | key | default | mode | consumer · activation | status |
|---|---|---|---|---|---|
| Max text size | `max_text_size_bytes` | defaultConfig | D | text-ingest path · next capture | Preserve (ingest-enforced) |
| Max image size | `max_image_size_bytes` | defaultConfig | D | image-ingest path · next capture | Preserve (ingest-enforced) |
| Max file size | `max_file_size_bytes` | defaultConfig | D | file-ingest (clipboard/share/import) · next acquire | **Repair** (no consumer today) |
| Storage quota | `storage_quota_bytes` | defaultConfig | D | ClipboardRepository prune · on Save / next write | Preserve |
| Sensitive TTL | `sensitive_ttl_secs` | defaultConfig | D | TTL expiry worker/event · scheduled | Preserve |
| Max history items | `max_history_items` | 1000 | D (prune confirm) | `applyHistoryCap` · on Save | Preserve |
| Excluded apps | ConfigKnobs list | [] | D | capture app-filter · next capture | Preserve |
| Export / Import / Clear / Reset / Vacuum | — | — | A | repository / SAF · on tap | Preserve |
| Export "include sensitive" | — | off | **E** | export builder · this export only | Preserve |

### Notifications tab
| Setting | key | default | mode | consumer · activation | status |
|---|---|---|---|---|---|
| Notify on copy | `notify_on_copy` | true | D | `postCopyNotification` · next capture | Preserve |
| Sound on copy | `sound_on_copy` | true | D | `playCopySound` (independent of notify) · next capture | Preserve |

### Non-tab user-controlled state
| Setting | key | default | mode | consumer · activation | status |
|---|---|---|---|---|---|
| Capture pause/resume | `capture_enabled` | true | I (notification action) | ClipboardService capture gate · immediate | Preserve |
| History sort by device | `sort_by_device` | false | I (overflow) | HistoryScreenState sort · immediate recompose | Preserve |
| Recent searches | `recent_searches` | [] | I (history search) | history search bar · immediate | Preserve |

### Internal / non-UI (untouched — not user settings)
`device_id`, clipboard encryption key, `relay_token`/`relay_token_url`/`relay_registration_key`,
`last_relay_subscribe_*`, `supabase_user_id`, cloud direct key, `last_sync_error*`,
`logcat_capture_working`, P2P identity, paired-peer roster, sync cursors, Lamport clock,
`theme_migrated_2axis` — preserved as-is, no UI.

**Guarantee:** the redesigned `Settings` façade keeps all keys/defaults/modes above; S9 verifies each
control still reads/writes its key, and an upgrade test loads pre-redesign prefs and asserts every
value survives.
