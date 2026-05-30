package com.copypaste.android

import android.content.Context
import android.util.Log
import java.io.File
import java.io.FileWriter
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import java.util.concurrent.locks.ReentrantLock
import kotlin.concurrent.withLock

/**
 * Lightweight, thread-safe, rotating file logger for CopyPaste Android.
 *
 * Files are written to app-scoped external storage so they are retrievable
 * without root and without the app running:
 *
 *   adb pull /sdcard/Android/data/com.copypaste.android/files/logs/
 *
 * Rotation: when `app.log` exceeds [MAX_BYTES] it is renamed to `app.log.1`
 * (overwriting any previous `.1`) and a fresh `app.log` is started.
 * At most 2 files are kept → max ~3 MB on disk.
 *
 * Crash files written by [CrashHandler] are placed in the same directory
 * (`crash_<timestamp>.txt`) and are NOT rotated — they are evidence artefacts.
 *
 * Usage:
 *   AppLogger.init(context)          // once in Application.onCreate
 *   AppLogger.d("MyTag", "message")  // mirrors android.util.Log
 *   AppLogger.w("MyTag", "message")
 *   AppLogger.e("MyTag", "message", optionalThrowable)
 *   AppLogger.logDir(context)        // File pointing at the log directory
 */
object AppLogger {

    /** Maximum size of the active log file before rotation. */
    private const val MAX_BYTES = 1_500_000L // 1.5 MB

    private const val LOG_FILE = "app.log"
    private const val LOG_FILE_OLD = "app.log.1"
    const val LOG_DIR = "logs"

    private val lock = ReentrantLock()
    private val dateFmt = SimpleDateFormat("yyyy-MM-dd HH:mm:ss.SSS", Locale.US)

    @Volatile private var logFile: File? = null

    // ── Public API ──────────────────────────────────────────────────────────

    /**
     * Initialise the logger. Must be called in [android.app.Application.onCreate]
     * before any log calls. Safe to call multiple times; subsequent calls are no-ops.
     */
    fun init(context: Context) {
        if (logFile != null) return
        lock.withLock {
            if (logFile != null) return
            val dir = logDir(context)
            dir.mkdirs()
            logFile = File(dir, LOG_FILE)
        }
    }

    fun d(tag: String, msg: String) {
        Log.d(tag, msg)
        write("D", tag, msg, null)
    }

    fun i(tag: String, msg: String) {
        Log.i(tag, msg)
        write("I", tag, msg, null)
    }

    fun w(tag: String, msg: String, t: Throwable? = null) {
        if (t != null) Log.w(tag, msg, t) else Log.w(tag, msg)
        write("W", tag, msg, t)
    }

    fun e(tag: String, msg: String, t: Throwable? = null) {
        if (t != null) Log.e(tag, msg, t) else Log.e(tag, msg)
        write("E", tag, msg, t)
    }

    /**
     * Returns the log directory. Works even before [init] is called —
     * useful for the FileProvider path and export action.
     *
     * Path: `<externalFilesDir>/logs/`
     *
     * adb pull equivalent (no root, app not running):
     *   adb pull /sdcard/Android/data/com.copypaste.android/files/logs/
     */
    fun logDir(context: Context): File {
        // getExternalFilesDir is app-scoped (no permission needed on API 29+)
        // and is accessible via adb without root even when the app is not running.
        val base = context.getExternalFilesDir(null)
            ?: context.filesDir // internal fallback if external storage is unavailable
        return File(base, LOG_DIR)
    }

    /**
     * Returns all log files (app.log, app.log.1, crash_*.txt) sorted newest first.
     */
    fun allLogFiles(context: Context): List<File> {
        val dir = logDir(context)
        return dir.listFiles()
            ?.filter { it.isFile && it.length() > 0 }
            ?.sortedByDescending { it.lastModified() }
            ?: emptyList()
    }

    // ── Internal ────────────────────────────────────────────────────────────

    private fun write(level: String, tag: String, msg: String, t: Throwable?) {
        val file = logFile ?: return // not initialised — skip silently
        val timestamp = dateFmt.format(Date())
        val line = buildString {
            append(timestamp)
            append(" $level/$tag: ")
            append(msg)
            if (t != null) {
                append('\n')
                append(t.stackTraceToString())
            }
            append('\n')
        }
        lock.withLock {
            try {
                rotateIfNeeded(file)
                FileWriter(file, /* append= */ true).use { it.write(line) }
            } catch (_: Exception) {
                // Never let the logger crash the app. Silently ignore I/O errors.
            }
        }
    }

    private fun rotateIfNeeded(file: File) {
        if (file.length() < MAX_BYTES) return
        val old = File(file.parent, LOG_FILE_OLD)
        if (old.exists()) old.delete()
        file.renameTo(old)
        // file no longer exists — next FileWriter call creates it fresh
    }
}
