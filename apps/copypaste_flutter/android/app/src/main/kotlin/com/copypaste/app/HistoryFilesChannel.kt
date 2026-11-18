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
    private var pendingOneShot = false
    @Volatile private var disposed = false

    init {
        worker.execute { runCatching { removeAbandonedImports() } }
        channel.setMethodCallHandler { call, result ->
            when (call.method) {
                "chooseFiles" -> {
                    if (pending != null) result.error("picker_busy", "The file picker is already open.", null)
                    else {
                        pending = result
                        // A one-shot copy keeps only the picker's transient read grant.
                        pendingOneShot = call.argument<Boolean>("oneShot") == true
                        try {
                            activity.willOpenHistoryFilePicker()
                            activity.startActivityForResult(Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
                                addCategory(Intent.CATEGORY_OPENABLE)
                                type = "*/*"
                                putExtra(Intent.EXTRA_ALLOW_MULTIPLE, !pendingOneShot)
                                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION)
                            }, REQUEST)
                        } catch (_: Exception) {
                            activity.failedToOpenHistoryFilePicker()
                            pending = null
                            result.error("picker_unavailable", "Files could not be selected.", null)
                        }
                    }
                }
                "chooseFolder" -> {
                    if (pending != null) result.error("picker_busy", "The file picker is already open.", null)
                    else {
                        pending = result
                        try {
                            activity.willOpenHistoryFilePicker()
                            activity.startActivityForResult(Intent(Intent.ACTION_OPEN_DOCUMENT_TREE).apply {
                                addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION or Intent.FLAG_GRANT_PERSISTABLE_URI_PERMISSION)
                            }, REQUEST_FOLDER)
                        } catch (_: Exception) {
                            activity.failedToOpenHistoryFilePicker()
                            pending = null
                            result.error("picker_unavailable", "A folder could not be selected.", null)
                        }
                    }
                }
                "materialize" -> {
                    val token = call.argument<String>("token")
                    val maxBytes = call.argument<Number>("maxBytes")?.toLong()
                    val destination = call.argument<String>("destination")
                    worker.execute {
                        var owned: File? = null
                        try {
                            val uri = selections[token] ?: error("Unknown selection")
                            // A caller-owned copy in the cache stays its caller's to delete.
                            val target = destination?.let { File(it).canonicalFile }
                            if (target != null) {
                                require(target.path.startsWith(activity.cacheDir.canonicalPath + File.separator) && target.createNewFile())
                                owned = target
                            }
                            val limit = maxBytes ?: target?.parentFile?.usableSpace
                            require(limit != null && limit > 0)
                            val tooLarge = { size: Long, prefix: String ->
                                FileTooLarge(if (maxBytes == null) activity.getString(R.string.files_read_failed) else "The file is $prefix$size bytes; the file size limit in Settings is $maxBytes bytes.")
                            }
                            activity.contentResolver.query(uri,arrayOf(OpenableColumns.SIZE),null,null,null)?.use { cursor ->
                                if (cursor.moveToFirst() && !cursor.isNull(0)) {
                                    val size=cursor.getLong(0)
                                    if (size > limit) throw tooLarge(size, "")
                                }
                            }
                            val file = target ?: File.createTempFile("history-import-", ".bin", activity.cacheDir).also {
                                staged[token!!] = it
                                activeImports.add(it.absolutePath)
                                val lease = RandomAccessFile(it, "rw")
                                leases[token] = lease
                                lease.channel.lock()
                            }
                            activity.contentResolver.openInputStream(uri).use { input ->
                                requireNotNull(input)
                                val output = if (target != null) RandomAccessFile(target, "rw") else leases.getValue(token!!)
                                try {
                                    val buffer = ByteArray(64 * 1024)
                                    var total = 0L
                                    while (true) {
                                        val count = input.read(buffer)
                                        if (count < 0) break
                                        total += count
                                        if (total > limit) throw tooLarge(total, "at least ")
                                        output.write(buffer, 0, count)
                                    }
                                } finally {
                                    if (target != null) output.close()
                                }
                            }
                            reply { result.success(file.absolutePath) }
                        } catch (error: Exception) {
                            owned?.delete()
                            leases.remove(token)?.close()
                            staged.remove(token)?.let { activeImports.remove(it.absolutePath); it.delete() }
                            if (error is FileTooLarge) reply { result.error(if (maxBytes == null) "insufficient_space" else "file_too_large", error.message, null) }
                            else reply { result.error("file_import_failed", activity.getString(R.string.files_read_failed), null) }
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
        if (request == REQUEST_FOLDER) {
            val result = pending ?: return true
            pending = null
            val tree = data?.data
            if (resultCode != Activity.RESULT_OK || tree == null) {
                result.success(null)
                return true
            }
            // One save folder: the new grant replaces any earlier folder's.
            runCatching {
                val access = Intent.FLAG_GRANT_READ_URI_PERMISSION or Intent.FLAG_GRANT_WRITE_URI_PERMISSION
                activity.contentResolver.persistedUriPermissions
                    .filter { it.isWritePermission && it.uri != tree }
                    .forEach { activity.contentResolver.releasePersistableUriPermission(it.uri, access) }
                activity.contentResolver.takePersistableUriPermission(tree, access)
            }.onSuccess { result.success(tree.toString()) }
                .onFailure { result.error("folder_unavailable", "The folder could not be used.", null) }
            return true
        }
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
        val oneShot = pendingOneShot
        worker.execute {
            val added = mutableListOf<String>()
            try {
                // Reclaim grants only for sources no live clip or pending selection owns.
                val protectedUris = uris.toSet() + selections.values
                if (!oneShot) activity.contentResolver.persistedUriPermissions.forEach { permission ->
                    if (permission.uri !in protectedUris && !permission.isWritePermission && !NativeRuntimeCapture.fileSourceRetained(permission.uri.toString())) {
                        runCatching { activity.contentResolver.releasePersistableUriPermission(permission.uri, Intent.FLAG_GRANT_READ_URI_PERMISSION) }
                    }
                }
                val files = uris.map { uri ->
                    if (!oneShot) activity.contentResolver.takePersistableUriPermission(uri, Intent.FLAG_GRANT_READ_URI_PERMISSION)
                    val metadata = FileReferenceMetadata.read(activity.contentResolver, uri, uri.lastPathSegment?.substringAfterLast('/') ?: "file")
                    val name = metadata.name
                    val size = metadata.size
                    val token = UUID.randomUUID().toString()
                    selections[token] = uri
                    added += token
                    mapOf("token" to token, "name" to name, "mimeType" to (activity.contentResolver.getType(uri) ?: "application/octet-stream"), "uri" to uri.toString(), "sizeBytes" to size)
                }
                reply { result.success(files) }
            } catch (_: Exception) {
                if (oneShot) added.forEach { selections.remove(it) } else selections.clear()
                reply { result.error("file_metadata_failed", "The selected files could not be read.", null) }
            }
        }
        return true
    }

    private fun reply(action: () -> Unit) = activity.runOnUiThread { if (!disposed) action() }

    fun dispose() {
        disposed = true
        channel.setMethodCallHandler(null)
        pending?.error("picker_closed", activity.getString(R.string.files_picker_closed), null)
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

    private class FileTooLarge(message: String) : Exception(message)

    companion object {
        private val activeImports = java.util.concurrent.ConcurrentHashMap.newKeySet<String>()
        private const val REQUEST = 48123
        private const val REQUEST_FOLDER = 48124
    }
}
