package com.copypaste.app

import android.os.RemoteException
import android.os.SystemClock
import java.io.BufferedReader
import java.io.InputStreamReader
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import kotlin.system.exitProcess

internal fun isClipboardAccessEvent(line: String, packageName: String): Boolean =
    line.contains("ClipboardService") && line.contains(packageName)

internal class ClipboardAccessSignals(
    private val nowMs: () -> Long = SystemClock::elapsedRealtime,
) {
    private var lastForwardedAtMs = Long.MIN_VALUE

    fun shouldForward(): Boolean {
        val now = nowMs()
        if (lastForwardedAtMs != Long.MIN_VALUE && now - lastForwardedAtMs < DEBOUNCE_MS) {
            return false
        }
        lastForwardedAtMs = now
        return true
    }

    private companion object {
        const val DEBOUNCE_MS = 1_000L
    }
}

internal class ShizukuCaptureRuns {
    internal class Run(val listener: IClipCascadeCaptureListener) {
        @Volatile var stopped = false
        var process: Process? = null
        var reader: Thread? = null
        val signals = ClipboardAccessSignals()
    }

    private var active: Run? = null

    @Synchronized
    fun begin(listener: IClipCascadeCaptureListener): Run? {
        if (active != null) return null
        return Run(listener).also { active = it }
    }

    @Synchronized
    fun attachReader(run: Run, reader: Thread): Boolean {
        if (active !== run || run.stopped) return false
        run.reader = reader
        return true
    }

    @Synchronized
    fun attachProcess(run: Run, process: Process): Boolean {
        if (active !== run || run.stopped) return false
        run.process = process
        return true
    }

    @Synchronized
    fun stop(): Run? {
        val run = active ?: return null
        run.stopped = true
        active = null
        return run
    }

    @Synchronized
    fun finish(run: Run): IClipCascadeCaptureListener? {
        run.process?.destroy()
        run.process = null
        run.reader = null
        if (active !== run) return null
        active = null
        return if (run.stopped) null else run.listener
    }

    @Synchronized
    fun activeForTest(): Run? = active
}

/** The Shizuku-owned half of background capture; it returns occurrence signals only. */
class ShizukuCaptureService : IShizukuCaptureService.Stub() {
    private val runs = ShizukuCaptureRuns()

    override fun start(callback: IClipCascadeCaptureListener?): Boolean {
        val listener = callback ?: return false
        val run = runs.begin(listener) ?: return false
        val reader = Thread({ readLogcat(run) }, "copypaste-shizuku-logcat").apply {
            isDaemon = true
        }
        if (!runs.attachReader(run, reader)) return false
        reader.start()
        return true
    }

    override fun stop() {
        val run = runs.stop() ?: return
        run.reader?.interrupt()
        run.process?.destroy()
    }

    override fun destroy() {
        stop()
        exitProcess(0)
    }

    private fun readLogcat(run: ShizukuCaptureRuns.Run) {
        try {
            val timestamp = SimpleDateFormat(
                "yyyy-MM-dd HH:mm:ss.SSS",
                Locale.getDefault(),
            ).format(Date())
            val process = ProcessBuilder(
                "logcat", "-T", timestamp, "ClipboardService:E", "*:S",
            ).start()
            if (!runs.attachProcess(run, process)) {
                process.destroy()
                return
            }
            BufferedReader(InputStreamReader(process.inputStream)).use { input ->
                while (!run.stopped) {
                    val line = input.readLine() ?: break
                    if (isClipboardAccessEvent(line, BuildConfig.APPLICATION_ID) &&
                        run.signals.shouldForward()
                    ) {
                        run.listener.onClipboardAccess()
                    }
                }
            }
        } catch (_: RemoteException) {
        } catch (_: Exception) {
        } finally {
            val listener = runs.finish(run) ?: return
            try {
                listener.onCaptureStopped()
            } catch (_: RemoteException) {
            }
        }
    }
}
