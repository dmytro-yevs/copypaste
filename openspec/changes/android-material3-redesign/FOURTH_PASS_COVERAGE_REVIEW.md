# Fourth-pass coverage review

Verdict: **view-symbol coverage is effectively complete, but the specification still cannot claim
that every component and behaviour is fully enumerated**.

`behavior-and-state-coverage.md` is a strong improvement. It adds behaviour owners, manifest
components and major flows. The remaining gap is control-level behaviour, especially Settings,
plus several unresolved dispositions hidden behind aggregate rows.

## 1. Component enumeration result

- All detected named composables/extensions appear in `component-inventory.md`.
- All 13 Activity subclasses appear.
- Services, receivers, workers and FileProvider dependencies now appear in the behaviour inventory.
- Major screen interactions have owning slices and evidence categories.

The count explanation in `component-inventory.md` is still incorrect. A reproducible scan currently
finds 117 `@Composable` annotation sites and 116 unique names (the duplicate name is
`rememberReducedMotion`, defined in two files). `Modifier.previewPeekGesture` is already one of the
unique annotated extension names; it does not make unique names exceed annotation sites.

Commit the extraction script or correct the header to avoid unverifiable inventory claims.

## 2. Rows that still have no actionable disposition

These component-inventory helper rows still use `—`:

- `rememberHistoryScreenState`
- `rememberHistoryItemPipelines`
- `HistoryScreenEffects`
- `rememberHistoryFilePickerLauncher`

Give each one a disposition:

- Preserve behaviour and only update dependencies/callers;
- Refactor with a named reason and regression tests; or
- Remove with replacement owner.

They own state/effects/file-picker behaviour, so `Helper | —` does not satisfy “what to do with it.”

The new Banner/pills/SECRET tile may legitimately have no existing file, but assign the target new
file/component owner rather than leaving file=`—` if implementation decomposition matters.

## 3. Settings behaviour is not enumerated at control level

The complete matrix currently reduces all Settings behaviour to about ten generic rows. The actual
tabs expose materially different persistence, security and side-effect behaviour. Add rows for each
group below.

### 3.1 General

- Private mode toggle and resulting History/capture state.
- Global sync enable toggle.
- Public-IP discovery consent and STUN-visible result.
- Paste-as-plain-text behaviour.
- Permissions, Devices, Background Capture, Logs and About navigation.
- Log export success/failure and FileProvider share.
- Logcat capture enable/disable and permission-dependent status.
- ADB command copy feedback.

### 3.2 Display

- Sensitive warning toggle.
- Reveal guard toggle.
- Mask-sensitive toggle and immediate/draft effect boundary.
- `allowScreenshots`: currently writes immediately and mutates `FLAG_SECURE` on the current window.
- Translucency draft preview/persistence.
- Image maximum height.
- Preview delay.
- Preview line count.
- New Theme/System/Accent controls.

### 3.3 Sync

- P2P enable.
- LAN visibility and mDNS registration effect.
- Auto-apply synced clipboard.
- Wi-Fi-only sync.
- Relay enable and Supabase enable as independent additive transports.
- Relay/Supabase enable toggles currently persist **immediately inside `SyncTab`**, unlike the main
  Settings draft model.
- Supabase URL/key/passphrase/account credentials visibility and validation.
- Relay URL validation.
- Sign-in/sign-out/account state.
- Test-connection unavailable/in-flight/success/failure.
- Unauthorized versus generic sync-error banner.
- Cloud-account mismatch remains inert.

### 3.4 Storage

- Each stepped slider and its exact step/sentinel behaviour.
- Lowering max-items and destructive-prune confirmation.
- Excluded-app add/remove/duplicate/invalid input.
- Export history through SAF.
- Include-sensitive export toggle: ephemeral, default off, plaintext warning.
- Import history success/partial/failure/dedup behaviour.
- Clear history confirmation/outcome.
- Reset database confirmation/outcome.
- Vacuum/compact unavailable/in-flight/success/failure.

### 3.5 Notifications

- Notify-on-copy toggle.
- Sound-on-copy toggle.
- Dependency rule, if sound is ineffective/disabled when notify-on-copy is off.
- Relationship to POST_NOTIFICATIONS denial and foreground-service visibility.

Each row needs persistence mode (draft/immediate/ephemeral), side effect, failure handling,
localization, fixture/golden and test.

## 4. A real Settings contract conflict remains

The specs broadly say Settings uses one draft model and atomic Save. Current code contains at least
two immediate-write paths:

- `DisplayTab.allowScreenshots` writes `Settings.allowScreenshots` immediately and changes
  `FLAG_SECURE` immediately.
- `SyncTab.relayEnabled` and `SyncTab.supabaseEnabled` write preferences immediately.

The plan must explicitly choose per control:

- preserve immediate application;
- convert to draft + Save;
- or use draft UI while maintaining a justified immediate security/runtime side effect.

Do not silently convert these behaviours under a visual redesign. Add a persistence-mode table for
every Settings field. The atomic `saveScreenSettings` requirement applies only to fields owned by
Save; immediate and ephemeral controls need separate explicit contracts.

## 5. Major-flow matrix still contains ambiguous behaviour

- Devices `expand/collapse card (if retained)` has no decision. Choose retain/remove; then specify
  behaviour and tests.
- History error/degraded is new, but the trigger/state source remains abstract. Name the exact
  presentation-state input and how existing repository errors reach it.
- Preview “large content scroll/zoom” groups two distinct behaviours; state whether image zoom
  actually exists and is preserved, newly added or out of scope.
- About “links/licenses/no-handler” needs individual link destinations and license presentation
  ownership.
- Notification row aggregates foreground, copy, pairing, native-unavailable and restart
  notifications. List each variant with channel, importance, visibility, actions, PendingIntent,
  debounce/auto-cancel and localization disposition.
- Share receiver failure logging is invisible, but security/privacy must specify that URI/file names
  and content are not leaked into logs.
- System-owned intents need each action/URI/fallback, not only a generic “correct Intent.”

## 6. Behaviour-owner inventory is improved but not fully exhaustive

Add or explicitly classify these owners where their outcome is user-visible:

- `HistoryFilePicker.kt` — SAF launcher/result/cancel/error.
- `HistoryImageCache.kt` / thumbnail helpers — loading/fallback/cache invalidation visible in rows.
- `HistoryScreenState.kt` — search/filter/load-more/preview/selection state orchestration.
- `SettingsActivity.persistAll` / save result — commit failure and post-save navigation.
- `SettingsUtils.kt` — slider snapping/formatting/sentinels.
- `ServiceNotifications.kt` and `NotificationHelper.kt` as separate builders, not only through
  service ownership.
- `NotificationPermissionHelper` interaction with MainActivity's first-launch service start.
- `PairActivity` deep-link/new-intent routing owner.
- `DevicesController` dialog-dismiss/in-flight guards.
- `LogViewerActivity` log clear/filter/export state orchestration.

Not every pure helper needs a row. The rule should be: add it when it owns a user-observable state
transition, security boundary, external intent, or failure mapping.

## 7. Evidence labels need exact commands

Entries such as `connected`, `security-semantics`, `channel+action tests`, and `intent unit` are
categories, not executable evidence. Before S0 closes, map them to:

- test class/file to add;
- runner (JVM, Paparazzi, connected Android test, manual);
- exact command;
- emulator/device prerequisite;
- CI job or explicit manual milestone.

Otherwise the matrix can claim evidence that no gate actually executes.

## 8. Remaining cross-document cleanup

- `tasks.md` traceability still shows Golden infra owned by S14. Align it to S0 spike → S2 infra →
  per-screen baselines → S14 audit.
- `design.md` D14 still says test infra “S14 lands early”; S14 is late. State that S2 establishes it.
- Proposal Impact still calls Paparazzi/Lucide compatible before S0 proves exact versions. Say
  compatibility is a blocking S0 proof.
- Remove any stale `open decision` labels already resolved in capability specs.

## 9. Final coverage rule

The updated documents now prove symbol-level coverage and broad-flow coverage. They do **not yet**
prove control-level behavioural coverage.

Coverage is sufficient when:

1. every composable/Activity/manifest component is inventoried;
2. every user-observable state owner is inventoried;
3. every interactive control has a persistence/side-effect/failure disposition;
4. every ambiguous “if retained” choice is resolved;
5. every evidence label maps to a real test command or explicit N/A/manual check.
