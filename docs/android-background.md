# Android Background Survival — Permissions, Mechanisms, and User Setup

## What the App Uses

| Mechanism | Purpose | Files |
|-----------|---------|-------|
| `FOREGROUND_SERVICE_SPECIAL_USE` foreground service | Keeps clipboard monitoring alive; holds process in "foreground" state | `ClipboardService.kt`, `AndroidManifest.xml` |
| `ClipboardAccessibilityService` | Grants `getPrimaryClip()` access in background on Android 10+ | `ClipboardAccessibilityService.kt`, `res/xml/accessibility_service_config.xml` |
| `BootReceiver` | Restarts the FGS after cold/warm/quick boot | `BootReceiver.kt`, `AndroidManifest.xml` |
| `ServiceRestartWorker` | Restarts the FGS via an expedited WorkManager job after swipe-away | `ServiceRestartWorker.kt` |
| `REQUEST_IGNORE_BATTERY_OPTIMIZATIONS` | Exempts the app from Doze-related process killing | `AndroidManifest.xml`, `OnboardingActivity.kt` |
| `OemAutoStartHelper` | Deep-links to OEM-specific autostart/protected-apps screens | `OemAutoStartHelper.kt`, `OnboardingActivity.kt` |
| `FgsSyncLoop` (60-sec poll inside FGS) | Near-real-time incoming Supabase sync while FGS is alive | `FgsSyncLoop.kt`, `ClipboardService.kt` |
| `SupabasePollWorker` (15-min WorkManager) | Catch-up sync when the process was dead | `SupabasePollWorker.kt` |

## Architecture Decision: Sync Strategy

**FGS long-poll loop (60 s) + WorkManager fallback (15 min)**

Supabase Realtime (WebSocket) was considered but rejected: the Android app uses
`HttpURLConnection` with no OkHttp or `supabase-kt` dependency. Implementing a
Doze-safe WebSocket from scratch would add ~400 lines of RFC-6455 code for a
~60-second latency benefit. The pragmatic approach:

- While the FGS is alive: `FgsSyncLoop` polls every 60 seconds — near-real-time for clipboard use.
- While the process is dead (OEM kill, deep Doze): `SupabasePollWorker` fires every 15 minutes via WorkManager.
- Battery cost of the FGS loop: one HTTPS GET/min ≈ negligible (< 1 mAh/h on LTE).
- Deep Doze (screen off + stationary for > 1 h) defers the FGS loop, but the user is not
  actively switching devices at that point, so 15-min WorkManager catches up.

## Android 10+ Background Clipboard: Honest Limits

`ClipboardManager.getPrimaryClip()` is blocked from any background context on Android 10+
(API 29+) **unless** the calling process is:
1. The current foreground app (has window focus), or
2. The default IME, or
3. An enabled `AccessibilityService`.

CopyPaste uses route 3 (`ClipboardAccessibilityService`). This **works on stock AOSP** and
on most OEM ROMs. Known exceptions:

- **MIUI (Xiaomi)**: Some MIUI versions add an extra restriction that blocks clipboard reads
  even from an enabled AccessibilityService. The user may need to grant "Clipboard access"
  permission separately under MIUI's App Permissions. No workaround available without root.
- **Android 12+ toast**: The system shows a one-time "App accessed your clipboard" toast on
  first use per session. This is expected and cannot be suppressed.
- `OnPrimaryClipChangedListener` fires (as notification) even in background on all APIs — but
  the actual content is only available if `getPrimaryClip()` is non-null. The AccessibilityService
  binding is what makes it non-null.

## Steps the User Must Do Manually

These cannot be granted programmatically and require user action:

### Required (without these, background clipboard capture does not work)
1. **Enable the Accessibility Service**
   - `Settings > Accessibility > Installed services > CopyPaste > Enable`
   - OnboardingActivity opens `ACTION_ACCESSIBILITY_SETTINGS` and explains why.

### Strongly Recommended (without these, OEM killers will kill the service)
2. **Grant Battery Optimization Exemption**
   - OnboardingActivity shows a "Request Exemption" button that opens
     `ACTION_REQUEST_IGNORE_BATTERY_OPTIMIZATIONS` directly.
   - On some OEMs this dialog is absent; the global battery settings list is the fallback.

3. **Enable Autostart in OEM Settings** (required on Xiaomi, Huawei, Oppo, Vivo, Samsung, etc.)
   - OnboardingActivity shows an "Open OEM Settings" card (visible only on recognized OEMs).
   - OEM-specific paths:
     - **Xiaomi/MIUI**: Security > Permissions > Auto-start > CopyPaste = ON
     - **Huawei/EMUI**: Settings > Battery > App launch > CopyPaste > Manage manually > Allow all
     - **Oppo/ColorOS**: Settings > Battery > Power saving > App battery management or
       Security > Privacy Permissions > Startup manager > CopyPaste = ON
     - **Vivo/FuntouchOS**: Settings > Battery > Background app management or
       iQOO Security > Phone boost > Whitelist
     - **Samsung/One UI**: Settings > Device Care > Battery > Background usage limits >
       remove CopyPaste from "Sleeping apps" or "Deep sleeping apps"
     - **OnePlus/OxygenOS**: Settings > Battery > Background app management > CopyPaste > Allow
     - **Asus/ZenUI**: Mobile Manager > Autostart > CopyPaste = ON
   - If the OEM screen isn't found by OemAutoStartHelper, the fallback opens the
     standard battery optimisation screen.

## What Requires a Real Device / OEM to Verify

- OEM component intents in `OemAutoStartHelper` — component paths change across ROM versions.
  Verified to compile and resolve correctly at runtime, but actual navigation to the settings
  screen must be tested on a physical device per manufacturer.
- MIUI clipboard access restrictions — behaviour varies by MIUI version (12–15).
- `BootReceiver` QUICKBOOT_POWERON variants — HTC and Xiaomi fast-boot must be tested on hardware.
- `ServiceRestartWorker` expedited execution timing on devices with low memory or Doze active.
- Samsung "App power management" auto-adds recently-unused apps to the sleeping list — this is
  a user-side action that overrides our battery exemption; users must remove the app from the list.

## Toolchain Requirements (JDK)

Gradle 8.7 bundles Kotlin 1.9.22's embedded compiler for its Kotlin DSL support
(`settings.gradle.kts`, `build.gradle.kts`). That compiler's `JavaVersion.parse()`
cannot handle JDK 22+ version strings and crashes with
`java.lang.IllegalArgumentException: <version>` while bootstrapping — before any
of our own build-script code runs. Recognise this symptom: a bare
`IllegalArgumentException` naming a JDK version, thrown from
`org.jetbrains.kotlin.com.intellij.util.lang.JavaVersion.parse`.

- **Supported/pinned JDK**: 17-21 (Temurin 17 recommended). On this dev machine
  it is installed at `/Library/Java/JavaVirtualMachines/temurin-17.jdk`.
- Both `android/gradlew` (direct invocation) and `scripts/android-verify.sh`
  (scripted path) now fail fast with an actionable message when an unsupported
  JDK is active, instead of surfacing the bare `IllegalArgumentException`.
- To select a supported JDK on macOS: `export JAVA_HOME=$(/usr/libexec/java_home -v 17)`.
