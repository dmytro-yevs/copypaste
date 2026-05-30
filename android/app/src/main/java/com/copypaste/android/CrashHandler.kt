package com.copypaste.android

import android.content.Context
import android.os.Build
import java.io.File
import java.io.FileWriter
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

/**
 * Global uncaught-exception handler that persists full crash reports to
 * app-scoped external storage before the process dies.
 *
 * Install once in [CopyPasteApp.onCreate] — which runs before MainActivity,
 * so even a crash in MainActivity.onCreate is captured.
 *
 * Crash files land in the same directory as [AppLogger]:
 *   /sdcard/Android/data/com.copypaste.android/files/logs/crash_<timestamp>.txt
 *
 * Retrievable without root while the app is NOT running:
 *   adb pull /sdcard/Android/data/com.copypaste.android/files/logs/
 *
 * The previous default handler is chained so the system still shows its
 * crash dialog / ANR report.
 */
object CrashHandler : Thread.UncaughtExceptionHandler {

    private const val TAG = "CrashHandler"

    /**
     * Key stored in SharedPreferences to signal that the previous run crashed.
     * Read on next launch to optionally surface "crash detected — export logs?".
     */
    const val PREF_CRASHED_LAST_RUN = "crashed_last_run"
    private const val PREFS_NAME = "copypaste_crash"

    private var appContext: Context? = null
    private var previousHandler: Thread.UncaughtExceptionHandler? = null

    /**
     * Install this handler as the global uncaught exception handler.
     * Must be called from [CopyPasteApp.onCreate].
     */
    fun install(context: Context) {
        appContext = context.applicationContext
        previousHandler = Thread.getDefaultUncaughtExceptionHandler()
        Thread.setDefaultUncaughtExceptionHandler(this)
        AppLogger.d(TAG, "CrashHandler installed (previous=$previousHandler)")
    }

    /**
     * Returns true if the previous app run ended with an uncaught exception.
     * Clears the flag on read so the "crashed last time" prompt only appears once.
     */
    fun consumeCrashedLastRun(context: Context): Boolean {
        val prefs = context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
        val crashed = prefs.getBoolean(PREF_CRASHED_LAST_RUN, false)
        if (crashed) prefs.edit().remove(PREF_CRASHED_LAST_RUN).apply()
        return crashed
    }

    // ── Thread.UncaughtExceptionHandler ─────────────────────────────────────

    override fun uncaughtException(thread: Thread, throwable: Throwable) {
        try {
            writeCrashReport(thread, throwable)
        } catch (_: Exception) {
            // Writing failed — do not mask the original crash.
        }
        // Chain to the previous handler so the system crash dialog still appears.
        previousHandler?.uncaughtException(thread, throwable)
    }

    // ── Internals ────────────────────────────────────────────────────────────

    private fun writeCrashReport(thread: Thread, throwable: Throwable) {
        val ctx = appContext ?: return

        // Mark "crashed last run" before we try to write the file, so even if
        // the file write fails the flag is still set for the next-launch prompt.
        ctx.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
            .edit()
            .putBoolean(PREF_CRASHED_LAST_RUN, true)
            .commit() // commit() not apply() — we may die before apply() flushes

        val dir = AppLogger.logDir(ctx)
        dir.mkdirs()

        val ts = SimpleDateFormat("yyyyMMdd_HHmmss_SSS", Locale.US).format(Date())
        val file = File(dir, "crash_$ts.txt")

        FileWriter(file, false).use { w ->
            w.write(buildCrashHeader(ctx, thread))
            w.write(throwable.stackTraceToString())
            // Walk the cause chain
            var cause = throwable.cause
            while (cause != null) {
                w.write("\nCaused by: ")
                w.write(cause.stackTraceToString())
                cause = cause.cause
            }
        }

        AppLogger.e(TAG, "Crash captured to ${file.absolutePath}", throwable)
    }

    private fun buildCrashHeader(ctx: Context, thread: Thread): String {
        val pm = ctx.packageManager
        val pi = runCatching {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
                pm.getPackageInfo(ctx.packageName, android.content.pm.PackageManager.PackageInfoFlags.of(0))
            } else {
                @Suppress("DEPRECATION")
                pm.getPackageInfo(ctx.packageName, 0)
            }
        }.getOrNull()

        val versionName = pi?.versionName ?: "unknown"
        val versionCode = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.P) {
            pi?.longVersionCode?.toString() ?: "unknown"
        } else {
            @Suppress("DEPRECATION")
            pi?.versionCode?.toString() ?: "unknown"
        }

        return buildString {
            appendLine("=== CopyPaste Crash Report ===")
            appendLine("Timestamp    : ${SimpleDateFormat("yyyy-MM-dd HH:mm:ss.SSS z", Locale.US).format(Date())}")
            appendLine("App version  : $versionName ($versionCode)")
            appendLine("Android API  : ${Build.VERSION.SDK_INT} (${Build.VERSION.RELEASE})")
            appendLine("Device       : ${Build.MANUFACTURER} ${Build.MODEL} (${Build.DEVICE})")
            appendLine("Thread       : ${thread.name} [id=${thread.id}]")
            appendLine("Package      : ${ctx.packageName}")
            appendLine("================================")
            appendLine()
        }
    }
}
