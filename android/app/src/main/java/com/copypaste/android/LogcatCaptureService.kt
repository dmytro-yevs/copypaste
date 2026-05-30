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
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch

/**
 * Optional power-user background capture path via `logcat`.
 *
 * ## Why this exists
 * [ClipboardAccessibilityService] is the PRIMARY background capture mechanism on
 * Android 10+. However, some users cannot or will not enable an AccessibilityService.
 * On those devices, clipboard content copied in background is never captured.
 *
 * When `android.permission.READ_LOGS` is granted to the app via adb, the process
 * can read the system logcat stream. The Android ClipboardManager logs a message
 * whenever the primary clip changes:
 *
 *   D ClipboardService: Setting primary clip <PrimaryClipDescription ...>
 *
 * We watch this tag + prefix and, when a clip-change event is detected, attempt to
 * read the clipboard via the normal ClipboardManager path.
 *
 * ## Limitations (important — reflected in Settings UI)
 * - `READ_LOGS` is a signature-level permission that CANNOT be granted by the user
 *   via the standard permission dialog. It must be granted over adb:
 *     `adb shell pm grant com.copypaste.android android.permission.READ_LOGS`
 * - The logcat-based detection may NOT work on Android 11+ where logcat output is
 *   scoped to the app's own process (AOSP hardened). On stock Android 11+ the
 *   ClipboardManager log line is emitted by the SYSTEM process and will not appear
 *   in our filtered stream. Samsung/MIUI ROMs vary.
 * - Even when the detection fires, [android.content.ClipboardManager.getPrimaryClip]
 *   on API 29+ may return null from a background context without an enabled
 *   accessibility service (READ_LOGS does not override this restriction on stock ROMs).
 * - This path DOES NOT replace [ClipboardAccessibilityService]. It is a best-effort
 *   fallback for power users who have granted READ_LOGS and understand the limitations.
 *
 * ## How to enable
 * 1. Grant via adb:
 *      adb shell pm grant com.copypaste.android android.permission.READ_LOGS
 * 2. Enable the toggle in Settings → Diagnostics → "adb logcat capture".
 * 3. The status indicator will update to reflect detected / not-detected state.
 *
 * The service is started/stopped by [syncState] based on the setting and the
 * permission check. It does NOT run unless both conditions are met.
 */
class LogcatCaptureService : Service() {

    private val scope = CoroutineScope(Dispatchers.IO)
    private lateinit var settings: Settings
    private lateinit var repository: ClipboardRepository
    private lateinit var syncManager: SyncManager
    private var logcatJob: Job? = null

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
     * Tails the logcat stream looking for ClipboardService "Setting primary clip"
     * events from the system process. When detected, reads the current clipboard.
     *
     * Started with `-T 1` (tail last 1 line) to avoid replaying old events,
     * and `-s ClipboardService:D` to filter to relevant tag only.
     *
     * On Android 11+ (scoped logcat / AOSP hardening) this tag may produce no
     * output; the loop exits cleanly and the status is updated to NOT_WORKING.
     */
    private suspend fun runLogcatLoop() {
        val process: Process = try {
            // `-T 1` = start from the last 1 line (avoids replaying old events on startup)
            // `-s ClipboardService:D` = only lines tagged ClipboardService at Debug level
            ProcessBuilder("logcat", "-T", "1", "-v", "brief", "-s", "ClipboardService:D")
                .redirectErrorStream(true)
                .start()
        } catch (e: Exception) {
            AppLogger.e(TAG, "Failed to start logcat process", e)
            return
        }

        AppLogger.i(TAG, "Logcat process started")

        try {
            process.inputStream.bufferedReader().use { reader ->
                var line: String?
                while (scope.isActive) {
                    line = reader.readLine() ?: break
                    if (CLIP_CHANGE_MARKER in line) {
                        AppLogger.d(TAG, "Clip-change event detected via logcat")
                        onClipChangedViaLogcat()
                    }
                }
            }
        } catch (e: Exception) {
            AppLogger.w(TAG, "Logcat reader terminated: ${e.message}")
        } finally {
            runCatching { process.destroy() }
            AppLogger.i(TAG, "Logcat process destroyed")
        }

        // Logcat stream ended — likely scoped on Android 11+ AOSP.
        AppLogger.w(
            TAG,
            "Logcat stream ended — READ_LOGS path may not be supported on this " +
                "Android build (scoped logcat, API 30+ AOSP hardening). " +
                "Enable ClipboardAccessibilityService for reliable background capture."
        )
        settings.logcatCaptureWorking = false
        stopSelf()
    }

    /**
     * Called when the logcat listener detected a clip-change marker.
     * Reads the current clipboard on the main thread and routes through the
     * shared capture pipeline (same dedup, size limits, encryption as A11y path).
     */
    private fun onClipChangedViaLogcat() {
        Handler(Looper.getMainLooper()).post {
            val cm = getSystemService(Context.CLIPBOARD_SERVICE) as android.content.ClipboardManager
            val clip = cm.primaryClip ?: run {
                AppLogger.d(
                    TAG,
                    "getPrimaryClip returned null — API 29+ background restriction applies " +
                        "even with READ_LOGS (no AccessibilityService binding)"
                )
                // Detection fired but read is still blocked — mark as not-working.
                settings.logcatCaptureWorking = false
                return@post
            }

            // Image clips: skip — logcat path is text-only.
            val imageMime = (0 until clip.description.mimeTypeCount)
                .map { clip.description.getMimeType(it) }
                .firstOrNull { it.startsWith("image/") }
            if (imageMime != null) {
                AppLogger.d(TAG, "Image clip via logcat — skipping (text-only path)")
                return@post
            }

            val text = clip.getItemAt(0)?.text?.toString()
            if (text.isNullOrBlank()) return@post

            // At least one successful read — mark working.
            settings.logcatCaptureWorking = true

            scope.launch {
                ClipboardService.captureClip(
                    this@LogcatCaptureService,
                    text,
                    settings,
                    repository,
                    syncManager,
                )
                AppLogger.d(TAG, "logcat-captured clip stored via shared pipeline")
            }
        }
    }

    companion object {
        private const val TAG = "LogcatCaptureService"

        /**
         * The logcat marker emitted by android/os/ClipboardService.java in AOSP
         * when setPrimaryClip is called. Present on Android 9–10; may be absent
         * or scoped-out on Android 11+ AOSP (system process log isolation).
         */
        private const val CLIP_CHANGE_MARKER = "Setting primary clip"

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

        /** Start or stop the service based on current settings + permission state. */
        fun syncState(context: Context, settings: Settings) {
            val shouldRun = hasReadLogsPermission(context) && settings.logcatCaptureEnabled
            val intent = Intent(context, LogcatCaptureService::class.java)
            if (shouldRun) {
                context.startService(intent)
            } else {
                context.stopService(intent)
            }
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
     * Likely cause: Android 11+ AOSP scoped logcat, or API 29+ clipboard
     * background restriction still blocking getPrimaryClip.
     */
    GRANTED_NOT_WORKING,
    /** Granted, enabled, and at least one clip was read successfully. */
    WORKING,
}
