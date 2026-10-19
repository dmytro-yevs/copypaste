package com.copypaste.app

import android.content.ClipData
import android.content.ClipDescription
import android.content.ClipboardManager
import android.content.Context
import android.os.PersistableBundle
import android.text.Html
import androidx.core.content.FileProvider
import java.io.File
import java.util.UUID
import java.util.concurrent.atomic.AtomicReference

internal interface ClipboardOwnerReceiptStore {
    fun read(context: Context): String?
    fun write(context: Context, receipt: String?): Boolean
}

private object SharedPreferencesClipboardOwnerReceiptStore : ClipboardOwnerReceiptStore {
    private const val preferences = "clipboard-write-receipt"
    private const val receiptKey = "receipt"

    override fun read(context: Context): String? =
        context.getSharedPreferences(preferences, Context.MODE_PRIVATE).getString(receiptKey, null)

    override fun write(context: Context, receipt: String?): Boolean {
        val editor = context.getSharedPreferences(preferences, Context.MODE_PRIVATE).edit()
        if (receipt == null) editor.remove(receiptKey) else editor.putString(receiptKey, receipt)
        return editor.commit()
    }
}

/** Native clipboard writes shared by manual copy and received clips. */
internal object AndroidClipboardWriter {
    const val label = "CopyPaste"
    private const val receiptExtra = "com.copypaste.app.owner-receipt"
    private val writeLock = Any()
    private val liveReceipt = AtomicReference<String?>(null)

    fun writeText(
        context: Context,
        text: String,
        contentType: String,
        receipts: ClipboardOwnerReceiptStore = SharedPreferencesClipboardOwnerReceiptStore,
    ): Boolean {
        val clip = when (contentType) {
            "text/html" -> ClipData.newHtmlText(
                label, Html.fromHtml(text, Html.FROM_HTML_MODE_LEGACY).toString(), text,
            )
            "text/rtf" -> ClipData(ClipDescription(label, arrayOf(contentType)), ClipData.Item(text))
            else -> ClipData.newPlainText(label, text)
        }
        return write(context, clip, receipts)
    }

    fun writeBinary(context: Context, bytes: ByteArray, filename: String, mimeType: String): Boolean {
        // Each published URI must keep its bytes when another clip arrives.
        var staged: File? = null
        val written = runCatching {
            val directory = File(context.cacheDir, "clipboard/${UUID.randomUUID()}").also { it.mkdirs() }
            val safeName = filename.replace(Regex("[^A-Za-z0-9._-]"), "_").take(100)
            val file = File(directory, safeName.ifBlank { "copypaste" })
            staged = file
            file.writeBytes(bytes)
            val uri = FileProvider.getUriForFile(context, "${context.packageName}.clipboard", file)
            ClipData(ClipDescription(label, arrayOf(mimeType)), ClipData.Item(uri))
        }.getOrNull()?.let { clip -> write(context, clip, SharedPreferencesClipboardOwnerReceiptStore, staged) } == true
        if (!written) staged?.parentFile?.deleteRecursively()
        AndroidClipboardCache.scheduleCleanup(context)
        return written
    }

    fun writeFile(context: Context, path: String, filename: String, mimeType: String): Boolean {
        var staged: File? = null
        val written = runCatching {
            val directory=File(context.cacheDir,"clipboard/${UUID.randomUUID()}").also { it.mkdirs() }
            val safeName=filename.replace(Regex("[^A-Za-z0-9._-]"),"_").take(100).ifBlank {"copypaste"}
            val file=File(directory,safeName); staged=file
            File(path).inputStream().use {input -> file.outputStream().use {output -> input.copyTo(output,64*1024)}}
            val uri=FileProvider.getUriForFile(context,"${context.packageName}.clipboard",file)
            ClipData(ClipDescription(label,arrayOf(mimeType)),ClipData.Item(uri))
        }.getOrNull()?.let {clip -> write(context,clip,SharedPreferencesClipboardOwnerReceiptStore,staged)} == true
        if (!written) staged?.parentFile?.deleteRecursively()
        AndroidClipboardCache.scheduleCleanup(context)
        return written
    }

    fun isOwned(
        context: Context,
        description: ClipDescription,
        receipts: ClipboardOwnerReceiptStore = SharedPreferencesClipboardOwnerReceiptStore,
    ): Boolean {
        if (description.label?.toString() != label) return false
        val receipt = description.extras?.getString(receiptExtra) ?: return false
        return liveReceipt.get() == receipt || storedReceipt(context, receipts) == receipt
    }

    private fun write(context: Context, clip: ClipData, receipts: ClipboardOwnerReceiptStore, file: File? = null): Boolean = synchronized(writeLock) {
        val previousLive = liveReceipt.get()
        val previousStored = storedReceipt(context, receipts)
        val receipt = UUID.randomUUID().toString()
        clip.description.extras = PersistableBundle().apply { putString(receiptExtra, receipt) }
        liveReceipt.set(receipt)
        // A receipt only recognizes our own writes. It must never decide whether
        // the user-visible clipboard write itself succeeds.
        storeReceipt(context, receipts, receipt)
        try {
            AndroidClipboardCache.publish(context, file) { clipboard(context).setPrimaryClip(clip) }
            AndroidClipboardCache.scheduleCleanup(context)
            true
        } catch (_: RuntimeException) {
            liveReceipt.set(previousLive)
            storeReceipt(context, receipts, previousStored)
            false
        }
    }

    private fun storedReceipt(context: Context, receipts: ClipboardOwnerReceiptStore): String? =
        runCatching { receipts.read(context) }.getOrNull()

    private fun storeReceipt(context: Context, receipts: ClipboardOwnerReceiptStore, receipt: String?) {
        runCatching { receipts.write(context, receipt) }
    }

    private fun clipboard(context: Context) =
        context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
}
