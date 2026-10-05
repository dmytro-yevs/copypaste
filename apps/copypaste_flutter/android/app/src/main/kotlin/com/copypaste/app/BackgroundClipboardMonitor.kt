package com.copypaste.app

import android.content.Context
import android.content.ClipboardManager
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import java.io.BufferedReader
import java.io.InputStreamReader
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale

internal object BackgroundClipboardMonitor {
    private const val activityDebounceMs = 1_000L
    private val main = Handler(Looper.getMainLooper())
    @Volatile private var generation = 0L
    private var process: Process? = null
    private var reader: Thread? = null
    private var lastActivityAt = 0L
    private var clipboard: ClipboardManager? = null
    private var clipboardListener: ClipboardManager.OnPrimaryClipChangedListener? = null

    @Synchronized
    fun start(context: Context, onStarted: (Boolean) -> Unit, onLost: () -> Unit): Boolean {
        if (isListening()) {
            onStarted(true)
            return true
        }
        if (!AndroidCaptureState.privilegedGrants(context)) return false
        stop()
        val currentGeneration = generation
        val app = context.applicationContext
        // Android logs denied background listener dispatches. The registration
        // must live with the service, not with the paused Flutter activity.
        clipboard = app.getSystemService(ClipboardManager::class.java)
        clipboardListener = ClipboardManager.OnPrimaryClipChangedListener {
            if (!MainActivity.isForeground && generation == currentGeneration) {
                AndroidClipboardReader.captureBackground(app)
            }
        }.also { clipboard?.addPrimaryClipChangedListener(it) }
        val thread = Thread(
            { readLogcat(currentGeneration, app, onStarted, onLost) },
            "copypaste-clipboard-logcat",
        ).apply { isDaemon = true }
        reader = thread
        thread.start()
        return true
    }

    @Synchronized
    fun stop() {
        generation += 1
        reader?.interrupt()
        process?.destroy()
        clipboardListener?.let { clipboard?.removePrimaryClipChangedListener(it) }
        clipboardListener = null
        clipboard = null
        reader = null
        process = null
    }

    @Synchronized
    fun isListening(): Boolean = reader?.isAlive == true && process?.isAlive == true

    private fun readLogcat(
        runGeneration: Long,
        context: Context,
        onStarted: (Boolean) -> Unit,
        onLost: () -> Unit,
    ) {
        var started = false
        try {
            val timestamp = SimpleDateFormat("yyyy-MM-dd HH:mm:ss.SSS", Locale.US)
                .format(Date())
            val logcat = Runtime.getRuntime().exec(
                arrayOf("logcat", "-T", timestamp, "ClipboardService:E", "*:S"),
            )
            synchronized(this) {
                if (generation != runGeneration) {
                    logcat.destroy()
                    return
                }
                process = logcat
            }
            started = true
            main.post { if (generation == runGeneration) onStarted(true) }
            BufferedReader(InputStreamReader(logcat.inputStream)).use { input ->
                while (!Thread.currentThread().isInterrupted && generation == runGeneration) {
                    val line = input.readLine() ?: break
                    if (!line.contains("Denying clipboard access to ${context.packageName}") ||
                        MainActivity.isForeground
                    ) continue
                    val now = SystemClock.elapsedRealtime()
                    if (now - lastActivityAt < activityDebounceMs) continue
                    lastActivityAt = now
                    main.post {
                        if (generation == runGeneration && !MainActivity.isForeground) {
                            runCatching { context.startActivity(ClipboardFloatingActivity.intent(context)) }
                        }
                    }
                }
            }
        } catch (_: Exception) {
            if (!started) main.post { if (generation == runGeneration) onStarted(false) }
        } finally {
            val lost = synchronized(this) {
                val owned = generation == runGeneration
                if (owned) {
                    process?.destroy()
                    process = null
                    reader = null
                }
                owned
            }
            if (lost && started) main.post {
                if (generation == runGeneration) onLost()
            }
        }
    }
}
