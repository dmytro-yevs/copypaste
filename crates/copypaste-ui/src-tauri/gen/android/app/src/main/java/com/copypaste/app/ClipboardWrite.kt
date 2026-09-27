package com.copypaste.app

import android.content.ClipData
import android.content.ClipDescription
import android.content.ClipboardManager
import android.content.Context
import android.os.Handler
import android.util.Base64
import android.webkit.MimeTypeMap
import androidx.core.content.FileProvider
import java.io.File

private const val STAGING_MAX_FILES = 8
internal const val STAGING_MAX_AGE_MS = 10 * 60 * 1000L

internal object ClipboardStaging {
    private var expiry: Runnable? = null

    fun initialize(context: Context, handler: Handler) {
        purgeClipboardStaging(stagingDirectory(context))
        schedule(context, handler)
    }

    fun schedule(context: Context, handler: Handler) {
        expiry?.let(handler::removeCallbacks)
        val application = context.applicationContext
        expiry = Runnable { purgeClipboardStaging(stagingDirectory(application)) }.also {
            handler.postDelayed(it, STAGING_MAX_AGE_MS)
        }
    }

    fun stop(handler: Handler) {
        expiry?.let(handler::removeCallbacks)
        expiry = null
    }
}

internal fun writeBinaryClipboard(context: Context, request: ClipboardWriteRequest): Boolean {
    val maximumBase64 = ((ClipQueue.MAX_BINARY_BYTES + 2) / 3) * 4
    if (request.bytesBase64.length > maximumBase64) return false
    val filename = filenameForMime(request.filename, request.contentType) ?: return false
    val bytes = try { Base64.decode(request.bytesBase64, Base64.DEFAULT) } catch (_: IllegalArgumentException) { return false }
    if (bytes.isEmpty() || bytes.size > ClipQueue.MAX_BINARY_BYTES) return false
    val directory = stagingDirectory(context)
    var staged: File? = null
    val file = try {
        if (!directory.exists() && !directory.mkdirs()) return false
        purgeClipboardStaging(directory, maximumFiles = STAGING_MAX_FILES - 1)
        File.createTempFile("clipboard-", "-$filename", directory).also { file ->
            staged = file
            file.outputStream().use { output -> output.write(bytes) }
        }
    } catch (_: Exception) { staged?.delete(); return false }
    return try {
        val uri = FileProvider.getUriForFile(context, "${context.packageName}.fileprovider", file)
        if (context.contentResolver.getType(uri) != request.contentType) {
            file.delete()
            return false
        }
        val clip = ClipData(ClipDescription(filename, arrayOf(request.contentType)), ClipData.Item(uri))
        (context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager).setPrimaryClip(clip)
        true
    } catch (_: SecurityException) { file.delete(); false } catch (_: IllegalArgumentException) { file.delete(); false }
}

internal fun purgeClipboardStaging(directory: File, now: Long = System.currentTimeMillis(), maximumFiles: Int = STAGING_MAX_FILES) {
    val files = directory.listFiles()?.sortedBy { it.lastModified() } ?: return
    files.forEachIndexed { index, file ->
        if (now - file.lastModified() > STAGING_MAX_AGE_MS || index < files.size - maximumFiles) file.delete()
    }
}

internal fun stagingDirectory(context: Context) = File(context.cacheDir, "clipboard-staging")

private fun filenameForMime(value: String, mime: String): String? {
    val base = value.substringAfterLast('/').substringAfterLast('\\').takeIf { it.isNotBlank() && it.length <= 255 } ?: return null
    val extension = MimeTypeMap.getSingleton().getExtensionFromMimeType(mime) ?: return null
    val stem = base.substringBeforeLast('.', base)
    return "$stem.$extension"
}
