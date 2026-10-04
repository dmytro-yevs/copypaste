package com.copypaste.app

import android.content.Context
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
    private var generation = 0L
    private var process: Process? = null
    private var reader: Thread? = null
    private var lastActivityAt = 0L

    @Synchronized
    fun start(context: Context, onStarted: (Boolean) -> Unit, onLost: () -> Unit): Boolean {
        generation += 1
        if (isListening()) {
            onStarted(true)
            return true
        }
        if (!AndroidCaptureState.privilegedGrants(context)) return false
        val currentGeneration = generation
        val app = context.applicationContext
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
            val timestamp = SimpleDateFormat("yyyy-MM-dd HH:mm:ss.SSS", Locale.getDefault())
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
            main.post { onStarted(true) }
            BufferedReader(InputStreamReader(logcat.inputStream)).use { input ->
                while (!Thread.currentThread().isInterrupted && generation == runGeneration) {
                    val line = input.readLine() ?: break
                    if (!line.contains(context.packageName)) continue
                    val now = SystemClock.elapsedRealtime()
                    if (now - lastActivityAt < activityDebounceMs) continue
                    lastActivityAt = now
                    main.post {
                        runCatching { context.startActivity(ClipboardFloatingActivity.intent(context)) }
                    }
                }
            }
        } catch (_: Exception) {
            if (!started) main.post { onStarted(false) }
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
            if (lost && started) main.post(onLost)
        }
    }
}
