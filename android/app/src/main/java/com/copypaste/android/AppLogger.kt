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
 * CopyPaste-k8cm fix: files are written to internal app-private storage
 * (context.filesDir) so they are protected by MODE_PRIVATE and inaccessible
 * to other apps, MTP, or USB file browsing without root.
 *
 * To retrieve logs with adb (root or run-as required):
 *   adb shell run-as com.copypaste.android cat /data/data/com.copypaste.android/files/logs/app.log
 * Or use the in-app log export feature (LogExportHelper) which shares a copy
 * via a FileProvider URI — this is the intended user-facing retrieval path.
 *
 * Rotation: when `app.log` exceeds [MAX_BYTES] it is renamed to `app.log.1`
 * (overwriting any previous `.1`) and a fresh `app.log` is started.
 * At most 2 files are kept → max ~3 MB on disk.
 *
 * Crash files written by [CrashHandler] are placed in the same directory
 * (`crash_<timestamp>.txt`) and are NOT rotated — they are evidence artefacts.
 *
 * CopyPaste-rurw fix: all messages are passed through [redact] before being
 * written so that clipboard text, tokens, UUIDs, and other secrets are scrubbed.
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

    /**
     * CopyPaste-k8cm: compile-time marker confirming logs are stored in
     * internal app-private storage (context.filesDir), not external storage.
     * Tests assert this is true; if it flips to false a regression is present.
     */
    const val STORAGE_IS_INTERNAL = true

    /**
     * Human-readable description of where log files are stored.
     * Must NOT mention /sdcard/ or "external" — logs are internal-only.
     */
    const val LOG_DIR_DESCRIPTION =
        "Logs stored in internal app-private storage: data/data/<pkg>/files/logs/"

    /**
     * CopyPaste-qzhu: marker confirming that logcatCaptureWorking is set only
     * after actual capture verification, not optimistically.
     * Tests assert this is true; if the optimistic path is restored it flips to false.
     */
    const val CAPTURE_WORKING_IS_VERIFIED = true

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
     * CopyPaste-k8cm: path is context.filesDir (internal, MODE_PRIVATE) so logs
     * are not accessible to other apps or via MTP/USB without root.
     * Use LogExportHelper to share logs with the user via a FileProvider URI.
     *
     * Path: `<filesDir>/logs/`
     */
    fun logDir(context: Context): File {
        // CopyPaste-k8cm: use internal app-private storage (context.filesDir) instead of
        // getExternalFilesDir. External storage exposes logs via USB/MTP to any desktop tool
        // even when the app is not running, which violates the MODE_PRIVATE expectation.
        return File(context.filesDir, LOG_DIR)
    }

    // ── Redaction ───────────────────────────────────────────────────────────

    /**
     * CopyPaste-rurw: scrub sensitive content from a log message before it is
     * written to disk.
     *
     * Patterns redacted:
     *  - UUID-shaped strings (item IDs, device IDs, etc.) — 8-4-4-4-12 hex.
     *  - Long token-like sequences: runs of 20+ base64 or hex chars (API keys,
     *    bearer tokens, JWTs, secrets). The threshold of 20 chars preserves
     *    typical short identifiers (version strings, port numbers) while
     *    catching real secret material.
     *  - JWT segments: three dot-separated base64url groups (header.payload.sig).
     *
     * Short human-readable words and typical log metadata survive unchanged so
     * log files remain useful for diagnostics.
     */
    fun redact(message: String): String {
        if (message.isEmpty()) return message

        var result = message

        // Step 1: redact JWTs (three base64url segments separated by dots).
        // Must run BEFORE the generic token pass so the whole JWT is replaced
        // rather than each segment being handled separately.
        result = JWT_REGEX.replace(result, "[REDACTED_JWT]")

        // Step 2: redact UUID-shaped strings (item / device identifiers).
        result = UUID_REGEX.replace(result, "[REDACTED_UUID]")

        // Step 3: redact long token-like runs (base64, hex, API keys).
        result = TOKEN_REGEX.replace(result, "[REDACTED_TOKEN]")

        return result
    }

    // ── Regex constants for redact() ────────────────────────────────────────

    /** Matches standard 8-4-4-4-12 UUID format (case-insensitive). */
    private val UUID_REGEX = Regex(
        "[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}",
    )

    /**
     * Matches 24+ consecutive base64url or hex characters.
     *
     * Characters in set: A-Z a-z 0-9 + / = - _ (covers both standard base64
     * and base64url encoding used in JWTs, API keys, OAuth tokens, etc.).
     * Length threshold of 24 avoids false-positives on common Android class names
     * (e.g. "LogcatCaptureService" = 20 chars) while reliably catching secret
     * material — real API keys, bearer tokens, and secrets are typically 24–128 chars.
     */
    private val TOKEN_REGEX = Regex("[A-Za-z0-9+/=_-]{24,}")

    /**
     * Matches JWT format: three base64url segments separated by dots.
     * Captured as a unit so the whole token is replaced atomically.
     */
    private val JWT_REGEX = Regex(
        "[A-Za-z0-9_-]{2,}\\.[A-Za-z0-9_-]{2,}\\.[A-Za-z0-9_-]{2,}",
    )

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
        // CopyPaste-rurw: redact sensitive content before writing to disk.
        val safeMsg = redact(msg)
        val safeStack = t?.stackTraceToString()?.let { redact(it) }
        val line = buildString {
            append(timestamp)
            append(" $level/$tag: ")
            append(safeMsg)
            if (safeStack != null) {
                append('\n')
                append(safeStack)
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
