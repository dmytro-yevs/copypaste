package com.copypaste.app

import android.Manifest
import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.provider.Settings
import android.util.Log
import androidx.core.content.ContextCompat
import java.io.BufferedReader
import java.io.InputStreamReader
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

object ClipCascadeCapture {
    private const val TAG = "CopyPasteClipCascade"
    private const val PREFS = "clipcascade-capture"
    private const val KEY_SETUP_COMPLETE = "setupComplete"
    private const val ACTIVITY_DEBOUNCE_MS = 1_000L

    @Volatile
    private var stopRequested = false

    @Volatile
    private var logcatThread: Thread? = null

    @Volatile
    private var logcatProcess: Process? = null

    @Volatile
    private var lastActivityStartAt = 0L

    private val main = Handler(Looper.getMainLooper())

    fun markSetupComplete(context: Context) {
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            .edit()
            .putBoolean(KEY_SETUP_COMPLETE, true)
            .apply()
    }

    fun isSetupComplete(context: Context): Boolean =
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            .getBoolean(KEY_SETUP_COMPLETE, false) &&
            hasRuntimePermissions(context)

    fun hasRuntimePermissions(context: Context): Boolean =
        ContextCompat.checkSelfPermission(
            context,
            Manifest.permission.READ_LOGS,
        ) == PackageManager.PERMISSION_GRANTED &&
            (Build.VERSION.SDK_INT < Build.VERSION_CODES.M || Settings.canDrawOverlays(context))

    fun isListening(): Boolean = logcatThread?.isAlive == true

    @Synchronized
    fun arm(context: Context, onLost: () -> Unit): Boolean {
        if (!hasRuntimePermissions(context)) return false
        if (isListening()) return true

        val app = context.applicationContext
        stopRequested = false
        logcatThread = Thread {
            var expectedStop = false
            try {
                val timeStamp = SimpleDateFormat(
                    "yyyy-MM-dd HH:mm:ss.SSS",
                    Locale.getDefault(),
                ).format(Date())
                logcatProcess = Runtime.getRuntime().exec(
                    arrayOf("logcat", "-T", timeStamp, "ClipboardService:E", "*:S"),
                )
                BufferedReader(InputStreamReader(logcatProcess!!.inputStream)).use { reader ->
                    while (!stopRequested) {
                        val line = reader.readLine() ?: break
                        if (!line.contains(BuildConfig.APPLICATION_ID)) continue
                        val now = System.currentTimeMillis()
                        if (now - lastActivityStartAt <= ACTIVITY_DEBOUNCE_MS) continue
                        lastActivityStartAt = now
                        val intent = ClipboardFloatingActivity.intent(app)
                        main.post {
                            try {
                                app.startActivity(intent)
                            } catch (e: Exception) {
                                Log.w(TAG, "floating capture activity launch failed", e)
                            }
                        }
                    }
                }
                expectedStop = stopRequested
            } catch (e: Exception) {
                expectedStop = stopRequested
                if (!expectedStop) {
                    Log.w(TAG, "logcat capture failed", e)
                }
            } finally {
                try {
                    logcatProcess?.destroy()
                } catch (_: Exception) {
                }
                logcatProcess = null
                logcatThread = null
                if (!expectedStop && !stopRequested) {
                    main.post(onLost)
                }
                stopRequested = false
            }
        }.apply {
            isDaemon = true
            name = "copypaste-clipcascade-logcat"
            start()
        }
        return true
    }

    @Synchronized
    fun disarm() {
        stopRequested = true
        try {
            logcatThread?.interrupt()
        } catch (_: Exception) {
        }
        try {
            logcatProcess?.destroy()
        } catch (_: Exception) {
        }
        logcatThread = null
        logcatProcess = null
    }
}
