package com.copypaste.app

import android.os.RemoteException
import java.io.BufferedReader
import java.io.InputStreamReader
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import kotlin.concurrent.thread
import kotlin.system.exitProcess

internal fun isClipboardAccessEvent(line: String, packageName: String): Boolean =
    line.contains("ClipboardService") && line.contains(packageName)

/** The Shizuku-owned half of background capture; it returns occurrence signals only. */
class ShizukuCaptureService : IShizukuCaptureService.Stub() {
    private val lock = Any()
    private var stopRequested = false
    private var process: Process? = null
    private var reader: Thread? = null
    private var listener: IClipCascadeCaptureListener? = null

    override fun start(callback: IClipCascadeCaptureListener?): Boolean = synchronized(lock) {
        if (callback == null || reader?.isAlive == true) return false
        stopRequested = false
        listener = callback
        reader = thread(start = true, isDaemon = true, name = "copypaste-shizuku-logcat") {
            var expectedStop = false
            try {
                val timestamp = SimpleDateFormat(
                    "yyyy-MM-dd HH:mm:ss.SSS",
                    Locale.getDefault(),
                ).format(Date())
                val started = ProcessBuilder(
                    "logcat", "-T", timestamp, "ClipboardService:E", "*:S",
                ).start()
                synchronized(lock) { process = started }
                BufferedReader(InputStreamReader(started.inputStream)).use { input ->
                    while (!stopRequested) {
                        val line = input.readLine() ?: break
                        if (isClipboardAccessEvent(line, BuildConfig.APPLICATION_ID)) {
                            listener?.onClipboardAccess()
                        }
                    }
                }
                expectedStop = stopRequested
            } catch (_: RemoteException) {
                expectedStop = stopRequested
            } catch (_: Exception) {
                expectedStop = stopRequested
            } finally {
                val notifyLoss = synchronized(lock) {
                    process?.destroy()
                    process = null
                    reader = null
                    val callback = listener
                    listener = null
                    if (!expectedStop && !stopRequested) callback else null
                }
                if (notifyLoss != null) {
                    try {
                        notifyLoss.onCaptureStopped()
                    } catch (_: RemoteException) {
                    }
                }
                stopRequested = false
            }
        }
        true
    }

    override fun stop() {
        synchronized(lock) {
            stopRequested = true
            reader?.interrupt()
            process?.destroy()
            reader = null
            process = null
            listener = null
        }
    }

    override fun destroy() {
        stop()
        exitProcess(0)
    }
}
