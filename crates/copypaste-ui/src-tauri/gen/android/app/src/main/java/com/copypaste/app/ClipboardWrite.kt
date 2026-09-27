package com.copypaste.app

import android.content.ClipData
import android.content.ClipDescription
import android.content.ClipboardManager
import android.content.Context
import android.util.Base64
import android.webkit.MimeTypeMap
import androidx.core.content.FileProvider
import kotlinx.serialization.Serializable
import java.io.File

@Serializable
data class ClipboardWriteRequest(val bytesBase64: String, val contentType: String, val filename: String)

private const val STAGING_MAX_FILES = 8
private const val STAGING_MAX_AGE_MS = 60_000L

internal fun writeBinaryClipboard(context: Context, request: ClipboardWriteRequest): Boolean {
    val maximumBase64 = ((ClipQueue.MAX_BINARY_BYTES + 2) / 3) * 4
    if (request.bytesBase64.length > maximumBase64) return false
    val filename = filenameForMime(request.filename, request.contentType) ?: return false
    val bytes = try { Base64.decode(request.bytesBase64, Base64.DEFAULT) } catch (_: IllegalArgumentException) { return false }
    if (bytes.isEmpty() || bytes.size > ClipQueue.MAX_BINARY_BYTES) return false
    val directory = File(context.cacheDir, "clipboard-staging")
    val file = try {
        if (!directory.exists() && !directory.mkdirs()) return false
        purgeClipboardStaging(directory)
        File.createTempFile("clipboard-", "-$filename", directory).also { file ->
            file.outputStream().use { output -> output.write(bytes) }
        }
    } catch (_: Exception) { return false }
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

internal fun purgeClipboardStaging(directory: File, now: Long = System.currentTimeMillis()) {
    directory.listFiles()?.sortedBy { it.lastModified() }?.forEachIndexed { index, file ->
        if (now - file.lastModified() > STAGING_MAX_AGE_MS || index < (directory.listFiles()?.size ?: 0) - STAGING_MAX_FILES) file.delete()
    }
}

private fun filenameForMime(value: String, mime: String): String? {
    val base = value.substringAfterLast('/').substringAfterLast('\\').takeIf { it.isNotBlank() && it.length <= 255 } ?: return null
    val extension = MimeTypeMap.getSingleton().getExtensionFromMimeType(mime) ?: return null
    val stem = base.substringBeforeLast('.', base)
    return "$stem.$extension"
}
