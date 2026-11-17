package com.copypaste.app

import android.content.ContentResolver
import android.content.ContentUris
import android.content.ContentValues
import android.content.Context
import android.database.ContentObserver
import android.database.Cursor
import android.database.sqlite.SQLiteDatabase
import android.database.sqlite.SQLiteOpenHelper
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Handler
import android.os.HandlerThread
import android.provider.MediaStore
import android.provider.MediaStore.Images.Media as Images
import java.util.Locale

internal interface ScreenshotCaptureReader {
    fun policyAllowsCapture(): Boolean
    fun readMetadata(action: () -> Unit): Boolean
    fun capture(uri: Uri, type: String, displayName: String): Boolean
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
    override fun capture(uri: Uri, type: String, displayName: String): Boolean = host()?.let {
        AndroidClipboardReader.captureScreenshot(context, it, uri, type, displayName)
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
        if (closed || !ScreenshotCaptureState.enabled(context)) return
        if (!ScreenshotCaptureState.reconcileMediaPermission(context)) return
        if (ScreenshotCaptureState.paused(context)) {
            ScreenshotCaptureState.skipThrough(context)
            return
        }
        val since = ScreenshotCaptureState.since(context)
        if (since == 0L) return
        // Rows are read in _ID (insertion) order. The persisted mark advances only
        // through a contiguous run of settled rows, so pending and failed rows,
        // and every row after them, are reconsidered by later scans. A row added
        // after the mark has a larger _ID even if the clock moved back, and a
        // rebuilt MediaStore restarts _IDs with current DATE_ADDED values.
        var mark = ScreenshotCaptureState.scanMark(context)
        var advancing = true
        var afterId = 0L
        while (!closed) {
            val rows = mutableListOf<Row>()
            var rowsRead = 0
            try {
                val allowed = reader.readMetadata {
                    val pathColumn = if (Build.VERSION.SDK_INT >= 29) MediaStore.Images.Media.RELATIVE_PATH else MediaStore.Images.Media.DATA
                    val columns = mutableListOf(Images._ID, Images.DISPLAY_NAME, pathColumn, Images.MIME_TYPE, Images.DATE_ADDED, Images.DATE_TAKEN)
                    if (Build.VERSION.SDK_INT >= 29) columns.add(Images.IS_PENDING)
                    val selection = "${Images.DATE_ADDED} >= ? AND (${Images._ID} > ? OR ${Images.DATE_ADDED} >= ?) AND ${Images._ID} > ?"
                    val arguments = arrayOf((since / 1000).toString(), mark.id.toString(),
                        mark.addedSeconds.toString(), afterId.toString())
                    queryImages(columns.toTypedArray(), selection, arguments)?.use { cursor ->
                        while (!closed && rowsRead < CANDIDATE_PAGE_SIZE && cursor.moveToNext()) {
                            rowsRead++
                            val id = cursor.getLong(0)
                            afterId = id
                            val added = cursor.getLong(4)
                            if (Build.VERSION.SDK_INT >= 29 && cursor.getInt(6) != 0) {
                                rows.add(Row(id, added, null, settled = false))
                                continue
                            }
                            val name = cursor.getString(1).orEmpty()
                            val path = cursor.getString(2).orEmpty()
                            val type = cursor.getString(3).orEmpty()
                            val taken = cursor.getLong(5)
                            if (!isScreenshot(name, path) || !type.startsWith("image/") ||
                                !isNew(since, added, taken)) {
                                rows.add(Row(id, added, null, settled = true))
                                continue
                            }
                            val uri = ContentUris.withAppendedId(Images.EXTERNAL_CONTENT_URI, id)
                            val receipt = "$uri:$added:$taken"
                            rows.add(if (receipts.contains(receipt)) Row(id, added, null, settled = true)
                                else Row(id, added, Candidate(uri, type, receipt, name), settled = false))
                        }
                    }
                }
                if (!allowed) {
                    // Paused/private/excluded capture must never replay these screenshots later.
                    if (!reader.policyAllowsCapture()) ScreenshotCaptureState.skipThrough(context)
                    else if (!closed) worker.postDelayed(scanTask, 250L)
                    return
                }
                for (row in rows) {
                    val (uri, type, receipt, displayName) = row.candidate ?: continue
                    if (closed || !ScreenshotCaptureState.enabled(context)) return
                    if (!ScreenshotCaptureState.reconcileMediaPermission(context)) return
                    if (reader.capture(uri, type, displayName)) {
                        receipts.record(receipt)
                        row.settled = true
                    } else {
                        if (!ScreenshotCaptureState.reconcileMediaPermission(context)) return
                        if (!reader.readMetadata {}) {
                            if (!reader.policyAllowsCapture()) ScreenshotCaptureState.skipThrough(context)
                            return
                        }
                    }
                }
                if (advancing) {
                    val previous = mark
                    for (row in rows) {
                        if (!row.settled) { advancing = false; break }
                        mark = ScreenshotCaptureState.ScanMark(row.id, maxOf(mark.addedSeconds, row.addedSeconds))
                    }
                    if (mark != previous) ScreenshotCaptureState.setScanMark(context, mark)
                }
            } catch (_: RuntimeException) {
                // Permission revocation and provider failures carry no image data into logs.
                ScreenshotCaptureState.reconcileMediaPermission(context)
                return
            }
            if (rowsRead < CANDIDATE_PAGE_SIZE) return
            if (!ScreenshotCaptureState.enabled(context) ||
                ScreenshotCaptureState.since(context) != since) return
        }
    }

    /** Pending rows must stay visible so they hold the scan mark until publication. */
    private fun queryImages(columns: Array<String>, selection: String, arguments: Array<String>): Cursor? {
        val order = "${Images._ID} ASC"
        return if (Build.VERSION.SDK_INT >= 30) context.contentResolver.query(
            Images.EXTERNAL_CONTENT_URI, columns, Bundle().apply {
                putString(ContentResolver.QUERY_ARG_SQL_SELECTION, selection)
                putStringArray(ContentResolver.QUERY_ARG_SQL_SELECTION_ARGS, arguments)
                putString(ContentResolver.QUERY_ARG_SQL_SORT_ORDER, order)
                putInt(MediaStore.QUERY_ARG_MATCH_PENDING, MediaStore.MATCH_INCLUDE)
            }, null,
        ) else {
            @Suppress("DEPRECATION")
            val uri = if (Build.VERSION.SDK_INT >= 29) MediaStore.setIncludePending(Images.EXTERNAL_CONTENT_URI)
                else Images.EXTERNAL_CONTENT_URI
            context.contentResolver.query(uri, columns, selection, arguments, order)
        }
    }

    private data class Candidate(val uri: Uri, val type: String, val receipt: String, val displayName: String)
    private class Row(val id: Long, val addedSeconds: Long, val candidate: Candidate?, var settled: Boolean)

    companion object {
        internal const val CANDIDATE_PAGE_SIZE = 32
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
