## ADDED Requirements

### Requirement: Notification channel identity and localization

The app SHALL create and maintain its **four** notification channels — `copypaste_service`
(foreground-service status), `copypaste_copy_event` (per-copy silent badge), `copypaste_pair_request`
(incoming pairing alert), and `copypaste_sync` (`NotificationHelper.CHANNEL_SYNC`, native/encryption-
library-failure alert) — via idempotent creation, with channel names/descriptions and all posted
notification titles/bodies/action labels sourced from localized string resources (the `copypaste_sync`
channel currently has hardcoded English strings that MUST be moved to resources), a consistent small
icon, and the accent color where the platform honors notification tinting.

#### Scenario: Channel creation is idempotent
- **WHEN** channel creation runs more than once (any of the four channels)
- **THEN** no duplicate channel is created and existing channel settings are left untouched

#### Scenario: Sync/native-unavailable channel is inventoried and localized
- **WHEN** `NotificationHelper` posts the native-unavailable alert on `copypaste_sync`
- **THEN** its name/description/title/body resolve from localized resources, and its importance/
  category/visibility are specified (not left as hardcoded English)

#### Scenario: All posted text is localized
- **WHEN** a notification is built for any of the three channels
- **THEN** its title, body, and action labels resolve via `getString`/`stringResource`, never a
  hardcoded literal — including `NotificationHelper`'s native-unavailable notification, which
  SHALL be moved off its current hardcoded strings

### Requirement: Notification behavioral properties are preserved

The redesign SHALL preserve each notification's existing behavioral properties unchanged:
`copypaste_service` stays `IMPORTANCE_LOW`/silent/no-badge, `copypaste_copy_event` stays
`IMPORTANCE_MIN`/silent/auto-cancel/debounced, `copypaste_pair_request` stays
`IMPORTANCE_HIGH`/badged, the foreground-service notification keeps `VISIBILITY_SECRET` and
`PRIORITY_LOW`, and every `PendingIntent` keeps its existing target: **Open → `MainActivity`
(getActivity)**, **Pause/Resume → `CaptureControlReceiver` (getBroadcast, `ACTION_PAUSE`/
`ACTION_RESUME`)** — not MainActivity — and the pairing alert → `DevicesActivity` with
`EXTRA_AUTO_OPEN_SAS`. The redesign SHALL NOT promise pixel parity with the OS-rendered shade layout.

#### Scenario: Copy-event notification stays silent
- **WHEN** a clipboard item is captured
- **THEN** the per-copy notification posts on `copypaste_copy_event` with no sound and no
  heads-up, unchanged from today

#### Scenario: Pairing alert opens the SAS modal
- **WHEN** the user taps the incoming-pair-request notification
- **THEN** it opens `DevicesActivity` with `EXTRA_AUTO_OPEN_SAS=true`, auto-opening the SAS
  confirmation modal, unchanged from today

### Requirement: Invisible clipboard-capture overlay preservation

`ClipboardFloatingActivity` and `CaptureOverlayController` SHALL remain genuinely invisible
functional surfaces: the capture overlay SHALL stay a 1×1 `TYPE_APPLICATION_OVERLAY` window
with `FLAG_NOT_TOUCHABLE`/`FLAG_NOT_FOCUSABLE` and zero alpha, gated by
`Settings.canDrawOverlays` before every `addView`, and `ClipboardManager.getPrimaryClip()`
reads SHALL occur only inside the `ViewTreeObserver.OnGlobalLayoutListener` focus callback,
never synchronously after requesting focus. The suppress/restore protocol between
`CaptureOverlayController` and `ClipboardFloatingActivity` SHALL remain idempotent (`add`/
`remove` no-op when already in the target state). The redesign SHALL NOT add a Compose surface,
visible view, or any decoration to this path.

#### Scenario: Overlay gated by permission
- **WHEN** `Settings.canDrawOverlays` is false
- **THEN** `CaptureOverlayController.add()` and `ClipboardFloatingActivity`'s own overlay both
  no-op rather than adding a window

#### Scenario: Clip read happens only after layout focus
- **WHEN** `ClipboardFloatingActivity` requests window focus for its overlay
- **THEN** `getPrimaryClip()` is called only from within the subsequent
  `OnGlobalLayoutListener` callback, never immediately after the focus request

#### Scenario: Suppress/restore is idempotent
- **WHEN** `suppressCaptureOverlay()` or `restoreCaptureOverlay()` is called while the overlay
  is already in the target state
- **THEN** the call is a no-op and does not throw or double-add/double-remove the window

### Requirement: Share-target surface stays UI-less

`ShareReceiverActivity` SHALL remain a translucent, `excludeFromRecents`, `noHistory` activity
that finishes only after its capture coroutine completes reading the shared URI(s) or text,
since the granted read-URI permission's lifetime is tied to the activity's lifecycle and
finishing early would truncate the read. This redesign SHALL NOT add visible UI, a toast, or a
notification to the share-receive path; keeping it UI-less is a resolved decision for this change.

#### Scenario: Finish waits for IO
- **WHEN** a share intent carries an image or file stream
- **THEN** `ShareReceiverActivity` calls `finish()` only after the capture coroutine has read
  the stream, not immediately on `onCreate`

#### Scenario: No UI added by default
- **WHEN** the redesign ships
- **THEN** `ShareReceiverActivity` still presents no visible screen, toast, or notification on
  share completion unless a separate product decision explicitly adds one

#### Scenario: Failure logs redact URIs and content
- **WHEN** a share capture fails and `ShareReceiverActivity` logs the failure
- **THEN** the log line contains only a non-identifying failure category — never the shared URI, file
  name, or content (the current `Log.w(... $uri ...)` paths MUST be redacted)

### Requirement: OS-owned surfaces get correct intents and labels only

The redesign SHALL treat OS- and library-owned surfaces as out of scope for restyling — runtime
permission dialogs, the Android sharesheet, system settings pages (overlay/battery/notification/
app-details), OEM autostart pages, and the ZXing-based QR scanner (`PortraitCaptureActivity`, a
thin `CaptureActivity` subclass that only locks portrait orientation and configures the decoder)
— limiting itself to correct labels, icons, and `Intent` targets for reaching these surfaces and
to the app-owned screens shown immediately before/after them; it SHALL NOT attempt Compose
golden-image parity with any of them.

#### Scenario: ZXing scanner is not re-skinned
- **WHEN** the user opens the QR scanner
- **THEN** `PortraitCaptureActivity` continues to present ZXing's default
  `DecoratedBarcodeView` UI (only orientation/decoder/framing configured), with no custom
  Compose overlay replacing it

#### Scenario: Correct pre/post screens around OS surfaces
- **WHEN** the user is sent to an OS-owned settings page and returns
- **THEN** the app-owned screen shown before departure and the one shown on return are both
  token-styled and accurately reflect the resulting permission state, while the OS page itself
  is untouched

### Requirement: No new user-visible surface is invented under this capability

This capability SHALL only preserve and correctly brand existing system/invisible surfaces; it
SHALL NOT introduce a new AppWidget, quick-settings tile, or other net-new system surface,
since no such surface exists in the current inventory and adding one is out of scope for a
presentation-layer redesign.

#### Scenario: No widget provider added
- **WHEN** this capability is implemented
- **THEN** no `AppWidgetProvider` or quick-settings `TileService` is added to the manifest as
  part of this work
