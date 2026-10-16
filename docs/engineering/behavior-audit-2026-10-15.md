# CopyPaste unexpected behavior audit

**Implementation follow-up, 16 October 2026:** F01–F08 and F10–F21 have been corrected in the working tree. F09 remains unchanged at the user's explicit request (item 3 in the second selected list). The original findings below describe the audit-time behavior; see the implementation record at the end for current outcomes and verification.

Audit date: 15 October 2026. Scope: the working tree observed at audit start, including existing uncommitted privacy, History and Quick Paste changes. Initial HEAD: `8311dba370daa0129531f676b29b1f7e50bde6a6`. Those changes were committed independently during the audit; final observed HEAD: `b7a3744890fbbf8bda92aade126b6641a8f9154b`. This report is not release qualification.

The audit retained 21 concrete code-path scenarios: six P1, thirteen P2 and two P3. The most important patterns are stale privacy state, errors converted into disabled/signed-out state, acknowledged work that was not stored, and operations whose partial success is hidden from the user. Two Flutter scenarios were reproduced in isolated controller tests. Intentional product behavior and unresolved native hypotheses are listed separately below.

P1 means a privacy, account-state or content-loss issue to address first. P2 means a functional or recovery issue. P3 means a narrow, lower-probability integrity or attribution issue. Severity reflects the documented trigger; it does not assume an attacker or claim an untested exploit.

## Findings index

| ID | Priority | Scenario | Platform or surface |
| --- | --- | --- | --- |
| F01 | P1 | A remote secrecy change leaves inspector plaintext visible | macOS, Android, Windows; History and desktop Quick Paste |
| F02 | P1 | Deduplication conflates text and binary representations | Shared core capture on macOS, Android, Windows |
| F03 | P1 | An SMS module lookup error becomes a destructive disabled state | Android |
| F04 | P1 | SMS processing failures are acknowledged before successful storage | Android |
| F05 | P1 | Granting screenshot access can import the denied interval | Android |
| F06 | P1 | Cloud sign out can succeed without clearing credentials | Current desktop CLI and shared cloud API; Flutter bridge exposes it but current screens do not call it |
| F07 | P2 | Unreadable settings are displayed as user choices | macOS, Android, Windows |
| F08 | P2 | History never reconnects after its watch stream ends | Shared Flutter History; desktop transport EOF is a concrete trigger |
| F09 | P2 | Quick Paste silently hides action failures | Desktop Quick Paste on macOS and Windows |
| F10 | P2 | Text export hides unreadable text omissions | macOS, Android, Windows Flutter Settings |
| F11 | P2 | A partially failed text import misses sync notifications | Desktop daemon text-import API and CLI; not Android explicit file import |
| F12 | P2 | Semantic search silently degrades with a corrupt model | macOS, Android, Windows with Semantic Search installed |
| F13 | P2 | Module removal can report failure after removing the module | macOS, Android, Windows |
| F14 | P2 | Failed foreground capture setup remains enabled for a later resume | Android onboarding |
| F15 | P2 | Native capture failures leave contradictory setting outcomes | Android |
| F16 | P2 | SMS enable can commit while monitor startup and rollback fail | Android; conditional on two failures |
| F17 | P2 | Revocation does not terminate an authenticated session | P2P on macOS, Android, Windows |
| F18 | P2 | Staged plaintext loses its cleanup owner after an early restart | macOS; shared Unix staging also used by Linux |
| F19 | P2 | A locked staged file can block Windows runtime startup | Windows; depends on the other application denying deletion |
| F20 | P3 | A public clipboard label can suppress another app capture | Android; uncommon label collision |
| F21 | P3 | Origin lookup failure mislabels a remote clip as local | macOS and Windows daemon; Android already preserves origin identity |

## Findings and regression contracts

### F01 A remote secrecy change leaves inspector plaintext visible

**P1 · macOS, Android, Windows; History and desktop Quick Paste**

**Trigger:** Open an ordinary clip in the inspector. Mark the same clip secret on a paired device, then receive its items-change event.

**Actual behavior:** The list is refreshed with secret=true and a hidden preview, but the selected detail object retains secret=false and the previously loaded body. The inspector therefore keeps rendering plaintext without the secret spoiler.

**Expected contract and verification:** Reconcile or clear the selected detail and cached presentation whenever the item privacy version changes. A regression must assert both the list and open inspector hide the body.

**Evidence:** [apps/copypaste_flutter/lib/features/history/controller/history_controller.dart:438](/Users/dmytro/Documents/CopyPaste/apps/copypaste_flutter/lib/features/history/controller/history_controller.dart:438); [apps/copypaste_flutter/lib/features/history/controller/history_controller.dart:604](/Users/dmytro/Documents/CopyPaste/apps/copypaste_flutter/lib/features/history/controller/history_controller.dart:604); [apps/copypaste_flutter/lib/features/history/view/history_inspector.dart:43](/Users/dmytro/Documents/CopyPaste/apps/copypaste_flutter/lib/features/history/view/history_inspector.dart:43).

**Validation:** Reproduced with a temporary Flutter controller test: after the watch refresh, items.single.secret was true while selectedClip.secret remained false and selectedClip.body still contained the original text.

### F02 Deduplication conflates text and binary representations

**P1 · Shared core capture on macOS, Android, Windows**

**Trigger:** Capture text hello, then copy an ordinary file whose bytes are exactly hello through the system clipboard. Reverse the order for the inverse outcome. This concerns automatic capture; explicit file import has a separate insertion path.

**Actual behavior:** Both captures have the same raw-byte SHA-256 hash. The dedup query ignores content type and only bumps the existing row, retaining its original payload/type/metadata. The later file becomes a text row, or later text remains a file. Different text representations can collide too.

**Expected contract and verification:** Define representation identity explicitly and retain each meaningful type. Add text/file and plain/HTML/RTF collision tests in both insertion orders.

**Evidence:** [crates/copypaste-core/src/binary.rs:44](/Users/dmytro/Documents/CopyPaste/crates/copypaste-core/src/binary.rs:44); [crates/copypaste-core/src/storage/items.rs:164](/Users/dmytro/Documents/CopyPaste/crates/copypaste-core/src/storage/items.rs:164); [crates/copypaste-core/src/storage/retention.rs:247](/Users/dmytro/Documents/CopyPaste/crates/copypaste-core/src/storage/retention.rs:247); [crates/copypaste-core/src/storage/retention.rs:273](/Users/dmytro/Documents/CopyPaste/crates/copypaste-core/src/storage/retention.rs:273).

**Validation:** Code path verified; failure scenario not executed on a native device.

### F03 An SMS module lookup error becomes a destructive disabled state

**P1 · Android**

**Trigger:** With SMS Codes enabled, fail the runtime module-handler lookup during receiver or synchronization processing.

**Actual behavior:** has_sms_module converts the error to false. Android interprets false as disabled, stops the service, disables its receiver and clears activation time and the last processed message ID. Recovery of the original lookup does not itself restore monitoring.

**Expected contract and verification:** Keep available, disabled and unavailable/error distinct. Preserve the enabled preference and cursor on lookup failure; expose a recoverable unavailable state.

**Evidence:** [crates/copypaste-runtime/src/lib.rs:733](/Users/dmytro/Documents/CopyPaste/crates/copypaste-runtime/src/lib.rs:733); [crates/copypaste-flutter-bridge/src/runtime_android.rs:542](/Users/dmytro/Documents/CopyPaste/crates/copypaste-flutter-bridge/src/runtime_android.rs:542); [apps/copypaste_flutter/android/app/src/main/kotlin/com/copypaste/app/SmsModuleReceiver.kt:14](/Users/dmytro/Documents/CopyPaste/apps/copypaste_flutter/android/app/src/main/kotlin/com/copypaste/app/SmsModuleReceiver.kt:14); [apps/copypaste_flutter/android/app/src/main/kotlin/com/copypaste/app/SmsModuleService.kt:28](/Users/dmytro/Documents/CopyPaste/apps/copypaste_flutter/android/app/src/main/kotlin/com/copypaste/app/SmsModuleService.kt:28); [apps/copypaste_flutter/android/app/src/main/kotlin/com/copypaste/app/SmsInboxMonitor.kt:94](/Users/dmytro/Documents/CopyPaste/apps/copypaste_flutter/android/app/src/main/kotlin/com/copypaste/app/SmsInboxMonitor.kt:94).

**Validation:** Code path verified; failure scenario not executed on a native device.

### F04 SMS processing failures are acknowledged before successful storage

**P1 · Android**

**Trigger:** An SMS containing a valid code arrives; native dispatch, storage or module execution fails for that message.

**Actual behavior:** The inbox monitor commits last-id before invoking ingestion and ignores the returned Boolean. Subsequent scans start beyond the failed message, so its code is never retried. Intentional pause/no-code decisions and operational failure share the same consumed outcome.

**Expected contract and verification:** Return an explicit result distinguishing consumed, rejected by policy and failed. Advance the durable cursor only for a terminal consumed/policy result, while preserving the existing no-replay policy for deliberate pause.

**Evidence:** [apps/copypaste_flutter/android/app/src/main/kotlin/com/copypaste/app/SmsInboxMonitor.kt:79](/Users/dmytro/Documents/CopyPaste/apps/copypaste_flutter/android/app/src/main/kotlin/com/copypaste/app/SmsInboxMonitor.kt:79); [crates/copypaste-flutter-bridge/src/runtime_android.rs:574](/Users/dmytro/Documents/CopyPaste/crates/copypaste-flutter-bridge/src/runtime_android.rs:574); [crates/copypaste-runtime/src/lib.rs:740](/Users/dmytro/Documents/CopyPaste/crates/copypaste-runtime/src/lib.rs:740).

**Validation:** Code path verified; failure scenario not executed on a native device.

### F05 Granting screenshot access can import the denied interval

**P1 · Android**

**Trigger:** Leave screenshot capture enabled, deny or revoke media access, take screenshots, then grant access again and resume the app.

**Actual behavior:** The missing-permission path exits without advancing since. The next permitted scan queries from the old cursor and imports screenshots created while access was unavailable. Settings promises new screenshots; intentional pause already advances the cursor to prevent replay.

**Expected contract and verification:** Make the capture boundary explicit on permission grant/regrant. If backlog import is desired, offer it separately; test denied, revoked and paused intervals independently.

**Evidence:** [apps/copypaste_flutter/android/app/src/main/kotlin/com/copypaste/app/ScreenshotCaptureMonitor.kt:93](/Users/dmytro/Documents/CopyPaste/apps/copypaste_flutter/android/app/src/main/kotlin/com/copypaste/app/ScreenshotCaptureMonitor.kt:93); [apps/copypaste_flutter/android/app/src/main/kotlin/com/copypaste/app/ScreenshotCaptureState.kt:15](/Users/dmytro/Documents/CopyPaste/apps/copypaste_flutter/android/app/src/main/kotlin/com/copypaste/app/ScreenshotCaptureState.kt:15); [apps/copypaste_flutter/lib/features/settings/view/settings_screen.dart:849](/Users/dmytro/Documents/CopyPaste/apps/copypaste_flutter/lib/features/settings/view/settings_screen.dart:849).

**Validation:** Code path verified; failure scenario not executed on a native device.

### F06 Cloud sign out can succeed without clearing credentials

**P1 · Current desktop CLI and shared cloud API; Flutter bridge exposes it but current screens do not call it**

**Trigger:** Sign in to Supabase, disable the provider module without removing it, then issue cloud sign-out through the CLI/API.

**Actual behavior:** The unavailable-provider guard returns a synthetic signed-out status for CloudSignOut. It never invokes the provider sign-out implementation that clears stored credentials. Re-enabling the module can resume the account despite the earlier success response.

**Expected contract and verification:** Ensure sign-out clears persisted account state independently of provider execution, or report an explicit failure. Unavailable provider state must not be represented as a completed sign-out.

**Evidence:** [crates/copypaste-modules/src/cloud_request.rs:16](/Users/dmytro/Documents/CopyPaste/crates/copypaste-modules/src/cloud_request.rs:16); [modules/supabase/src/lib.rs:350](/Users/dmytro/Documents/CopyPaste/modules/supabase/src/lib.rs:350); [crates/copypaste-cli/src/main.rs:172](/Users/dmytro/Documents/CopyPaste/crates/copypaste-cli/src/main.rs:172); [crates/copypaste-flutter-bridge/src/api.rs:749](/Users/dmytro/Documents/CopyPaste/crates/copypaste-flutter-bridge/src/api.rs:749).

**Validation:** Code path verified; failure scenario not executed on a native device.

### F07 Unreadable settings are displayed as user choices

**P2 · macOS, Android, Windows**

**Trigger:** Persist an unreadable settings record or invalid field, then restart the runtime.

**Actual behavior:** The runtime exposes settings_health and applies fallback values, but the Flutter bridge drops that health field. Settings renders the fallback switches as normal selected values, hiding why capture behavior changed. The IPC contract explicitly requires a visible warning in this state.

**Expected contract and verification:** Propagate settings health through bridge, repository and UI. Display which settings could not be read and distinguish temporary safety behavior from persisted user choices.

**Evidence:** [crates/copypaste-ipc/src/payload/mod.rs:457](/Users/dmytro/Documents/CopyPaste/crates/copypaste-ipc/src/payload/mod.rs:457); [crates/copypaste-daemon/src/server/items.rs:80](/Users/dmytro/Documents/CopyPaste/crates/copypaste-daemon/src/server/items.rs:80); [crates/copypaste-flutter-bridge/src/api.rs:351](/Users/dmytro/Documents/CopyPaste/crates/copypaste-flutter-bridge/src/api.rs:351); [crates/copypaste-flutter-bridge/src/api.rs:664](/Users/dmytro/Documents/CopyPaste/crates/copypaste-flutter-bridge/src/api.rs:664); [apps/copypaste_flutter/lib/features/settings/repository/runtime_settings_repository.dart:79](/Users/dmytro/Documents/CopyPaste/apps/copypaste_flutter/lib/features/settings/repository/runtime_settings_repository.dart:79).

**Validation:** Code path verified; failure scenario not executed on a native device.

### F08 History never reconnects after its watch stream ends

**P2 · Shared Flutter History; desktop transport EOF is a concrete trigger**

**Trigger:** End the live history watch while keeping the controller alive. Reinitialize or reload it after the repository can provide a fresh stream.

**Actual behavior:** The old subscription remains non-null. initialize uses null-coalescing assignment, so it never creates a replacement watch. Manual reads can succeed while future captures no longer refresh History. There is also no action for stream error/completion in the listener.

**Expected contract and verification:** Handle stream completion/error, clear the dead subscription, expose unavailable state where appropriate and reconnect under an explicit lifecycle policy.

**Evidence:** [crates/copypaste-flutter-bridge/src/client.rs:171](/Users/dmytro/Documents/CopyPaste/crates/copypaste-flutter-bridge/src/client.rs:171); [apps/copypaste_flutter/lib/features/history/repository/runtime_history_repository.dart:29](/Users/dmytro/Documents/CopyPaste/apps/copypaste_flutter/lib/features/history/repository/runtime_history_repository.dart:29); [apps/copypaste_flutter/lib/features/history/controller/history_controller.dart:393](/Users/dmytro/Documents/CopyPaste/apps/copypaste_flutter/lib/features/history/controller/history_controller.dart:393).

**Validation:** Reproduced with a temporary Flutter controller test: after closing the first stream and reinitializing, watch() was still called only once; events on the replacement stream caused no reads.

### F09 Quick Paste silently hides action failures

**P2 · Desktop Quick Paste on macOS and Windows**

**Trigger:** Make a repository copy, download, pin, delete or clear operation fail after Quick Paste has loaded successfully.

**Actual behavior:** History records an error message, but Quick Paste renders it only for the initial load-error state. Primary activation does nothing after copy failure; inspector actions or dialogs remain without a visible failure explanation.

**Expected contract and verification:** Provide a shared action-error surface in the ready state, including inside destructive dialogs. Keep failed native auto-paste separate from a failed clipboard copy.

**Evidence:** [apps/copypaste_flutter/lib/features/quick_paste/quick_paste_controller.dart:312](/Users/dmytro/Documents/CopyPaste/apps/copypaste_flutter/lib/features/quick_paste/quick_paste_controller.dart:312); [apps/copypaste_flutter/lib/features/quick_paste/quick_paste_view.dart:218](/Users/dmytro/Documents/CopyPaste/apps/copypaste_flutter/lib/features/quick_paste/quick_paste_view.dart:218); [apps/copypaste_flutter/lib/features/history/controller/history_controller.dart:629](/Users/dmytro/Documents/CopyPaste/apps/copypaste_flutter/lib/features/history/controller/history_controller.dart:629); [apps/copypaste_flutter/lib/features/history/view/history_inspector.dart:531](/Users/dmytro/Documents/CopyPaste/apps/copypaste_flutter/lib/features/history/view/history_inspector.dart:531); [apps/copypaste_flutter/lib/features/history/view/history_delete_dialog.dart:46](/Users/dmytro/Documents/CopyPaste/apps/copypaste_flutter/lib/features/history/view/history_delete_dialog.dart:46).

**Validation:** Code path verified; failure scenario not executed on a native device.

### F10 Text export hides unreadable text omissions

**P2 · macOS, Android, Windows Flutter Settings**

**Trigger:** Export text history containing at least one text row that cannot be decrypted.

**Actual behavior:** The backend correctly counts omitted rows and the repository preserves the counts. SettingsController discards the result and presents the file with no omission warning. Non-text omission is expected from a text export; missing readable/exportable text is the important undisclosed distinction. Size-limit failures return errors and are not this issue.

**Expected contract and verification:** Show exported count and unreadable omissions, with non-text exclusions clearly labeled. Test a nonzero skippedUndecryptable result.

**Evidence:** [crates/copypaste-core/src/transfer/export.rs:18](/Users/dmytro/Documents/CopyPaste/crates/copypaste-core/src/transfer/export.rs:18); [crates/copypaste-core/src/transfer/mod.rs:8](/Users/dmytro/Documents/CopyPaste/crates/copypaste-core/src/transfer/mod.rs:8); [apps/copypaste_flutter/lib/features/settings/repository/runtime_settings_repository.dart:105](/Users/dmytro/Documents/CopyPaste/apps/copypaste_flutter/lib/features/settings/repository/runtime_settings_repository.dart:105); [apps/copypaste_flutter/lib/features/settings/controller/settings_controller.dart:243](/Users/dmytro/Documents/CopyPaste/apps/copypaste_flutter/lib/features/settings/controller/settings_controller.dart:243).

**Validation:** Code path verified; failure scenario not executed on a native device.

### F11 A partially failed text import misses sync notifications

**P2 · Desktop daemon text-import API and CLI; not Android explicit file import**

**Trigger:** Import old-stamped text history after cloud sync has advanced its upload floor. Let an early item commit, then inject a later storage/encryption failure.

**Actual behavior:** Core keeps earlier committed rows but returns Err. The daemon lowers upload floors and announces changes only in the success branch. Those old rows can remain local and absent from ordinary future uploads until another operation explicitly moves the floor back.

**Expected contract and verification:** Make the batch atomic or return/report partial progress and announce every committed row even on a later failure. Test post-first-commit failure and the upload floor.

**Evidence:** [crates/copypaste-core/src/transfer/import.rs:95](/Users/dmytro/Documents/CopyPaste/crates/copypaste-core/src/transfer/import.rs:95); [crates/copypaste-core/src/transfer/import.rs:138](/Users/dmytro/Documents/CopyPaste/crates/copypaste-core/src/transfer/import.rs:138); [crates/copypaste-core/src/transfer/import.rs:176](/Users/dmytro/Documents/CopyPaste/crates/copypaste-core/src/transfer/import.rs:176); [crates/copypaste-daemon/src/server/transfer.rs:30](/Users/dmytro/Documents/CopyPaste/crates/copypaste-daemon/src/server/transfer.rs:30).

**Validation:** Code path verified; failure scenario not executed on a native device.

### F12 Semantic search silently degrades with a corrupt model

**P2 · macOS, Android, Windows with Semantic Search installed**

**Trigger:** Corrupt a downloaded model file without changing its length, then reload/enable the module and search.

**Actual behavior:** Availability checks only file type and length, so the UI can report downloaded/enabled. Model hash verification later rejects the resource, while embedding errors become an empty semantic result and ordinary lexical search continues without a degraded-state warning.

**Expected contract and verification:** Track verified model health and show unavailable/degraded semantic search independently of lexical search. Provide a repair path and test same-length corruption.

**Evidence:** [crates/copypaste-modules/src/resources.rs:17](/Users/dmytro/Documents/CopyPaste/crates/copypaste-modules/src/resources.rs:17); [crates/copypaste-modules/src/manager.rs:315](/Users/dmytro/Documents/CopyPaste/crates/copypaste-modules/src/manager.rs:315); [crates/copypaste-modules/src/manager.rs:521](/Users/dmytro/Documents/CopyPaste/crates/copypaste-modules/src/manager.rs:521); [crates/copypaste-modules/src/native.rs:51](/Users/dmytro/Documents/CopyPaste/crates/copypaste-modules/src/native.rs:51); [crates/copypaste-modules/src/semantic_search.rs:192](/Users/dmytro/Documents/CopyPaste/crates/copypaste-modules/src/semantic_search.rs:192); [crates/copypaste-modules/src/semantic_search.rs:250](/Users/dmytro/Documents/CopyPaste/crates/copypaste-modules/src/semantic_search.rs:250).

**Validation:** Code path verified; failure scenario not executed on a native device.

### F13 Module removal can report failure after removing the module

**P2 · macOS, Android, Windows**

**Trigger:** Remove a semantic module; let package/registry removal succeed and fail the subsequent semantic-index cleanup.

**Actual behavior:** The host returns an error from cleanup after the module has already been disabled/removed. A retry can find no installed module. The operation result does not distinguish removal success from deferred index cleanup failure.

**Expected contract and verification:** Report the actual removal result with a separate cleanup state, or arrange transaction/recovery semantics that preserve an accurate operation outcome.

**Evidence:** [crates/copypaste-modules/src/host.rs:287](/Users/dmytro/Documents/CopyPaste/crates/copypaste-modules/src/host.rs:287); [crates/copypaste-modules/src/manager.rs:443](/Users/dmytro/Documents/CopyPaste/crates/copypaste-modules/src/manager.rs:443).

**Validation:** Code path verified; failure scenario not executed on a native device.

### F14 Failed foreground capture setup remains enabled for a later resume

**P2 · Android onboarding**

**Trigger:** Choose foreground capture while host/admission acquisition fails; later resume the app after the host becomes available.

**Actual behavior:** The native channel persists foreground-capture-enabled=true before acquisition succeeds. Onboarding reports failure and stays incomplete, but the next activity resume can start capture using the persisted flag.

**Expected contract and verification:** Roll back the preference or expose a pending/recovering state that clearly says capture will start automatically when available.

**Evidence:** [apps/copypaste_flutter/android/app/src/main/kotlin/com/copypaste/app/AndroidCaptureChannel.kt:110](/Users/dmytro/Documents/CopyPaste/apps/copypaste_flutter/android/app/src/main/kotlin/com/copypaste/app/AndroidCaptureChannel.kt:110); [apps/copypaste_flutter/android/app/src/main/kotlin/com/copypaste/app/AndroidCaptureState.kt:54](/Users/dmytro/Documents/CopyPaste/apps/copypaste_flutter/android/app/src/main/kotlin/com/copypaste/app/AndroidCaptureState.kt:54); [apps/copypaste_flutter/android/app/src/main/kotlin/com/copypaste/app/MainActivity.kt:428](/Users/dmytro/Documents/CopyPaste/apps/copypaste_flutter/android/app/src/main/kotlin/com/copypaste/app/MainActivity.kt:428); [apps/copypaste_flutter/lib/features/onboarding/controller/android_onboarding_controller.dart:301](/Users/dmytro/Documents/CopyPaste/apps/copypaste_flutter/lib/features/onboarding/controller/android_onboarding_controller.dart:301).

**Validation:** Code path verified; failure scenario not executed on a native device.

### F15 Native capture failures leave contradictory setting outcomes

**P2 · Android**

**Trigger:** Disable Save screenshots and make service draining fail. A related case is Pause capture when native stop/drain fails after the runtime setting commits.

**Actual behavior:** Screenshot disable persists enabled=false before draining; on error Dart retains its old enabled=true value. Restart uses OFF although the visible switch stayed ON. General capture pause similarly returns an overall failure after parts of the pause have committed, without a structured partial outcome.

**Expected contract and verification:** Reread authoritative state on failure and distinguish preference saved from service still draining. Do not show the old preference as though the entire operation was rolled back.

**Evidence:** [apps/copypaste_flutter/android/app/src/main/kotlin/com/copypaste/app/AndroidCaptureChannel.kt:74](/Users/dmytro/Documents/CopyPaste/apps/copypaste_flutter/android/app/src/main/kotlin/com/copypaste/app/AndroidCaptureChannel.kt:74); [apps/copypaste_flutter/android/app/src/main/kotlin/com/copypaste/app/AndroidCaptureChannel.kt:104](/Users/dmytro/Documents/CopyPaste/apps/copypaste_flutter/android/app/src/main/kotlin/com/copypaste/app/AndroidCaptureChannel.kt:104); [apps/copypaste_flutter/android/app/src/main/kotlin/com/copypaste/app/ScreenshotCaptureState.kt:22](/Users/dmytro/Documents/CopyPaste/apps/copypaste_flutter/android/app/src/main/kotlin/com/copypaste/app/ScreenshotCaptureState.kt:22); [apps/copypaste_flutter/lib/features/settings/controller/settings_controller.dart:70](/Users/dmytro/Documents/CopyPaste/apps/copypaste_flutter/lib/features/settings/controller/settings_controller.dart:70); [apps/copypaste_flutter/lib/features/settings/controller/settings_controller.dart:134](/Users/dmytro/Documents/CopyPaste/apps/copypaste_flutter/lib/features/settings/controller/settings_controller.dart:134).

**Validation:** Code path verified; failure scenario not executed on a native device.

### F16 SMS enable can commit while monitor startup and rollback fail

**P2 · Android; conditional on two failures**

**Trigger:** Enable the SMS module successfully, fail Android monitor startup, then fail the compensating disable request.

**Actual behavior:** The registry remains enabled while background monitoring did not start. A subsequent refresh accurately shows enabled=true, but the original operation only reports a generic failure; module enablement and native monitoring health have diverged.

**Expected contract and verification:** Model enabled preference and monitor readiness separately, report partial completion, and recover the failed compensation. Test failed compensation in addition to the existing successful rollback test.

**Evidence:** [apps/copypaste_flutter/lib/features/modules/controller/modules_controller.dart:218](/Users/dmytro/Documents/CopyPaste/apps/copypaste_flutter/lib/features/modules/controller/modules_controller.dart:218); [crates/copypaste-modules/src/manager.rs:303](/Users/dmytro/Documents/CopyPaste/crates/copypaste-modules/src/manager.rs:303); [apps/copypaste_flutter/android/app/src/main/kotlin/com/copypaste/app/SmsModuleService.kt:28](/Users/dmytro/Documents/CopyPaste/apps/copypaste_flutter/android/app/src/main/kotlin/com/copypaste/app/SmsModuleService.kt:28); [apps/copypaste_flutter/test/features/modules/sms_module_access_test.dart:525](/Users/dmytro/Documents/CopyPaste/apps/copypaste_flutter/test/features/modules/sms_module_access_test.dart:525).

**Validation:** Code path verified; failure scenario not executed on a native device.

### F17 Revocation does not terminate an authenticated session

**P2 · P2P on macOS, Android, Windows**

**Trigger:** Revoke or unpair a device after its session has authenticated but before its transfer finishes.

**Actual behavior:** The durable pairing and future handshake credentials are removed, but the existing session uses its snapshot and has no per-pair cancellation. It can continue until completion or timeout. This is a bounded in-flight window, not permanent restoration of the pairing.

**Expected contract and verification:** Cancel/drain active sessions before acknowledging revocation, or make the UI explicit that an already-started transfer may finish. Test both inbound and outbound revocation during transfer.

**Evidence:** [crates/copypaste-p2p/src/node/listen.rs:104](/Users/dmytro/Documents/CopyPaste/crates/copypaste-p2p/src/node/listen.rs:104); [crates/copypaste-p2p/src/node/listen.rs:175](/Users/dmytro/Documents/CopyPaste/crates/copypaste-p2p/src/node/listen.rs:175); [crates/copypaste-p2p/src/node/mod.rs:822](/Users/dmytro/Documents/CopyPaste/crates/copypaste-p2p/src/node/mod.rs:822); [crates/copypaste-p2p/src/node/dial.rs:33](/Users/dmytro/Documents/CopyPaste/crates/copypaste-p2p/src/node/dial.rs:33); [apps/copypaste_flutter/lib/features/devices/devices_screen.dart:647](/Users/dmytro/Documents/CopyPaste/apps/copypaste_flutter/lib/features/devices/devices_screen.dart:647).

**Validation:** Code path verified; failure scenario not executed on a native device.

### F18 Staged plaintext loses its cleanup owner after an early restart

**P2 · macOS; shared Unix staging also used by Linux**

**Trigger:** Receive a file with Instant Clipboard enabled so it is materialized locally. Quit before its ten-minute deadline, restart before expiry, and perform no further staged file writes.

**Actual behavior:** The shutdown worker stops without deleting fresh files. Startup skips fresh entries and only creates a future cleanup worker when the immediate sweep reports unfinished expired work. No worker remains to remove the retained plaintext when it later reaches its deadline. Files remain owner-only.

**Expected contract and verification:** Resume cleanup ownership for retained unexpired files on startup. Test restart before expiry and advance time without another file paste.

**Evidence:** [crates/copypaste-core/src/sync/instant_clipboard.rs:79](/Users/dmytro/Documents/CopyPaste/crates/copypaste-core/src/sync/instant_clipboard.rs:79); [crates/copypaste-daemon/src/clipboard/file_materialize/mod.rs:64](/Users/dmytro/Documents/CopyPaste/crates/copypaste-daemon/src/clipboard/file_materialize/mod.rs:64); [crates/copypaste-daemon/src/clipboard/file_materialize/worker.rs:148](/Users/dmytro/Documents/CopyPaste/crates/copypaste-daemon/src/clipboard/file_materialize/worker.rs:148); [crates/copypaste-daemon/src/clipboard/file_materialize/sweep.rs:55](/Users/dmytro/Documents/CopyPaste/crates/copypaste-daemon/src/clipboard/file_materialize/sweep.rs:55).

**Validation:** Code path verified; failure scenario not executed on a native device.

### F19 A locked staged file can block Windows runtime startup

**P2 · Windows; depends on the other application denying deletion**

**Trigger:** Another application holds a previously materialized clipboard file open without delete sharing. Restart CopyPaste while that handle is retained.

**Actual behavior:** Staging initialization removes the old directory synchronously and propagates removal failure through WindowsClipboard construction to daemon main, before IPC/capture is ready. Optional cache cleanup becomes total runtime unavailability.

**Expected contract and verification:** Preserve the cleanup obligation while allowing a bounded recoverable staging state, instead of making a locked leftover abort the entire runtime. Validate using a real Windows deny-delete handle.

**Evidence:** [crates/copypaste-daemon/src/clipboard/windows/staging.rs:41](/Users/dmytro/Documents/CopyPaste/crates/copypaste-daemon/src/clipboard/windows/staging.rs:41); [crates/copypaste-daemon/src/clipboard/windows.rs:64](/Users/dmytro/Documents/CopyPaste/crates/copypaste-daemon/src/clipboard/windows.rs:64); [crates/copypaste-daemon/src/main.rs:129](/Users/dmytro/Documents/CopyPaste/crates/copypaste-daemon/src/main.rs:129).

**Validation:** Failure propagation verified in source. The native locked-handle scenario was not executed on Windows.

### F20 A public clipboard label can suppress another app capture

**P3 · Android; uncommon label collision**

**Trigger:** Another application publishes a ClipData item with the description label CopyPaste.

**Actual behavior:** The reader treats that public label as proof of a self-write and silently ignores the new item. The label does not identify the writer or prove it matches the app's previous output.

**Expected contract and verification:** Bind self-write suppression to owned write state/identity rather than a fixed public display label. Test a foreign item using the same label.

**Evidence:** [apps/copypaste_flutter/android/app/src/main/kotlin/com/copypaste/app/AndroidClipboardReader.kt:253](/Users/dmytro/Documents/CopyPaste/apps/copypaste_flutter/android/app/src/main/kotlin/com/copypaste/app/AndroidClipboardReader.kt:253); [apps/copypaste_flutter/android/app/src/main/kotlin/com/copypaste/app/AndroidClipboardWriter.kt:13](/Users/dmytro/Documents/CopyPaste/apps/copypaste_flutter/android/app/src/main/kotlin/com/copypaste/app/AndroidClipboardWriter.kt:13).

**Validation:** Code path verified; failure scenario not executed on a native device.

### F21 Origin lookup failure mislabels a remote clip as local

**P3 · macOS and Windows daemon; Android already preserves origin identity**

**Trigger:** Read a remote row successfully, then fail the separate device name/class database lookup. A further UI consequence requires its remote path to happen to exist on this device.

**Actual behavior:** The wire adapter replaces the whole origin with this device, even though row.origin_device_id is available. It can also mark the source file available locally and suppress Download. Missing labels alone do not trigger this; an actual lookup error is required.

**Expected contract and verification:** Preserve the authoritative device ID and degrade only its name/class to unknown. Test name/class lookup failure independently from missing origin.

**Evidence:** [crates/copypaste-daemon/src/meta/mod.rs:130](/Users/dmytro/Documents/CopyPaste/crates/copypaste-daemon/src/meta/mod.rs:130); [crates/copypaste-daemon/src/server/items/wire.rs:73](/Users/dmytro/Documents/CopyPaste/crates/copypaste-daemon/src/server/items/wire.rs:73); [crates/copypaste-daemon/src/server/items/wire.rs:222](/Users/dmytro/Documents/CopyPaste/crates/copypaste-daemon/src/server/items/wire.rs:222); [crates/copypaste-daemon/src/server/items/wire.rs:131](/Users/dmytro/Documents/CopyPaste/crates/copypaste-daemon/src/server/items/wire.rs:131); [apps/copypaste_flutter/lib/features/history/controller/history_controller.dart:384](/Users/dmytro/Documents/CopyPaste/apps/copypaste_flutter/lib/features/history/controller/history_controller.dart:384).

**Validation:** Code path verified; failure scenario not executed on a native device.

## Intentional behavior that may still surprise users

These are product decisions or documented limitations, not additional implementation-defect claims.

### R01 Secret text is included in plaintext export

Export text history includes secret text without a separate warning or inclusion choice. Export filters by type and decryption success, not secrecy. An explicit export can legitimately include all user content, but users may infer stronger protection from the secret label. Decide whether to exclude secret text by default or explicitly disclose its inclusion. Encrypted backup is a different operation.

[crates/copypaste-core/src/transfer/export.rs:28](/Users/dmytro/Documents/CopyPaste/crates/copypaste-core/src/transfer/export.rs:28).

### R02 Manual file Copy and Instant Clipboard produce different content

Manual Copy of a captured/synced file with source_reference writes the original device path or content URI as text, on all three platforms. Instant Clipboard deliberately clears the foreign reference and materializes stored bytes. For example, manual Copy on Windows can produce a macOS path instead of a file. This is documented; make the action explicit as Copy reference or offer a separate Copy file action.

[crates/copypaste-daemon/src/clipboard/mod.rs:290](/Users/dmytro/Documents/CopyPaste/crates/copypaste-daemon/src/clipboard/mod.rs:290); [crates/copypaste-flutter-bridge/src/runtime_android.rs:671](/Users/dmytro/Documents/CopyPaste/crates/copypaste-flutter-bridge/src/runtime_android.rs:671); [crates/copypaste-core/src/sync/instant_clipboard.rs:79](/Users/dmytro/Documents/CopyPaste/crates/copypaste-core/src/sync/instant_clipboard.rs:79); [crates/copypaste-ipc/src/payload/mod.rs:136](/Users/dmytro/Documents/CopyPaste/crates/copypaste-ipc/src/payload/mod.rs:136).

### R03 Auto Paste failure closes the popup after copying

When native auto-paste is denied or fails, Quick Paste deliberately leaves the completed clipboard copy and closes. The target may receive nothing until the user manually pastes. ADR 0033 and permission UI describe this fallback. It is distinct from F09, where the clipboard copy itself failed and no error appears.

[docs/adr/0033-quick-paste-popup.md:25](/Users/dmytro/Documents/CopyPaste/docs/adr/0033-quick-paste-popup.md:25); [apps/copypaste_flutter/lib/features/quick_paste/quick_paste_controller.dart:321](/Users/dmytro/Documents/CopyPaste/apps/copypaste_flutter/lib/features/quick_paste/quick_paste_controller.dart:321).

### R04 Windows updater can fail after the app has closed

The app reports that installation started and exits after launching its helper. The helper can later fail waiting for exit, re-verifying, or starting the installer, with no UI remaining to report the result. ADR 0020 already calls out the unqualified handoff. Treat this as a known recovery gap and retain a diagnosable next-launch result.

[apps/copypaste_flutter/windows/runner/app_update_channel.cpp:157](/Users/dmytro/Documents/CopyPaste/apps/copypaste_flutter/windows/runner/app_update_channel.cpp:157); [apps/copypaste_flutter/windows/runner/app_update_helper.cpp:61](/Users/dmytro/Documents/CopyPaste/apps/copypaste_flutter/windows/runner/app_update_helper.cpp:61); [docs/adr/0020-windows-distribution-and-update-signing.md:42](/Users/dmytro/Documents/CopyPaste/docs/adr/0020-windows-distribution-and-update-signing.md:42).

### R05 Repairing malformed legacy metadata can discard optional fields

The explicit secrecy action defaults a strictly unparseable metadata envelope, then restores valid privacy hints independently. A text/image row may therefore lose otherwise recoverable optional legacy attributes while being canonicalized. A file row requiring its missing file metadata fails atomically instead. This is limited to pre-existing legacy/corrupt metadata; it is not evidence that the current code loses an explicit secrecy flag. Consider a repair notice or field-level preservation test.

[crates/copypaste-core/src/storage/privacy.rs:29](/Users/dmytro/Documents/CopyPaste/crates/copypaste-core/src/storage/privacy.rs:29); [crates/copypaste-payload/src/lib.rs:176](/Users/dmytro/Documents/CopyPaste/crates/copypaste-payload/src/lib.rs:176); [crates/copypaste-core/tests/clipboard_privacy.rs:81](/Users/dmytro/Documents/CopyPaste/crates/copypaste-core/tests/clipboard_privacy.rs:81).

## Unresolved native scenarios

- **U01 Notification permission revocation:** active Android capture paths do not consistently recheck notification permission. Whether revocation leaves a running foreground service collecting without the expected notification needs device verification and a clear product contract. Notification permission is not itself clipboard/media permission. This was not counted as a confirmed defect.
- **U02 Newer local clipboard outside capture policy:** InstantClipboard records local recency after accepted captures and manual History copies. An excluded or confidential local copy may not advance that stamp; an older remote item arriving later may overwrite it. A native integration test must establish the complete ordering and intended pause/exclusion semantics before filing it as confirmed. Relevant paths: `crates/copypaste-core/src/sync/instant_clipboard.rs:15-103`, `crates/copypaste-daemon/src/state.rs:179-184`.

## Rejected suspicions and the original metadata issue

- **Sync Off does block inbound history exchange.** Although the listener can authenticate a connection, StoreSource checks the live setting before summaries, fetch and apply operations. The initial listener-only suspicion was rejected after independent review.
- **Pairing screenshots follow the current policy.** ADR 0015 allows QR/SAS capture when screenshot blocking is off; the ordinary route is not a bypass of the current contract.
- **Hiding both taskbar and tray icons is intentional.** Existing tests cover the choice, and platform launch/reopen handlers restore the window.
- **Screenshot default-on and global pause coupling are intentional.** They are separate from the permission-interval backlog in F05.
- **Optional module update preparation failures are intentionally skipped** under ADR 0034; this was not filed as a false-success defect.
- **The reported metadata-error-to-secret case appears addressed in the current dirty tree.** The inspected ingest path no longer invents secrecy from unreadable optional metadata; valid privacy hints and explicit overrides have dedicated tests. This is a source-level observation, not release qualification or proof that every metadata path is correct. F07 and F21 retain closely related error-to-semantic-value patterns elsewhere.

## Coverage and evidence limits

Ten agents reviewed bounded areas, with an independent pass used to challenge high-risk findings. Coverage included shared ingest/privacy/dedup; storage, retention, pinning, deletion, migrations and transfer; P2P/cloud and device pairing; History/Quick Paste; settings/onboarding/lifecycle; modules/OCR/semantic search/SMS; Android native integration; macOS/Windows clipboard and staging; and app/module update flows. Root verification traced selected end-to-end paths and reproduced F01/F08.

This was a source and controller-test audit. No user browser, Computer Use, physical device, emulator, production clipboard or account was operated. A full native fault-injection matrix was not run, and this report cannot establish that every possible surprising behavior has been found. Windows lock behavior and the Android service/permission scenarios particularly need platform reproduction before fixes are qualified. Existing tests were read as evidence of intent, not counted as executed tests.

The initial graph covered only an older History subset, so it was used for orientation; accepted findings rely on current source. The working tree was already dirty. A hash comparison found an independent change to `apps/copypaste_flutter/lib/main.dart` during the audit; no retained finding depends on the rejected startup candidate in that file. All other 708 files in the tracked audit baseline retained their content hashes. Existing production changes and the independent commit were preserved.

### Executed reproductions

A temporary Flutter test used the real HistoryController and a controlled HistoryRepository. Both assertions reproduced the current undesirable behavior:

1. Closing a watch stream and reinitializing does not subscribe again; later item events do not refresh history.
2. An items-change event replacing a list clip with secret=true leaves the selected detail object non-secret with its original plaintext.

The successful command was run from `apps/copypaste_flutter` with the Rust toolchain bin directory added to PATH:

```sh
PATH=/Users/dmytro/.cargo/bin:$PATH flutter test --no-pub test/__audit_watch_repro_test.dart --reporter expanded
```

Result: **2 tests passed**, where each test asserted the observed defect rather than desired corrected behavior. Initial attempts stopped in Flutter/native-asset setup before test execution; dependency resolution and PATH correction allowed the final run. The temporary repository test was removed after execution. Its source remains at `/tmp/copypaste-behavior-audit/watch_repro_test.dart` for this session.

During the audit phase, no production fixes, commits, releases or migrations were performed. The implementation phase is recorded below.

## Recommended order

Address F01 through F06 first, beginning with stale secret presentation and destructive SMS fallback. Then propagate explicit unavailable/partial states through the shared bridge and controller contracts (F07 through F16). Validate revocation and platform cleanup behavior on native targets. Keep product-policy changes R01 through R05 explicit instead of silently redefining what Secret, Copy, Auto Paste or successful update handoff means.

## Implementation record — 16 October 2026

The authorized scope was 20 corrections, excluding F09. No new permission prompts, confirmation dialogs or product admission policies were added. Intentional policies R01–R05 remain unchanged; U01 and U02 were not expanded into speculative changes.

| ID | Corrected behavior |
| --- | --- |
| F01 | List invalidations refresh selected details, and detail generations prevent stale public responses from overwriting newer secret state. Suspension restores the current selection; ordinary refresh and Quick Paste reopening remain nonblocking. |
| F02 | Capture deduplication includes content type. Binary capture IDs distinguish representations while existing IDs remain readable; migration 004 updates the unique index. |
| F03 | SMS handler lookup distinguishes unavailable from disabled and preserves monitoring preferences and cursors on lookup errors. |
| F04 | SMS acknowledgement follows successful ingestion or an intentional terminal decision. Failed ingestion retains the cursor for the next existing scan. |
| F05 | Screenshot access transitions preserve a permission boundary, excluding the observed denied interval after access returns. |
| F06 | Sign out clears canonical credential keys when the provider is unavailable. Available-provider failures still propagate; shared account-state keys do not introduce a cloud transport dependency into the runtime. |
| F07 | Settings health reaches the existing nonblocking Settings alert, so protective fallback values are not presented as ordinary saved choices. |
| F08 | History reconnects failed or ended watches while active, refreshes missed changes, and cancels recovery on suspension or disposal. |
| F09 | Unchanged by explicit user request: Quick Paste action-error presentation. |
| F10 | Text export reports unreadable text omissions through the existing toast. Ordinary non-text exclusion alone does not produce a warning. |
| F11 | A partial import still reports its error, but committed rows reset sync floors and notify sync subscribers. |
| F12 | Semantic model health verifies resource hashes and reports corrupt resources as unavailable. Cached health avoids rehashing unchanged large files on every query. |
| F13 | Module removal durably records cleanup debt before derived-state cleanup. Completed removal is not reported as a new failure; later listing finishes pending cleanup. |
| F14 | Refused Android foreground-capture setup restores the prior preference instead of enabling capture on a later resume. |
| F15 | Capture and screenshot failures reconcile the displayed state with native state and retain the original operation error. |
| F16 | Failed SMS startup and failed compensation reconcile the module's actual persisted state and report enabled, disabled or unknown accurately. |
| F17 | Revocation cancels active authenticated exchanges and drains already-running synchronous storage work before acknowledging success. Subsequent batches are rejected. |
| F18 | Unix staging restores cleanup ownership and the remaining expiry deadline after restart without extending plaintext lifetime. |
| F19 | A locked Windows staged file starts cleanup recovery instead of preventing runtime startup. New plaintext materialization waits for safe cleanup; unrelated clipboard capabilities remain available. |
| F20 | Android self-copy detection uses an opaque ownership receipt as well as the public label. Receipt persistence is best effort and cannot block copying or foreign capture. |
| F21 | Origin lookup failures preserve the row's authoritative device identity rather than falsely presenting remote content as local. |

### Verification and limits

Regression coverage includes representation collisions, SMS failure and acknowledgement ordering, clipboard ownership persistence failures, screenshot permission transitions, cloud credential cleanup, corrupt models, durable module cleanup, partial import sync, revocation during a blocked source batch, staged-file recovery, origin lookup failure, History reconnection and stale privacy responses.

- `cargo test --workspace --locked --no-fail-fast`: passed, 1,770 tests; 17 existing ignored tests remain ignored.
- `PATH=/Users/dmytro/.cargo/bin:$PATH flutter test --no-pub --reporter expanded`: passed, 1,013 tests, including the existing suspend/resume and Quick Paste reopening contracts.
- `flutter analyze --no-pub`: passed with no issues.
- `JAVA_HOME=/opt/homebrew/opt/openjdk@25/libexec/openjdk.jdk/Contents/Home PATH=/Users/dmytro/.cargo/bin:/opt/homebrew/opt/openjdk@25/bin:$PATH ./gradlew :app:testDebugUnitTest --console=plain` from the Android project: passed in the final serialized run after Flutter, 82 tests across 14 suites, no failures, errors or skips; `BUILD SUCCESSFUL`.
- Modified Dart and Rust files pass their format checks; `git diff --check` passes.

The Windows locked-file regression now holds a real file handle denying deletion, but Windows execution is unavailable on this macOS host. Android tests use local JVM/Robolectric coverage, not a physical device or emulator. A permission revoke-and-regrant that is entirely unobserved while the Android process is absent cannot be inferred; the fix does not reset every launch and discard allowed screenshots. No production account, user database, browser or clipboard was operated. No commits or releases were made. Migration 004 was registered and exercised in tests, not applied to the user's production database.
