package com.copypaste.app

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.provider.OpenableColumns
import io.flutter.plugin.common.BinaryMessenger
import io.flutter.plugin.common.MethodChannel
import java.io.File
import java.util.UUID
import java.util.concurrent.Executors

/** Preserves document metadata and reads only the file currently being imported. */
internal class HistoryFilesChannel(private val activity: MainActivity, messenger: BinaryMessenger) {
    private val channel = MethodChannel(messenger, "com.copypaste.app/history_files")
    private val worker = Executors.newSingleThreadExecutor()
    private val selections = mutableMapOf<String, Uri>()
    private val staged = mutableMapOf<String, File>()
    private var pending: MethodChannel.Result? = null
    @Volatile private var disposed = false

    init {
        channel.setMethodCallHandler { call, result ->
            when (call.method) {
                "chooseFiles" -> {
                    if (pending != null) result.error("picker_busy", "The file picker is already open.", null)
                    else {
                        pending = result
                        try {
                            activity.startActivityForResult(Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
                                addCategory(Intent.CATEGORY_OPENABLE)
                                type = "*/*"
                                putExtra(Intent.EXTRA_ALLOW_MULTIPLE, true)
                                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION)
                            }, REQUEST)
                        } catch (_: Exception) {
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
                            val file = File.createTempFile("history-import-", ".bin", activity.cacheDir)
                            staged[token!!] = file
                            activity.contentResolver.openInputStream(uri).use { input ->
                                requireNotNull(input)
                                file.outputStream().use { output ->
                                    val buffer = ByteArray(64 * 1024)
                                    var total = 0L
                                    while (true) {
                                        val count = input.read(buffer)
                                        if (count < 0) break
                                        total += count
                                        require(total <= maxBytes)
                                        output.write(buffer, 0, count)
                                    }
                                }
                            }
                            reply { result.success(file.absolutePath) }
                        } catch (_: Exception) {
                            staged.remove(token)?.delete()
                            reply { result.error("file_import_failed", "The selected file could not be read or exceeds the History size limit.", null) }
                        }
                    }
                }
                "release" -> worker.execute {
                    val token = call.argument<String>("token")
                    staged.remove(token)?.delete()
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
            staged.values.forEach { it.delete() }
            staged.clear()
            selections.clear()
        }
        worker.shutdown()
    }

    companion object { private const val REQUEST = 48123 }
}
