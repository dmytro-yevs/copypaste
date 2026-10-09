package com.copypaste.app

import android.content.ContentUris
import android.content.ContentValues
import android.content.Context
import android.database.ContentObserver
import android.database.sqlite.SQLiteDatabase
import android.database.sqlite.SQLiteOpenHelper
import android.net.Uri
import android.os.Build
import android.os.Handler
import android.os.HandlerThread
import android.provider.MediaStore
import android.provider.MediaStore.Images.Media as Images
import java.util.Locale

internal interface ScreenshotCaptureReader {
    fun policyAllowsCapture(): Boolean
    fun readMetadata(action: () -> Unit): Boolean
    fun capture(uri: Uri, type: String, takenAt: Long): Boolean
    fun close(completion: (Boolean) -> Unit)
}

internal class NativeScreenshotCaptureReader(private val context: Context) : ScreenshotCaptureReader {
    @Volatile private var closed = false
    private var capability: AndroidClipboardReader.Host? = null
    @Synchronized private fun host(): AndroidClipboardReader.Host? {
        if (closed) return null
        return capability ?: AndroidClipboardReader.openHost().also { capability = it }
    }
    override fun policyAllowsCapture(): Boolean = runCatching {
        NativeRuntimeCapture.implicitCaptureAllowed()
    }.getOrDefault(false)
    override fun readMetadata(action: () -> Unit): Boolean = host()?.let {
        AndroidClipboardReader.readCaptureMetadata(it, action)
    } ?: false
    override fun capture(uri: Uri, type: String, takenAt: Long): Boolean = host()?.let {
        AndroidClipboardReader.captureScreenshot(context, it, uri, type, takenAt)
    } ?: false
    @Synchronized override fun close(completion: (Boolean) -> Unit) {
        closed = true
        capability?.close(completion) ?: completion(true)
    }
}

/** Tracks receipts separately from History so deleted clips are never reimported. */
internal class ScreenshotCaptureReceipts(context: Context) : SQLiteOpenHelper(context, "screenshot-receipts.db", null, 1) {
    override fun onCreate(db: SQLiteDatabase) { db.execSQL("CREATE TABLE receipts (source TEXT PRIMARY KEY)") }
    override fun onUpgrade(db: SQLiteDatabase, oldVersion: Int, newVersion: Int) = Unit
    fun contains(source: String): Boolean = readableDatabase.rawQuery(
        "SELECT 1 FROM receipts WHERE source = ?", arrayOf(source),
    ).use { it.moveToFirst() }
    fun record(source: String) { writableDatabase.insertWithOnConflict(
        "receipts", null, ContentValues().apply { put("source", source) }, SQLiteDatabase.CONFLICT_IGNORE,
    ) }
}

internal class ScreenshotCaptureMonitor(
    private val context: Context,
    private val reader: ScreenshotCaptureReader = NativeScreenshotCaptureReader(context),
) {
    private val thread = HandlerThread("copypaste-screenshots").apply { start() }
    private val worker = Handler(thread.looper)
    private val receipts = ScreenshotCaptureReceipts(context)
    @Volatile private var closed = false
    private val scanTask = Runnable { scan() }
    private val observer = object : ContentObserver(worker) {
        override fun onChange(selfChange: Boolean) { refresh() }
    }

    fun start(): Boolean = try {
        context.contentResolver.registerContentObserver(MediaStore.Images.Media.EXTERNAL_CONTENT_URI, true, observer)
        refresh()
        true
    } catch (_: RuntimeException) { false }

    fun refresh() {
        // Coalesce provider notifications; IS_PENDING rows are reconsidered after publication.
        worker.removeCallbacks(scanTask)
        worker.post(scanTask)
    }

    fun close(completion: (Boolean) -> Unit = {}) {
        closed = true
        runCatching { context.contentResolver.unregisterContentObserver(observer) }
        worker.removeCallbacksAndMessages(null)
        reader.close { drained ->
            worker.post { receipts.close(); thread.quitSafely() }
            completion(drained)
        }
    }

    internal fun scan() {
        if (closed || !ScreenshotCaptureState.enabled(context) || !ScreenshotCaptureState.mediaGranted(context)) return
        if (ScreenshotCaptureState.paused(context)) {
            ScreenshotCaptureState.skipThrough(context)
            return
        }
        val since = ScreenshotCaptureState.since(context)
        if (since == 0L) return
        val candidates = mutableListOf<Candidate>()
        try {
            val allowed = reader.readMetadata {
                val pathColumn = if (Build.VERSION.SDK_INT >= 29) MediaStore.Images.Media.RELATIVE_PATH else MediaStore.Images.Media.DATA
                val columns = mutableListOf(Images._ID, Images.DISPLAY_NAME, pathColumn, Images.MIME_TYPE, Images.DATE_ADDED, Images.DATE_TAKEN)
                if (Build.VERSION.SDK_INT >= 29) columns.add(Images.IS_PENDING)
                context.contentResolver.query(
                    Images.EXTERNAL_CONTENT_URI, columns.toTypedArray(), "${Images.DATE_ADDED} >= ?",
                    arrayOf((since / 1000).toString()), "${Images.DATE_ADDED} ASC, ${Images._ID} ASC",
                )?.use { cursor ->
                    while (!closed && cursor.moveToNext()) {
                        if (Build.VERSION.SDK_INT >= 29 && cursor.getInt(6) != 0) continue
                        val name = cursor.getString(1).orEmpty()
                        val path = cursor.getString(2).orEmpty()
                        val type = cursor.getString(3).orEmpty()
                        val added = cursor.getLong(4)
                        val taken = cursor.getLong(5)
                        if (!isScreenshot(name, path) || !type.startsWith("image/") ||
                            !isNew(since, added, taken)) continue
                        val uri = ContentUris.withAppendedId(Images.EXTERNAL_CONTENT_URI, cursor.getLong(0))
                        val receipt = "$uri:$added:$taken"
                        if (!receipts.contains(receipt)) candidates.add(Candidate(uri, type, receipt, taken))
                    }
                }
            }
            if (!allowed) {
                // Paused/private/excluded capture must never replay these screenshots later.
                if (!reader.policyAllowsCapture()) ScreenshotCaptureState.skipThrough(context)
                else if (!closed) worker.postDelayed(scanTask, 250L)
                return
            }
            for ((uri, type, receipt, takenAt) in candidates) {
                if (closed || !ScreenshotCaptureState.enabled(context) || !ScreenshotCaptureState.mediaGranted(context)) return
                if (reader.capture(uri, type, takenAt)) receipts.record(receipt)
                else if (!reader.readMetadata {}) {
                    if (!reader.policyAllowsCapture()) ScreenshotCaptureState.skipThrough(context)
                    return
                }
            }
        } catch (_: RuntimeException) {
            // Permission revocation and provider failures carry no image data into logs.
        }
    }

    private data class Candidate(val uri: Uri, val type: String, val receipt: String, val takenAt: Long)

    companion object {
        internal fun isScreenshot(name: String, path: String): Boolean {
            val normalizedName = name.lowercase(Locale.ROOT).replace("_", "").replace("-", "").replace(" ", "")
            val directories = path.lowercase(Locale.ROOT).replace('\\', '/').split('/')
            return normalizedName.startsWith("screenshot") || normalizedName.startsWith("screencapture") ||
                directories.any { it == "screenshots" || it == "screenshot" || it == "screen captures" || it == "screencapture" }
        }
        internal fun isNew(since: Long, addedSeconds: Long, takenMillis: Long): Boolean =
            if (takenMillis > 0) takenMillis >= since else addedSeconds * 1000 >= since
    }
}
