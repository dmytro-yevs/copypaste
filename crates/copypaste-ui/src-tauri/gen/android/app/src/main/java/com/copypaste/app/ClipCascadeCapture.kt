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
    private val main = Handler(Looper.getMainLooper())

    private class Run(val onStarted: (Boolean) -> Unit, val onLost: () -> Unit) {
        var generation = 0L
        @Volatile var stopped = false
        @Volatile var started = false
        var process: Process? = null
        var reader: Thread? = null
        var lastActivityAt = 0L
    }

    private val runs = ClipCascadeRunGate<Run>()
    private var generation = 0L

    fun markSetupComplete(context: Context) {
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            .edit().putBoolean(KEY_SETUP_COMPLETE, true).apply()
    }

    fun isSetupComplete(context: Context): Boolean = hasRuntimePermissions(context)

    fun hasRuntimePermissions(context: Context): Boolean =
        ContextCompat.checkSelfPermission(context, Manifest.permission.READ_LOGS) == PackageManager.PERMISSION_GRANTED &&
            (Build.VERSION.SDK_INT < Build.VERSION_CODES.M || Settings.canDrawOverlays(context))

    fun isListening(): Boolean = runs.active()?.reader?.isAlive == true

    @Synchronized
    fun arm(context: Context, onStarted: (Boolean) -> Unit, onLost: () -> Unit): Boolean {
        generation++
        if (!isSetupComplete(context)) return false
        if (isListening()) {
            runs.active()?.generation = generation
            onStarted(true)
            return true
        }
        val run = runs.begin(Run(onStarted, onLost)) ?: return false
        run.generation = generation
        val app = context.applicationContext
        val reader = Thread({ readLogcat(run, app) }, "copypaste-clipcascade-logcat")
        reader.isDaemon = true
        run.reader = reader
        reader.start()
        return true
    }

    @Synchronized
    fun disarm() {
        generation++
        val run = runs.stop() ?: return
        run.stopped = true
        run.reader?.interrupt()
        run.process?.destroy()
    }

    private fun readLogcat(run: Run, app: Context) {
        try {
            val timestamp = SimpleDateFormat("yyyy-MM-dd HH:mm:ss.SSS", Locale.getDefault())
                .format(Date())
            val process = Runtime.getRuntime().exec(
                arrayOf("logcat", "-T", timestamp, "ClipboardService:E", "*:S"),
            )
            synchronized(this) {
                if (!runs.owns(run) || run.stopped) {
                    process.destroy()
                    return
                }
                run.process = process
            }
            main.post {
                synchronized(this) {
                    if (!runs.owns(run) || run.stopped) return@post
                    run.started = true
                }
                run.onStarted(true)
            }
            BufferedReader(InputStreamReader(process.inputStream)).use { input ->
                while (!run.stopped) {
                    val line = input.readLine() ?: break
                    if (!line.contains(BuildConfig.APPLICATION_ID)) continue
                    val now = android.os.SystemClock.elapsedRealtime()
                    if (now - run.lastActivityAt < ACTIVITY_DEBOUNCE_MS) continue
                    run.lastActivityAt = now
                    main.post {
                        synchronized(this) {
                            if (!runs.owns(run) || run.stopped) return@post
                        }
                        try {
                            app.startActivity(ClipboardFloatingActivity.intent(app))
                        } catch (error: Exception) {
                            Log.w(TAG, "floating capture activity launch failed", error)
                        }
                    }
                }
            }
        } catch (_: Exception) {
        } finally {
            val lost = synchronized(this) {
                run.process?.destroy()
                run.process = null
                run.reader = null
                runs.finish(run) && !run.stopped
            }
            if (lost) main.post {
                synchronized(this) {
                    if (runs.active() != null || generation != run.generation) return@post
                }
                if (run.started) run.onLost() else run.onStarted(false)
            }
        }
    }
}

internal class ClipCascadeRunGate<T> {
    private var active: T? = null

    @Synchronized fun begin(run: T): T? {
        if (active != null) return null
        active = run
        return run
    }

    @Synchronized fun active(): T? = active
    @Synchronized fun owns(run: T): Boolean = active === run
    @Synchronized fun stop(): T? = active.also { active = null }
    @Synchronized fun finish(run: T): Boolean {
        if (active !== run) return false
        active = null
        return true
    }
}
