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
| Splash / starting window (Android 12) | Restyle → canonical first paint (no flash) | S1 |
| XML themes + `values-night` | Restyle (resolved-theme; drive first paint) | S1 |
| Status/navigation-bar icon appearance | New (D16, resolved-theme) | S1/S4 |
| Recents thumbnail | Preserve privacy (FLAG_SECURE where sensitive) | S1/S12 |
| Sharesheet target label/icon | Preserve/localize | S12 |
| Notification small icon | Restyle/confirm | S12 |

## Notes on model types (review M4)

`OwnDeviceInfo` and `PairedDevice` referenced in `android-devices/spec.md` are **presentation DTOs
to be introduced** — the real roster model is `PairedPeer`. Their construction/adaptation is assigned
to S7. Specs that name them treat them as new, not existing, types.
