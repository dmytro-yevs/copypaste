package com.copypaste.app

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.provider.OpenableColumns
import io.flutter.plugin.common.BinaryMessenger
import io.flutter.plugin.common.MethodChannel
import java.io.File
import java.io.RandomAccessFile
import java.util.UUID
import java.util.concurrent.Executors

/** Preserves document metadata and reads only the file currently being imported. */
internal class HistoryFilesChannel(private val activity: MainActivity, messenger: BinaryMessenger) {
    private val channel = MethodChannel(messenger, "com.copypaste.app/history_files")
    private val worker = Executors.newSingleThreadExecutor()
    private val selections = mutableMapOf<String, Uri>()
    private val staged = mutableMapOf<String, File>()
    private val leases = mutableMapOf<String, RandomAccessFile>()
    private var pending: MethodChannel.Result? = null
    @Volatile private var disposed = false

    init {
        worker.execute { runCatching { removeAbandonedImports() } }
        channel.setMethodCallHandler { call, result ->
            when (call.method) {
                "chooseFiles" -> {
                    if (pending != null) result.error("picker_busy", "The file picker is already open.", null)
                    else {
                        pending = result
                        try {
                            activity.willOpenHistoryFilePicker()
                            activity.startActivityForResult(Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
                                addCategory(Intent.CATEGORY_OPENABLE)
                                type = "*/*"
                                putExtra(Intent.EXTRA_ALLOW_MULTIPLE, true)
                                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                            }, REQUEST)
                        } catch (_: Exception) {
                            activity.failedToOpenHistoryFilePicker()
                            pending = null
                            result.error("picker_unavailable", "Files could not be selected.", null)
                        }
                    }
                }
                "materialize" -> {
                    val token = call.argument<String>("token")
                    val maxBytes = call.argument<Number>("maxBytes")?.toLong()
                    worker.execute {
                        try {
                            val uri = selections[token] ?: error("Unknown selection")
                            require(maxBytes != null && maxBytes > 0)
                            activity.contentResolver.query(uri,arrayOf(OpenableColumns.SIZE),null,null,null)?.use { cursor ->
                                if (cursor.moveToFirst() && !cursor.isNull(0)) {
                                    val size=cursor.getLong(0)
                                    require(size<=maxBytes) { "The file is $size bytes; the file size limit in Settings is $maxBytes bytes." }
                                }
                            }
                            val file = File.createTempFile("history-import-", ".bin", activity.cacheDir)
                            staged[token!!] = file
                            activeImports.add(file.absolutePath)
                            val lease = RandomAccessFile(file, "rw")
                            leases[token] = lease
                            lease.channel.lock()
                            activity.contentResolver.openInputStream(uri).use { input ->
                                requireNotNull(input)
                                run {
                                    val output = leases.getValue(token)
                                    val buffer = ByteArray(64 * 1024)
                                    var total = 0L
                                    while (true) {
                                        val count = input.read(buffer)
                                        if (count < 0) break
                                        total += count
                                        require(total <= maxBytes) { "The file is at least $total bytes; the file size limit in Settings is $maxBytes bytes." }
                                        output.write(buffer, 0, count)
                                    }
                                }
                            }
                            reply { result.success(file.absolutePath) }
                        } catch (error: Exception) {
                            leases.remove(token)?.close()
                            staged.remove(token)?.let { activeImports.remove(it.absolutePath); it.delete() }
                            val message = if (error is IllegalArgumentException && error.message?.startsWith("The file is ")==true) error.message else "The selected file could not be read. Check file access and available disk space."
                            reply { result.error("file_import_failed", message, null) }
                        }
                    }
                }
                "release" -> worker.execute {
                    val token = call.argument<String>("token")
                    leases.remove(token)?.close()
                    staged.remove(token)?.let { activeImports.remove(it.absolutePath); it.delete() }
                    selections.remove(token)
                    reply { result.success(null) }
                }
                else -> result.notImplemented()
            }
        }
    }

    fun onActivityResult(request: Int, resultCode: Int, data: Intent?): Boolean {
        if (request != REQUEST) return false
        val result = pending ?: return true
        pending = null
        if (resultCode != Activity.RESULT_OK || data == null) {
            result.success(emptyList<Any>())
            return true
        }
        val uris = buildList {
            val clips = data.clipData
            if (clips != null) for (index in 0 until clips.itemCount) add(clips.getItemAt(index).uri)
            else data.data?.let { add(it) }
        }
        worker.execute {
            try {
                val files = uris.map { uri ->
                    var name = uri.lastPathSegment?.substringAfterLast('/') ?: "file"
                    activity.contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { cursor ->
                        if (cursor.moveToFirst() && !cursor.isNull(0)) name = cursor.getString(0)
                    }
                    val token = UUID.randomUUID().toString()
                    selections[token] = uri
                    mapOf("token" to token, "name" to name, "mimeType" to (activity.contentResolver.getType(uri) ?: "application/octet-stream"), "uri" to uri.toString())
                }
                reply { result.success(files) }
            } catch (_: Exception) {
                selections.clear()
                reply { result.error("file_metadata_failed", "The selected files could not be read.", null) }
            }
        }
        return true
    }

    private fun reply(action: () -> Unit) = activity.runOnUiThread { if (!disposed) action() }

    fun dispose() {
        disposed = true
        channel.setMethodCallHandler(null)
        pending?.error("picker_closed", "The file picker was closed.", null)
        pending = null
        worker.execute {
            leases.values.forEach { it.close() }
            leases.clear()
            staged.values.forEach { activeImports.remove(it.absolutePath); it.delete() }
            staged.clear()
            selections.clear()
        }
        worker.shutdown()
    }

    private fun removeAbandonedImports() {
        val cutoff = System.currentTimeMillis() - 24 * 60 * 60 * 1000L
        val root = activity.cacheDir.canonicalFile
        activity.cacheDir.listFiles()?.forEach { file ->
            if (!file.name.startsWith("history-import-") || !file.name.endsWith(".bin") ||
                !file.isFile || file.absolutePath in activeImports || file.lastModified() >= cutoff || file.canonicalFile != File(root, file.name)) return@forEach
            runCatching {
                RandomAccessFile(file, "rw").use { lease ->
                    lease.channel.tryLock()?.use { file.delete() }
                }
            }
        }
    }

    companion object {
        private val activeImports = java.util.concurrent.ConcurrentHashMap.newKeySet<String>()
        private const val REQUEST = 48123
    }
}
