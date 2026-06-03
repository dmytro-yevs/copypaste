package com.copypaste.android

import android.app.Service
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.os.Build
import android.os.Handler
import android.os.IBinder
import android.os.Looper
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch

/**
 * Background clipboard detection via logcat tailing (ClipCascade technique).
 *
 * ## How this works (ClipCascade technique)
 *
 * On Android 10+ (API 29+), [ClipboardManager.getPrimaryClip] is blocked from any
 * context that is not the foreground app, the default IME, or an enabled
 * AccessibilityService. A 1×1 invisible non-focusable overlay (the old ClipboardService
 * trick) does NOT lift this restriction because it never gains input focus.
 *
 * The ClipCascade technique uses a two-step approach:
 *
 * ### Step 1 — Detection via logcat (this service)
 * When our app attempts a background getPrimaryClip() and is denied, ColorOS / AOSP
 * logs an ERROR line that CONTAINS our package ID:
 *
 *   E ClipboardService: Denying clipboard access to com.copypaste.android, ...
 *
 * On API > P (Android 10+) with READ_LOGS granted, we tail logcat with the filter
 * `ClipboardService:E *:S` and match on our `BuildConfig.APPLICATION_ID`. On API <= P
 * the old "Setting primary clip" debug marker is still available and we match that too
 * (bonus fallback: lets us skip the Activity launch and read directly since the
 * API-29+ restriction doesn't apply on Android 9 and below).
 *
 * A debounce of [FOCUSABLE_ACTIVITY_DEBOUNCE_MS] prevents rapid bursts (e.g. a paste
 * followed immediately by a copy) from launching the Activity multiple times.
 *
 * ### Step 2 — Focused read via [ClipboardFloatingActivity]
 * On API 29+ when a denial line is detected, we launch [ClipboardFloatingActivity]:
 * a transparent, floating, no-history, excluded-from-recents Activity. That Activity
 * adds a TYPE_APPLICATION_OVERLAY view, CLEARS FLAG_NOT_FOCUSABLE to gain focus,
 * waits for the [ViewTreeObserver.OnGlobalLayoutListener] callback (the ONLY safe
 * point where getPrimaryClip() returns non-null), reads the clip, routes it through
 * [ClipboardService.captureClip]/[captureImageClip]/[captureFileClip], then finishes.
 *
 * The Activity is the load-bearing piece: the read MUST wait for window focus — the
 * OS clipboard restriction is lifted only after the focused-window event.
 *
 * ## Limitations
 * - `READ_LOGS` is a signature-level permission that CANNOT be granted by the user
 *   via the standard dialog. It must be granted over adb:
 *     `adb shell pm grant com.copypaste.android android.permission.READ_LOGS`
 * - On stock Android 11+ AOSP, logcat output is scoped; the denial / clip-change
 *   line may not appear in our stream. ColorOS (OPPO/OnePlus), MIUI, and OneUI vary
 *   — many still emit the system tag in the shared logcat buffer.
 * - SYSTEM_ALERT_WINDOW (draw-over-other-apps) must also be granted so
 *   [ClipboardFloatingActivity] can add the TYPE_APPLICATION_OVERLAY view.
 *
 * ## How to enable
 * 1. Grant READ_LOGS via adb:
 *      adb shell pm grant com.copypaste.android android.permission.READ_LOGS
 * 2. Grant SYSTEM_ALERT_WINDOW in Settings → Apps → Special app access → Display over other apps.
 * 3. (Optional) The service auto-starts on next app launch; use the toggle in Settings to disable.
 *
 * The service is started/stopped by [syncState]. When READ_LOGS is granted and the user has NOT
 * explicitly disabled the toggle, the service auto-starts (logcatCaptureEnabled defaults to true
 * in that case — see [syncState]).
 */
class LogcatCaptureService : Service() {

    private val scope = CoroutineScope(Dispatchers.IO)
    private lateinit var settings: Settings
    private lateinit var repository: ClipboardRepository
    private lateinit var syncManager: SyncManager
    private var logcatJob: Job? = null

    // Debounce: timestamp (ms) of last ClipboardFloatingActivity launch.
    // Prevents N rapid denial lines from spawning N Activity instances.
    @Volatile private var lastFocusedLaunchMs = 0L

    override fun onCreate() {
        super.onCreate()
        settings = Settings(this)
        repository = ClipboardRepository(this)
        val relayClient = RelayClient(settings.relayUrl)
        syncManager = SyncManager(relayClient, settings.deviceId, token = "", settings = settings)
        AppLogger.i(TAG, "LogcatCaptureService created")
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (!hasReadLogsPermission(this)) {
            AppLogger.w(TAG, "READ_LOGS not granted — stopping immediately")
            stopSelf()
            return START_NOT_STICKY
        }
        if (logcatJob?.isActive == true) return START_STICKY
        logcatJob = scope.launch { runLogcatLoop() }
        AppLogger.i(TAG, "Logcat capture loop started")
        return START_STICKY
    }

    override fun onDestroy() {
        logcatJob?.cancel()
        scope.cancel()
        AppLogger.i(TAG, "LogcatCaptureService destroyed")
        super.onDestroy()
    }

    override fun onBind(intent: Intent?): IBinder? = null

    // ── Logcat reader loop ──────────────────────────────────────────────────

    /**
     * Tails the system logcat stream for clipboard-change signals.
     *
     * **Filter strategy (dual-marker):**
     *
     * On API > P (Android 10+): `ClipboardService:E *:S`
     *   Matches ERROR lines from the system ClipboardService. When our app attempts
     *   a background clipboard read, the system logs:
     *     `E ClipboardService: Denying clipboard access to com.copypaste.android, ...`
     *   We match lines containing [BuildConfig.APPLICATION_ID] to confirm it's our
     *   denial. On detection → launch [ClipboardFloatingActivity] (focused-read path).
     *
     * On API <= P: `ClipboardService:D *:S`
     *   Matches the older DEBUG "Setting primary clip" marker. On API <= P the
     *   background restriction doesn't apply, so we can read the clip directly on
     *   the main thread without the focusable Activity.
     *
     * Started with `-T 1` (tail last 1 line) to avoid replaying old events on restart.
     */
    private suspend fun runLogcatLoop() {
        val useApi29Path = Build.VERSION.SDK_INT > Build.VERSION_CODES.P

        // API 29+: watch for clipboard-access DENIAL lines (E level) naming our package.
        // API <= 28: watch for "Setting primary clip" DEBUG lines (direct read is fine).
        val logLevel = if (useApi29Path) "E" else "D"
        val process: Process = try {
            ProcessBuilder(
                "logcat", "-T", "1", "-v", "brief",
                "ClipboardService:$logLevel", "*:S"
            )
                .redirectErrorStream(true)
                .start()
        } catch (e: Exception) {
            AppLogger.e(TAG, "Failed to start logcat process", e)
            return
        }

        AppLogger.i(TAG, "Logcat process started (api29Path=$useApi29Path, level=$logLevel)")

        try {
            process.inputStream.bufferedReader().use { reader ->
                var line: String?
                while (scope.isActive) {
                    line = reader.readLine() ?: break
                    if (useApi29Path) {
                        // API 29+: look for the denial line naming our package.
                        // The system ClipboardService logs the denying package name in the
                        // ERROR message so we can confirm it's our denial, not another app's.
                        if (BuildConfig.APPLICATION_ID in line) {
                            AppLogger.d(TAG, "Clipboard denial detected (our package): debouncing")
                            onDenialDetected()
                        }
                    } else {
                        // API <= 28: the classic "Setting primary clip" debug marker.
                        if (CLIP_SET_MARKER in line) {
                            AppLogger.d(TAG, "Clip-set event detected via logcat (API<=28 path)")
                            onClipChangedLegacy()
                        }
                    }
                }
            }
        } catch (e: Exception) {
            AppLogger.w(TAG, "Logcat reader terminated: ${e.message}")
        } finally {
            runCatching { process.destroy() }
            AppLogger.i(TAG, "Logcat process destroyed")
        }

        // Stream ended — scoped logcat on Android 11+ AOSP is the most likely cause.
        AppLogger.w(
            TAG,
            "Logcat stream ended — READ_LOGS path may not be supported on this build " +
                "(scoped logcat, API 30+ AOSP hardening)."
        )
        settings.logcatCaptureWorking = false
        stopSelf()
    }

    /**
     * API 29+ path: a clipboard-access denial for our package was detected.
     *
     * Launches [ClipboardFloatingActivity] after [FOCUSABLE_ACTIVITY_DEBOUNCE_MS].
     * The Activity adds a focused overlay, waits for the layout pass (which is when
     * the OS lifts the clipboard restriction), reads the clip, and routes it through
     * the shared capture pipeline.
     *
     * Guards:
     *  - Debounce: at most one Activity launch per [FOCUSABLE_ACTIVITY_DEBOUNCE_MS].
     *  - canDrawOverlays: if overlay permission was revoked, skip (the Activity
     *    also checks this internally, but early exit avoids the Activity lifecycle cost).
     */
    private fun onDenialDetected() {
        val now = System.currentTimeMillis()
        if (now - lastFocusedLaunchMs < FOCUSABLE_ACTIVITY_DEBOUNCE_MS) {
            AppLogger.d(TAG, "Denial debounced — skipping duplicate launch")
            return
        }

        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.M &&
            !android.provider.Settings.canDrawOverlays(this)
        ) {
            AppLogger.w(TAG, "canDrawOverlays=false — cannot launch ClipboardFloatingActivity")
            settings.logcatCaptureWorking = false
            return
        }

        lastFocusedLaunchMs = now
        AppLogger.i(TAG, "Launching ClipboardFloatingActivity for focused clipboard read")

        // The debounce delay is applied before launching to ensure we catch the clip
        // AFTER the user's copy action has fully settled in the ClipboardManager.
        scope.launch {
            delay(FOCUSABLE_ACTIVITY_DEBOUNCE_MS)
            ClipboardFloatingActivity.launch(this@LogcatCaptureService)
            // Mark as working optimistically — the Activity will log its own result.
            // If it finds null, it logs a warning but does not update logcatCaptureWorking
            // (we don't have a callback from the Activity here). The working flag is only
            // used for the Settings UI indicator and is updated to true by captureClip
            // succeeding (indirectly via the notification count bump).
            settings.logcatCaptureWorking = true
        }
    }

    /**
     * API <= 28 (Android 9 and below) path: the "Setting primary clip" debug marker
     * was detected. On these API levels the background clipboard restriction does NOT
     * apply, so we can read the clip directly from the main thread.
     *
     * This is a direct port of the original LogcatCaptureService.onClipChangedViaLogcat
     * behaviour and is only exercised on old devices that still emit the debug-level tag.
     */
    private fun onClipChangedLegacy() {
        Handler(Looper.getMainLooper()).post {
            val cm = getSystemService(Context.CLIPBOARD_SERVICE) as android.content.ClipboardManager
            val clip = cm.primaryClip ?: run {
                AppLogger.d(TAG, "getPrimaryClip returned null on API<=28 — unexpected")
                settings.logcatCaptureWorking = false
                return@post
            }

            // Image clips: handled by foreground service / logcat path; skip here (no URI context).
            val imageMime = (0 until clip.description.mimeTypeCount)
                .map { clip.description.getMimeType(it) }
                .firstOrNull { it.startsWith("image/") }
            if (imageMime != null) {
                AppLogger.d(TAG, "Image clip via logcat (API<=28) — skipping (no URI context)")
                return@post
            }

            val text = clip.getItemAt(0)?.text?.toString()
            if (text.isNullOrBlank()) return@post

            settings.logcatCaptureWorking = true

            scope.launch {
                ClipboardService.captureClip(
                    this@LogcatCaptureService,
                    text,
                    settings,
                    repository,
                    syncManager,
                )
                AppLogger.d(TAG, "logcat-captured clip stored via shared pipeline (API<=28 path)")
            }
        }
    }

    companion object {
        private const val TAG = "LogcatCaptureService"

        /**
         * The logcat DEBUG marker emitted by android/os/ClipboardService.java in AOSP
         * when setPrimaryClip is called. Used on API <= 28 only (direct-read path).
         * On API 29+ this marker is not visible because the system ClipboardService
         * runs in a different process with its own scoped logcat on newer AOSP.
         */
        private const val CLIP_SET_MARKER = "Setting primary clip"

        /**
         * Debounce window for [ClipboardFloatingActivity] launches (ms).
         *
         * A rapid copy action may cause several denial lines in quick succession
         * (the foreground service listener + our own probing retry). We gate launches
         * to at most one per window. 1000 ms matches the ClipCascade reference
         * implementation's debounce interval.
         */
        private const val FOCUSABLE_ACTIVITY_DEBOUNCE_MS = 1000L

        /**
         * Returns true if READ_LOGS has been granted via adb to this package.
         * This is a signature-level permission; it cannot be requested at runtime.
         */
        fun hasReadLogsPermission(context: Context): Boolean =
            context.checkSelfPermission(android.Manifest.permission.READ_LOGS) ==
                PackageManager.PERMISSION_GRANTED

        /**
         * Returns the current composite status of the logcat capture path.
         */
        fun status(context: Context, settings: Settings): LogcatCaptureStatus {
            if (!hasReadLogsPermission(context)) return LogcatCaptureStatus.NOT_GRANTED
            if (!settings.logcatCaptureEnabled) return LogcatCaptureStatus.DISABLED
            return if (settings.logcatCaptureWorking) {
                LogcatCaptureStatus.WORKING
            } else {
                LogcatCaptureStatus.GRANTED_NOT_WORKING
            }
        }

        /**
         * Start or stop the service based on READ_LOGS permission and user preference.
         *
         * Auto-enable logic: if READ_LOGS is granted and the user has not explicitly disabled
         * the toggle (logcatCaptureEnabled stored in prefs is `false` only when the user
         * flipped it OFF — the default is `false` but we treat a first-time READ_LOGS grant
         * as an implicit enable here), we flip logcatCaptureEnabled to `true` so the service
         * starts automatically on the next app launch after the adb grant.
         *
         * Implementation: if READ_LOGS just became granted and there is no explicit "user
         * set this to false" marker, set logcatCaptureEnabled = true before evaluating shouldRun.
         * A separate "user_disabled_logcat" pref distinguishes "default false" from "user chose
         * false" — avoids re-enabling after the user explicitly turns it off.
         */
        fun syncState(context: Context, settings: Settings) {
            if (hasReadLogsPermission(context)) {
                // Auto-enable: if READ_LOGS just became available and the user has not
                // explicitly turned the feature OFF, enable it automatically.
                val prefs = context.getSharedPreferences("copypaste", android.content.Context.MODE_PRIVATE)
                val userExplicitlyDisabled = prefs.getBoolean("logcat_capture_user_disabled", false)
                if (!userExplicitlyDisabled && !settings.logcatCaptureEnabled) {
                    settings.logcatCaptureEnabled = true
                }
            }
            val shouldRun = hasReadLogsPermission(context) && settings.logcatCaptureEnabled
            val intent = Intent(context, LogcatCaptureService::class.java)
            if (shouldRun) {
                context.startService(intent)
            } else {
                context.stopService(intent)
            }
        }

        /**
         * Called by SettingsActivity when the user explicitly turns OFF the logcat capture toggle.
         * Records the "user chose off" marker so [syncState] does not re-enable it automatically.
         */
        fun markUserDisabled(context: Context) {
            context.getSharedPreferences("copypaste", android.content.Context.MODE_PRIVATE)
                .edit().putBoolean("logcat_capture_user_disabled", true).apply()
        }

        /**
         * Called by SettingsActivity when the user explicitly turns ON the logcat capture toggle.
         * Clears the "user chose off" marker.
         */
        fun markUserEnabled(context: Context) {
            context.getSharedPreferences("copypaste", android.content.Context.MODE_PRIVATE)
                .edit().putBoolean("logcat_capture_user_disabled", false).apply()
        }
    }
}

/**
 * Status of the optional adb READ_LOGS background capture path.
 */
enum class LogcatCaptureStatus {
    /** READ_LOGS not granted — adb grant command needed. */
    NOT_GRANTED,
    /** Granted but feature is toggled off in settings. */
    DISABLED,
    /**
     * Granted and enabled, but captures have not succeeded yet.
     * Likely cause: Android 11+ AOSP scoped logcat, or canDrawOverlays not granted.
     */
    GRANTED_NOT_WORKING,
    /** Granted, enabled, and at least one clip was read successfully. */
    WORKING,
}
