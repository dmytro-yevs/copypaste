package com.copypaste.app

import android.content.ClipData
import android.content.ClipDescription
import android.content.ClipboardManager
import android.content.Context
import android.text.Html
import androidx.core.content.FileProvider
import java.io.File
import java.util.UUID

/** Native clipboard writes shared by manual copy and received clips. */
internal object AndroidClipboardWriter {
    const val label = "CopyPaste"

    fun writeText(context: Context, text: String, contentType: String): Boolean = runCatching {
        val clip = when (contentType) {
            "text/html" -> ClipData.newHtmlText(
                label, Html.fromHtml(text, Html.FROM_HTML_MODE_LEGACY).toString(), text,
            )
            "text/rtf" -> ClipData(ClipDescription(label, arrayOf(contentType)), ClipData.Item(text))
            else -> ClipData.newPlainText(label, text)
        }
        clipboard(context).setPrimaryClip(clip)
    }.isSuccess

    fun writeBinary(context: Context, bytes: ByteArray, filename: String, mimeType: String): Boolean = runCatching {
        // Each published URI must keep its bytes when another clip arrives.
        val directory = File(context.cacheDir, "clipboard/${UUID.randomUUID()}").also { it.mkdirs() }
        val safeName = filename.replace(Regex("[^A-Za-z0-9._-]"), "_").take(100)
        val file = File(directory, safeName.ifBlank { "copypaste" })
        file.writeBytes(bytes)
        val uri = FileProvider.getUriForFile(context, "${context.packageName}.clipboard", file)
        clipboard(context).setPrimaryClip(
            ClipData(ClipDescription(label, arrayOf(mimeType)), ClipData.Item(uri)),
        )
    }.isSuccess

    private fun clipboard(context: Context) =
        context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
}
