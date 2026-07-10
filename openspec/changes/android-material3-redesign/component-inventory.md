# Android component inventory (view-level)

Complete enumeration of every user-visible composable and Activity in
`android/app/src/main/java/com/copypaste/android/`. Reproducible counts: **13 Activities** and **117 `@Composable` annotation sites** = **116 unique
names** (`rememberReducedMotion` is defined twice — `DevicesAnimations.kt` and `ui/SyncStatusBadge.kt`).
Extraction commands:
`rg -c '^\s*@Composable' <src> | awk -F: '{s+=$2} END{print s}'` (sites) and
`rg -oU --pcre2 '(?s)@Composable.*?\bfun\s+(\w+)' -r '$1' <src> | sort -u | wc -l` minus the one
duplicate (names). The 13 Activities are Main, Onboarding,
History, Pair, PortraitCapture, Settings, ShareReceiver, LogViewer, PermissionsSettings, About,
BackgroundCaptureSetup, ClipboardFloating, Devices. Non-composable behaviour owners
(services/receivers/worker/helpers) are inventoried in `behavior-and-state-coverage.md`. Each row
maps to its owning capability, slice, and redesign **disposition**:

- **Restyle** — migrate to `CpColors`/`AccentColor`/`CpShapes`/`CpTypography`/Lucide; no behaviour change.
- **Refactor** — extract/reshape structure (e.g. lift shell out of `MainActivity`) before restyle.
- **Preserve** — behaviour/flags/UI-less; do NOT decorate (goldens N/A).
- **New** — component or state to be added by this change.
- **Helper** — non-view remember/Modifier/color-resolver; retheme via token source, no golden.

Behaviour-owner inventory and the complete per-state evidence matrix live in
`behavior-and-state-coverage.md`. Golden lifecycle: S0 spike → S2 infra/policy → each owning screen
slice adds its baselines → S14 audits coverage (not S14-only).

---

## S1/S2 — Design system, shared components, icons  (`android-design-system`, `android-iconography`)

| Component | File | Kind | Disposition |
|---|---|---|---|
| SecureWindowChrome | Theme.kt | theme wrapper | Refactor (compose `CopyPasteTheme`; keep both SideEffects verbatim) |
| CopyPasteTopBar | Components.kt | shared | Restyle |
| CopyPasteCard | Components.kt | shared | Restyle (re-activate `accent`/`translucent`) |
| GlassAlertDialog | Components.kt | shared | Restyle (§9.9) |
| IdeSwitch | Components.kt | shared | Restyle (§9.2 toggle) |
| SectionLabel | Components.kt | shared | Restyle (micro type) |
| CopyPasteButton | Components.kt | shared | Restyle (§9.1 variants) |
| CopyPasteIconButton | Components.kt | shared | Restyle (48dp target, Lucide) |
| SharedSettingsRow | Components.kt | shared | Restyle |
| SharedSettingsNavRow | Components.kt | shared | Restyle |
| EmptyStateCard | Components.kt | shared | Restyle (§9.10) |
| ideTextFieldColors | Components.kt | Helper | Restyle (token colors) |
| SteppedSliderRow | SliderComponents.kt | shared | Restyle |
| ContinuousSliderRow | SliderComponents.kt | shared | Restyle |
| SettingsCard | SettingsComponents.kt | shared | Restyle |
| SettingsCardDivider | SettingsComponents.kt | shared | Restyle |
| IdeSegmentedControl | SettingsComponents.kt | shared | Restyle (§9.2; reused by Theme picker) |
| SettingsTextField | SettingsComponents.kt | shared | Restyle (§9.3 input) |
| SettingsNavRow | SettingsComponents.kt | shared | Restyle |
| DiagnosticsNavRow | SettingsComponents.kt | shared | Restyle |
| SettingsRow | SettingsComponents.kt | shared | Restyle |
| AdbCaptureStatusLine | SettingsComponents.kt | shared | Restyle |
| AdbCaptureCommandRows | SettingsComponents.kt | shared | Restyle |
| AdbCmdRow | SettingsComponents.kt | shared | Restyle |
| Content-type tile glyphs (Lucide) | NavIcons.kt → Lucide | New/Refactor | New (real Lucide provider; retire `NavIcons`) |
| Banner (shared) | `ui/theme/Banner.kt` (new) | New | New (§9.8; replaces ad-hoc Cards in SyncTab) |
| Transport/Verified/This-device pill | `ui/theme/Components.kt` (new chips) | New | New (§9.4; today plain `Text`) |
| SECRET lock tile + `cSecret` | `HistoryChips.kt` | New | New |

## S3 — Appearance  (`android-appearance`)

| Component | File | Kind | Disposition |
|---|---|---|---|
| DisplayTab | DisplayTab.kt | screen tab | Restyle + New (Theme segmented, Accent swatch row, Mask toggle; translucency exists) |
| Theme segmented control | DisplayTab.kt | New | New (reuses `IdeSegmentedControl`) |
| Accent swatch row | DisplayTab.kt | New | New (6 swatches, selected ring) |

## S4 — Shell + navigation  (`android-navigation-chrome`)

| Component | File | Kind | Disposition |
|---|---|---|---|
| MainActivity | MainActivity.kt | Activity | Restyle |
| MainShell | MainActivity.kt | shell | Refactor (extract to reusable/previewable; adaptive width) |
| FloatingTabBar | MainActivity.kt | nav | Refactor + Restyle (frosted pill §9.12, Lucide icons, accent pill, backdrop blur) |

## S5 — History  (`android-history`)

| Component | File | Kind | Disposition |
|---|---|---|---|
| HistoryActivity | HistoryActivity.kt | Activity | Restyle |
| HistoryScreen | HistoryScreen.kt | screen | Restyle |
| HistoryList | HistoryList.kt | list | Restyle |
| HistoryRow | HistoryRow.kt | row | Restyle (tile/glyph/mono/meta/pin/actions; preserve masking) |
| SourceAppBadge | HistoryRow.kt | row part | Restyle |
| ScaleIconButton | HistoryRow.kt | control | Restyle |
| ContentTypeChip | HistoryChips.kt | tile | Restyle (shared 10-color source) |
| ContentIconTile | HistoryChips.kt | tile | Restyle (glyph not label) |
| ColorSwatchOrTile | HistoryChips.kt | tile | Restyle (COLOR swatch) |
| TooLargeBadge | HistoryChips.kt | badge | Restyle |
| DeviceFilterRow | HistoryDeviceFilter.kt | filter | Restyle (pills) |
| DeviceChip | HistoryDeviceFilter.kt | chip | Restyle |
| OriginDeviceBadge | HistoryDeviceFilter.kt | badge | Restyle |
| LoadingBox | HistoryEmptyStates.kt | state | Restyle |
| EmptyHistoryState | HistoryEmptyStates.kt | state | Restyle (normal + private) |
| EmptySearchState | HistoryEmptyStates.kt | state | Restyle |
| HistoryNormalTopBar | HistoryNormalTopBar.kt | top bar | Restyle (Lucide icons; today `Text`-as-icon) |
| SelectionTopBar | HistorySelectionBar.kt | top bar | Restyle |
| ConfirmationDialog | HistorySelectionBar.kt | dialog | Restyle (§9.9) |
| rememberHistoryScreenState | HistoryScreenState.kt | Helper | Preserve behaviour; update deps/callers (regression via `HistoryScreenState` tests) |
| rememberHistoryItemPipelines | HistoryScreenState.kt | Helper | Preserve (search/filter/load-more/selection orchestration) |
| HistoryScreenEffects | HistoryScreenState.kt | Helper | Preserve (side-effect wiring) |
| rememberHistoryFilePickerLauncher | HistoryFilePicker.kt | Helper | Preserve (SAF launcher/result/cancel/error) |
| Error/degraded list state | — | New | New (needs presentation-state plumbing — see tasks) |

## S6 — Full-screen preview  (`android-preview`)

| Component | File | Kind | Disposition |
|---|---|---|---|
| PreviewOverlay | PreviewOverlay.kt | overlay | Restyle + New (wire `revealed` state) |
| PreviewHeader | PreviewChrome.kt | chrome | Restyle |
| PreviewContentTypeChip | PreviewChrome.kt | chip | Restyle (shared color source) |
| previewChipColor | PreviewChrome.kt | Helper | Remove (use shared source — kills List/Preview divergence) |
| PreviewTextContent | PreviewContent.kt | content | Restyle + **New (Reveal wiring)** + fix masking `clearAndSetSemantics` |
| PreviewImageContent | PreviewContent.kt | content | Restyle + **New (Reveal wiring)** + fix masking |
| PreviewFileContent | PreviewContent.kt | content | Restyle |
| PreviewActionRow | PreviewActionRow.kt | actions | Restyle (Lucide) + New (Reveal action) |
| Modifier.previewPeekGesture | PreviewGesture.kt | Helper | Preserve (gesture logic) |

## S7 — Devices  (`android-devices`)

| Component | File | Kind | Disposition |
|---|---|---|---|
| DevicesActivity | DevicesActivity.kt | Activity | Restyle |
| DevicesScreen | DevicesScreen.kt | screen | Restyle |
| PeerRow | PeerRow.kt | card | Restyle (§9.7 grid; fingerprint tap-to-copy) |
| NoPeerCard | PeerRow.kt | empty | Restyle |
| OwnDeviceRow | PeerRow.kt | card | Restyle |
| DiscoveredPeerRow | PeerRow.kt | card | Restyle |
| RowDivider | DevicesUtils.kt | part | Restyle |
| MetaRow | DevicesUtils.kt | part | Restyle |
| PulseDot | DevicesAnimations.kt | presence | Restyle (reduced-motion) |
| TransportChipLabel | DevicesAnimations.kt | chip | Restyle (pill) |
| DevicesDialogs | DevicesDialogs.kt | dialog host | Restyle |
| UnpairConfirmDialog | DevicesDialogs.kt | dialog | Restyle |
| RevokeConfirmDialog | DevicesDialogs.kt | dialog | Restyle |
| RevokeRotateDialog | DevicesDialogs.kt | dialog | Restyle |
| RevokeErrorDialog | DevicesDialogs.kt | dialog | Restyle |
| RevokeAllConfirmDialog | DevicesDialogs.kt | dialog | Restyle |
| SasPairingModalHost | DevicesDialogs.kt | host | Restyle |
| ScanErrorDialog | DevicesDialogs.kt | dialog | Restyle |
| rememberDevicesController | DevicesController.kt | Helper | Preserve (state/IPC) |
| rememberReducedMotion | DevicesAnimations.kt | Helper | Preserve |

## S8 — Pairing  (`android-pairing`)

| Component | File | Kind | Disposition |
|---|---|---|---|
| PairActivity | PairActivity.kt | Activity | Restyle (preserve unconditional FLAG_SECURE) |
| PairScreen | PairScreen.kt | screen | Restyle |
| PairQrCard | PairQrCard.kt | card | Restyle |
| OwnQrSection | QrHelper.kt | section | Restyle (blur-at-rest) |
| ScannedPeerReviewCard | PairedPeerList.kt | card | Restyle (reclassify → pairing) |
| PairedDeviceSummaryCard | PairedPeerList.kt | card | Restyle |
| SasPeerMetadataCard | SasPairingDialog.kt | card | Restyle (supplemental peer metadata; 6-digit SAS is primary) |
| SasPairingDialog | SasPairingDialog.kt | dialog | Restyle |
| PairedSuccessPopup | PairSuccessPopup.kt | popup | Restyle |
| PopupMetaRow | PairSuccessPopup.kt | part | Restyle |
| PortraitCaptureActivity | PortraitCaptureActivity.kt | Activity (ZXing) | Preserve (theme/orientation/decoder only) |

## S9 — Settings  (`android-settings`)

| Component | File | Kind | Disposition |
|---|---|---|---|
| SettingsActivity | SettingsActivity.kt | Activity | Restyle |
| SettingsScreen | SettingsActivity.kt | screen | Restyle (draft model, embedded vs standalone) |
| GeneralTab | GeneralTab.kt | tab | Restyle |
| SyncTab | SyncTab.kt | tab | Restyle |
| SyncDiagnosticsCard | SyncTab.kt | card | Restyle |
| DiagnosticsRow | SyncTab.kt | row | Restyle |
| DiagnosticsDivider | SyncTab.kt | part | Restyle |
| StorageTab | StorageTab.kt | tab | Restyle |
| ExcludedAppsRow | StorageTab.kt | row | Restyle |
| NotificationsTab | NotificationsTab.kt | tab | Restyle |

## S10 — Onboarding + permissions  (`android-onboarding-permissions`)

| Component | File | Kind | Disposition |
|---|---|---|---|
| OnboardingActivity | OnboardingActivity.kt | Activity | Restyle |
| OnboardingScreen | OnboardingScreen.kt | screen | Restyle |
| PermissionCard | OnboardingCards.kt | card | Restyle (icon+text status) |
| AdbBackgroundCaptureCard | OnboardingCards.kt | card | Restyle |
| StatusPill | OnboardingCards.kt | pill | Restyle |
| AdbCommandRow | OnboardingCards.kt | row | Restyle |
| CrashDetectedDialog | OnboardingDialogs.kt | dialog | Restyle |
| PermissionsSettingsActivity | PermissionsSettingsActivity.kt | Activity | Restyle |
| PermissionsScreen | PermissionsSettingsActivity.kt | screen | Restyle |
| BgCaptureStatusCard | PermissionsSettingsActivity.kt | card | Restyle |
| AdbCommandBlock | PermissionsSettingsActivity.kt | block | Restyle |
| PermissionStatusCard | PermissionsSettingsActivity.kt | card | Restyle |
| BackgroundCaptureSetupActivity | BackgroundCaptureSetupActivity.kt | Activity | Restyle |
| BackgroundCaptureSetupScreen | BackgroundCaptureSetupActivity.kt | screen | Restyle |
| BgCaptureCard | BackgroundCaptureSetupActivity.kt | card | Restyle |

## S11 — Feedback, About, Logs  (`android-feedback-states`)

| Component | File | Kind | Disposition |
|---|---|---|---|
| GlassToastHost | GlassToast.kt | toast | Restyle |
| GlassToastContent | GlassToast.kt | toast | Restyle |
| SyncStatusBadge | SyncStatusBadge.kt | badge | Restyle |
| SyncStatusSheet | SyncStatusBadge.kt | sheet | Restyle |
| SheetContent | SyncStatusBadge.kt | sheet part | Restyle |
| SheetRow | SyncStatusBadge.kt | row | Restyle |
| AboutActivity | AboutActivity.kt | Activity | Restyle |
| AboutScreen | AboutActivity.kt | screen | Restyle + New (build id, licenses, gradient mark) |
| LogViewerActivity | LogViewerActivity.kt | Activity | Restyle |
| LogViewerScreen | LogViewerActivity.kt | screen | Restyle |
| LogLine | LogViewerActivity.kt | row | Restyle (level via icon/text + color) |
| rememberReducedMotion | SyncStatusBadge.kt | Helper | Preserve |

### 11.1 Feedback-producer inventory (task #15)

One row per producer call site. Mechanism: `Toast`=GlassToast (`toastState.show`), `Banner`=`CpBanner`,
`Dialog`=`GlassAlertDialog`, `Notif`=system notification (`ServiceNotifications`), `Toast.makeText`=legacy
Android toast fallback. Kind: success/danger/info/accent/warn/error, or n/a for dialogs (neutral confirm).

| Owning screen/service | File:line | Mechanism | Kind | Message source | Action/retry | Migration slice |
|---|---|---|---|---|---|---|
| BackgroundCaptureSetupScreen (OEM autostart hint) | BackgroundCaptureSetupActivity.kt:254 | Toast | info | caller-passed `oemHint` string (resource, OEM-specific copy) | no | S10 |
| HistoryScreen (load error) | HistoryScreen.kt:151 | Toast | danger | `ErrorMessages.friendlyOperationError(msg)` (sanitized) | no | S6 |
| HistoryScreen (clearAll error) | HistoryScreen.kt:177 | Toast | danger | `ErrorMessages.friendlyOperationError(msg)` (sanitized) | no | S6 |
| HistoryScreen (file picker captured/failed) | HistoryScreen.kt:127-128 | Toast | success/danger | `R.string.snackbar_file_captured` / `R.string.error_file_pick_failed` | no | S6 |
| HistoryScreen (single delete) | HistoryScreen.kt:213-218 | Toast | info | `R.string.snackbar_item_deleted` | yes: `R.string.snackbar_undo` (5s window) | S6 |
| HistoryScreen (bulk copy) | HistoryScreen.kt:293,295-298 | Toast | info/success | `R.string.snackbar_bulk_copied_no_text` / `R.string.snackbar_bulk_copied` (formatted) | no | S6 |
| HistoryScreen (sensitive-item tap) | HistoryScreen.kt:414 | Toast | info | `R.string.sensitive_tap_hint` | no | S6 |
| HistoryScreen (save file, row) | HistoryScreen.kt:424,426 | Toast | success/danger | `R.string.file_saved_ok` / `R.string.file_save_failed` | no | S6 |
| HistoryScreen (open file, row) | HistoryScreen.kt:437,440 | Toast | danger | `R.string.file_open_no_app` / `resolution.nameOrError` (may be resource or sanitized error) | no | S6 |
| HistoryScreen (media copy-as-text) | HistoryScreen.kt:459 | Toast | info | caller-passed `msg` (resource) | no | S6 |
| HistoryScreen (save/open file, preview overlay) | HistoryScreen.kt:510,512,524,527 | Toast | success/danger | same resources as row variants above | no | S6 |
| PermissionsSettingsActivity (OEM autostart hint) | PermissionsSettingsActivity.kt:239 | Toast | info | caller-passed `oemHint` (resource) | no | S10 |
| PermissionsSettingsActivity (bg-capture ADB step) | PermissionsSettingsActivity.kt:310 | Toast | success | caller-passed `msg` via `onToastRequest` (resource) | no | S10 |
| LogViewerActivity (export error) | LogViewerActivity.kt:231 | Toast | danger | `LogExportHelper.shareLogsZip` `onError` `msg` (mix: resource + one hardcoded empty-files string) | no | S11 |
| LogViewerActivity (export success) | LogViewerActivity.kt:235 | Toast | success | `exportedMsg` (resource) | no | S11 |
| LogViewerActivity (clear-logs dialog) | LogViewerActivity.kt:159 | Dialog | n/a (destructive confirm) | **HARDCODED**: "Clear Logs" / "Delete all log files…" / "Clear" / "Cancel" | yes: Clear (destructive) / Cancel | S11 — pre-existing, needs resource extraction |
| SettingsActivity (settings save failed) | SettingsActivity.kt:417 | Toast | danger | `R.string.toast_settings_save_failed` | no | S9 |
| SettingsActivity (generic toast passthrough) | SettingsActivity.kt:559 | Toast | (caller-chosen) | caller-passed `msg` via `onToastRequest` | no | S9 |
| SettingsActivity (history export ok/failed) | SettingsActivity.kt:622,625 | Toast | success/danger | `R.string.history_export_ok` / `R.string.history_export_failed` | no | S9 |
| SettingsActivity (history import ok/failed) | SettingsActivity.kt:646-649,652 | Toast | success/danger | `R.string.history_import_ok` (formatted count) / `R.string.history_import_failed` | no | S9 |
| SettingsActivity (compact DB ok/fail) | SettingsActivity.kt:713-716,718-721 | Toast | success/danger | `R.string.toast_compact_db_ok` / `R.string.toast_compact_db_fail` | no | S9 |
| SettingsActivity (test-connection: no transport) | SettingsActivity.kt:782-785 | Toast | danger | **HARDCODED**: "No enabled transport to test — enable Relay or Supabase above." | no | S9 — pre-existing, needs resource extraction |
| SettingsActivity (test-connection: per-transport result) | SettingsActivity.kt:789-810 | Toast | success/danger | **HARDCODED**: "Relay: OK" / "Relay: failed" / "Supabase: OK" / "Supabase: failed" (joined) | no | S9 — pre-existing, needs resource extraction |
| SettingsActivity (discard-changes dialog) | SettingsActivity.kt:367 | Dialog | n/a | `R.string.dialog_unsaved_title` / `R.string.dialog_unsaved_body` | yes: discard / cancel | S9 |
| SettingsActivity (max-items cap-reduction dialog) | SettingsActivity.kt:454 | Dialog | n/a | `R.string.dialog_max_items_reduce_title` / `R.string.dialog_max_items_reduce_body` (formatted) | yes: confirm / cancel | S9 |
| StorageTab (clear-history dialog) | StorageTab.kt:255 | Dialog | n/a | `R.string.dialog_clear_all_title` / `R.string.setting_clear_history_label` | yes: clear / cancel | S9 |
| StorageTab (reset-DB dialog) | StorageTab.kt:282 | Dialog | n/a | `R.string.dialog_reset_db_title` / `R.string.dialog_reset_db_body` | yes: reset / cancel | S9 |
| SyncTab (sync error, unauthorized) | SyncTab.kt:85-89 | Banner | error | `R.string.sync_error_unauthorized` (formatted) | no (credentials fix, not retry) | S11 |
| SyncTab (sync error, generic/transient) | SyncTab.kt:93-108 | Banner | warn | live `syncError` string (source: `FgsSyncLoop`/`SupabasePollWorker`, not a resource key) | yes: `R.string.btn_retry` → `onTestConnection` | S11 |
| SyncTab (cloud-account mismatch) | SyncTab.kt:245-251 | Banner | info | `R.string.setting_cloud_account_mismatch_title` + `_body` | no | S11 |
| OnboardingScreen (OEM autostart hint) | OnboardingScreen.kt:78 | Toast | info | caller-passed `oemHint` (resource) | no | S10 |
| OnboardingScreen (bg-capture ADB step) | OnboardingScreen.kt:159 | Toast | success | caller-passed `msg` via `onToastRequest` (resource) | no | S10 |
| OnboardingDialogs (crash-detected) | OnboardingDialogs.kt:20 | Dialog | n/a | `R.string.crash_detected_title` / `_message` / `_export` | yes: export | S10 |
| DevicesScreen (fingerprint copied) | DevicesScreen.kt:92 | Toast | accent | `R.string.devices_fingerprint_copied` | no | (Devices, not S11) |
| DevicesDialogs (forget/unpair, revoke, rotate, revoke-error, revoke-all, scanner-unavailable) | DevicesDialogs.kt:53,97,153,216,235,284 | Dialog | n/a | all `R.string.dialog_*`/`R.string.devices_*` resources | yes on 4 of 6 (unpair/revoke/rotate/revoke-all confirm) | (Devices, not S11) |
| HistorySelectionBar (bulk-delete confirm) | HistorySelectionBar.kt:145 | Dialog | n/a | caller-passed `title`/`message` (resource) | yes: confirm | (History, not S11) |
| SasPairingDialog (pairing SAS) | SasPairingDialog.kt:381 | Dialog | n/a | `R.string.sas_dialog_title` (formatted) + conditional body | yes: confirm/reject | (Devices, not S11) |
| PairSuccessPopup (pair success) | PairSuccessPopup.kt:57 | Dialog | n/a | `R.string.s8_pair_success_title` | no (dismiss only) | (Devices, not S11) |
| PairScreen (fingerprint copied) | PairScreen.kt:85 | Toast | accent | **HARDCODED**: "Fingerprint copied" | no | (Devices, not S11) — pre-existing, needs resource extraction |
| PairScreen (deep-link error) | PairScreen.kt:216 | Toast | danger | caller-passed `errMsg` (resource/sanitized) | no | (Devices, not S11) |
| PairScreen (controller error) | PairScreen.kt:224 | Toast | danger | `controller.errorMessage` — pre-sanitized via `ErrorMessages.friendly*` | no | (Devices, not S11) |
| AboutScreen (no browser handler) | AboutActivity.kt:203 | Toast | danger | `linkFailedMsg` (resource) | no | S11 |
| LogExportHelper (no-callback fallback) | LogExportHelper.kt:48 | Toast.makeText | (system default) | `msg` passed by caller (resource: `R.string.log_export_empty` or an error string) — legacy fallback path, only hit if caller omits `onError` | no | S11 (verify all current callers supply `onError` so this path stays unreached in-app) |
| ServiceNotifications (copy event) | ServiceNotifications.kt:88-121 | Notif | (silent, badge only) | `R.string.notif_copy_event_title` / `_content` | no | S12 (full per-channel table there) |
| ServiceNotifications (sensitive-skip) | ServiceNotifications.kt:135-161 | Notif | (silent, badge only) | `R.string.notif_sensitive_skip_title` / `_content` | no | S12 |
| ServiceNotifications (incoming pair request) | ServiceNotifications.kt:248-282 | Notif | (high-priority) | `R.string.notif_pair_request_title` / `_content` (formatted) / `_content_unknown` | yes: `R.string.notif_pair_action_confirm` → `DevicesActivity` SAS | S12 |
| ServiceNotifications (foreground service) | ServiceNotifications.kt:292,408-422 | Notif | (ongoing) | `buildNotification` resources | n/a (persistent, not dismissible) | S12 |

**Hardcoded producer strings found (pre-existing, outside S11 file scope except LogViewerActivity's clear-logs
dialog): `SettingsActivity.kt` sync test-connection toasts (lines 782-785, 796, 804), `PairScreen.kt:85`
("Fingerprint copied" — note `DevicesScreen.kt` has the equivalent already resourced as
`R.string.devices_fingerprint_copied`, so `PairScreen` is the odd one out), and `LogViewerActivity.kt:159`
(clear-logs `GlassAlertDialog` title/text/buttons). These predate this slice; migration to `ui/GlassToast`/
banners on status tokens (task 11.1) should also extract them to string resources, but that migration is not
part of this wave's diff — flagged here per the inventory mandate, not fixed silently.

## S12 — System & invisible surfaces  (`android-system-surfaces`, PRESERVE — no golden)

| Component | File | Kind | Disposition |
|---|---|---|---|
| ClipboardFloatingActivity | ClipboardFloatingActivity.kt | Activity (UI-less) | Preserve |
| CaptureOverlayController | CaptureOverlayController.kt | overlay (1×1) | Preserve |
| ShareReceiverActivity | ShareReceiverActivity.kt | Activity (UI-less) | Preserve |
| Notification builders | ServiceNotifications.kt, NotificationHelper.kt | non-view | Restyle (localize/brand only) |
| BootReceiver / CaptureControlReceiver | *.kt | receiver | Preserve |

---

## Resource / system-visible surfaces (non-composable, P0-4)

| Surface | Disposition | Slice |
|---|---|---|
| Launcher / adaptive icon | Restyle/confirm brand mark | S1 |
| Splash / starting window (Android 12) | Done — `androidx.core:core-splashscreen` + `Theme.CopyPaste.Splash` | S12 |
| XML themes + `values-night` | Restyle (resolved-theme; drive first paint) | S1 |
| Status/navigation-bar icon appearance | New (D16, resolved-theme) | S1/S4 |
| Recents thumbnail | Preserve privacy (FLAG_SECURE where sensitive) | S1/S12 |
| Sharesheet target label/icon | Preserve/localize | S12 |
| Notification small icon | Done (alpha-only `ic_stat_notify` + monochrome adaptive layer) | S12 |

## Notes on model types (review M4)

`OwnDeviceInfo` and `PairedDevice` referenced in `android-devices/spec.md` are **presentation DTOs
to be introduced** — the real roster model is `PairedPeer`. Their construction/adaptation is assigned
to S7. Specs that name them treat them as new, not existing, types.
