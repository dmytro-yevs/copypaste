package com.copypaste.app

import android.content.ClipboardManager
import android.content.ClipData
import android.content.Context
import androidx.core.content.FileProvider
import java.io.File
import org.junit.Assert.*
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class AndroidClipboardWriterTest {
    @Before fun initializeProvider() = initializeClipboardTestProvider()

    @Test fun sensitivitySurvivesNativeWritesWithoutLosingOwnershipOrRepresentations() {
        val context = RuntimeEnvironment.getApplication()
        val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        fun assertSensitive(expected: Boolean) {
            val description = clipboard.primaryClipDescription!!
            assertEquals(expected, description.extras!!.getBoolean("android.content.extra.IS_SENSITIVE"))
            assertTrue(AndroidClipboardWriter.isOwned(context, description))
        }
        assertTrue(AndroidClipboardWriter.writeText(context, "<b>synthetic secret</b>", "text/html", secret = true))
        assertSensitive(true)
        assertEquals("<b>synthetic secret</b>", clipboard.primaryClip!!.getItemAt(0).htmlText)
        assertTrue(AndroidClipboardWriter.writeBinary(context, byteArrayOf(1, 2), "secret.png", "image/png", secret = true))
        assertSensitive(true)
        val original = File(context.filesDir, "copypaste-downloads/secret.pdf")
        original.parentFile!!.mkdirs()
        original.writeBytes(byteArrayOf(3, 4))
        val uri = FileProvider.getUriForFile(context, "${context.packageName}.clipboard", original)
        assertTrue(AndroidClipboardWriter.writeFile(context, uri.toString(), "secret.pdf", "application/pdf", secret = true))
        assertSensitive(true)
        assertEquals(uri, clipboard.primaryClip!!.getItemAt(0).uri)
        assertTrue(AndroidClipboardWriter.writeText(context, "ordinary", "text"))
        assertSensitive(false)
    }

    @Test fun fileCopyPublishesTheOriginalReferenceWithoutMaterializingAnotherBody() {
        val context = RuntimeEnvironment.getApplication()
        val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        val original = File(context.filesDir, "copypaste-downloads/original.pdf")
        original.parentFile!!.mkdirs()
        original.writeBytes(byteArrayOf(1, 2, 3, 4))
        val uri = FileProvider.getUriForFile(context, "${context.packageName}.clipboard", original)
        assertTrue(AndroidClipboardWriter.writeFile(context, uri.toString(), "original.pdf", "application/pdf"))
        assertEquals(uri, clipboard.primaryClip!!.getItemAt(0).uri)
        assertNull(clipboard.primaryClip!!.getItemAt(0).text)
        assertTrue(clipboard.primaryClipDescription!!.hasMimeType("application/pdf"))
        assertTrue(File(context.cacheDir, "clipboard").listFiles().orEmpty().none { it.isDirectory })
        assertTrue(AndroidClipboardWriter.writeFile(context, original.absolutePath, "original.pdf", "application/pdf"))
        assertArrayEquals(original.readBytes(), context.contentResolver.openInputStream(clipboard.primaryClip!!.getItemAt(0).uri)!!.use { it.readBytes() })
        assertTrue(File(context.cacheDir, "clipboard").listFiles().orEmpty().none { it.isDirectory })
    }

    private object FailingReceiptStore : ClipboardOwnerReceiptStore {
        override fun read(context: Context): String? = null
        override fun write(context: Context, receipt: String?): Boolean = false
    }

    private object ThrowingReceiptStore : ClipboardOwnerReceiptStore {
        override fun read(context: Context): String? = error("storage unavailable")
        override fun write(context: Context, receipt: String?): Boolean = error("storage unavailable")
    }

    @Test fun textHtmlAndRtfKeepTheirNativeRepresentationsAndSelfWriteLabel() {
        val context = RuntimeEnvironment.getApplication()
        val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        assertTrue(AndroidClipboardWriter.writeText(context, "original text", "text"))
        assertEquals("original text", clipboard.primaryClip!!.getItemAt(0).text.toString())
        assertTrue(AndroidClipboardWriter.writeText(context, "<b>bold</b>", "text/html"))
        assertEquals("<b>bold</b>", clipboard.primaryClip!!.getItemAt(0).htmlText)
        assertTrue(clipboard.primaryClipDescription!!.hasMimeType("text/html"))
        val rtf = "{\\rtf1 original}"
        assertTrue(AndroidClipboardWriter.writeText(context, rtf, "text/rtf"))
        assertEquals(rtf, clipboard.primaryClip!!.getItemAt(0).text.toString())
        assertTrue(clipboard.primaryClipDescription!!.hasMimeType("text/rtf"))
        assertEquals(AndroidClipboardWriter.label, clipboard.primaryClipDescription!!.label)
    }

    @Test fun onlyAnOwnedReceiptSuppressesCopyPasteLabelledContent() {
        val context = RuntimeEnvironment.getApplication()
        val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        val foreign = ClipData.newPlainText(AndroidClipboardWriter.label, "foreign copy")
        clipboard.setPrimaryClip(foreign)
        assertFalse(AndroidClipboardWriter.isOwned(context, clipboard.primaryClipDescription!!))

        assertTrue(AndroidClipboardWriter.writeText(context, "owned copy", "text/plain"))
        assertTrue(AndroidClipboardWriter.isOwned(context, clipboard.primaryClipDescription!!))
    }

    @Test fun receiptPersistenceFailureDoesNotRejectOrForgetAnOwnedWrite() {
        val context = RuntimeEnvironment.getApplication()
        val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        assertTrue(
            AndroidClipboardWriter.writeText(
                context,
                "clipboard copy survives receipt failure",
                "text/plain",
                FailingReceiptStore,
            ),
        )
        assertEquals(
            "clipboard copy survives receipt failure",
            clipboard.primaryClip!!.getItemAt(0).text.toString(),
        )
        assertTrue(
            AndroidClipboardWriter.isOwned(
                context,
                clipboard.primaryClipDescription!!,
                FailingReceiptStore,
            ),
        )
    }

    @Test fun receiptStorageExceptionsDoNotRejectOrForgetAnOwnedWrite() {
        val context = RuntimeEnvironment.getApplication()
        val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        assertTrue(
            AndroidClipboardWriter.writeText(
                context,
                "clipboard copy survives receipt exception",
                "text/plain",
                ThrowingReceiptStore,
            ),
        )
        assertEquals(
            "clipboard copy survives receipt exception",
            clipboard.primaryClip!!.getItemAt(0).text.toString(),
        )
        assertTrue(
            AndroidClipboardWriter.isOwned(
                context,
                clipboard.primaryClipDescription!!,
                ThrowingReceiptStore,
            ),
        )
        assertFalse(
            AndroidClipboardWriter.isOwned(
                context,
                ClipData.newPlainText(AndroidClipboardWriter.label, "foreign").description,
                ThrowingReceiptStore,
            ),
        )
    }

    @Test fun imagesAndFilesKeepTheirMimeAndOriginalBytesAcrossLaterWrites() {
        val context = RuntimeEnvironment.getApplication()
        val clipboard = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager
        val original = byteArrayOf(0, 1, -1, 42)
        assertTrue(AndroidClipboardWriter.writeBinary(context, original, "image.png", "image/png"))
        val imageUri = clipboard.primaryClip!!.getItemAt(0).uri
        assertTrue(clipboard.primaryClipDescription!!.hasMimeType("image/png"))
        assertTrue(AndroidClipboardWriter.writeBinary(context, byteArrayOf(9, 8), "image.png", "application/pdf"))
        val fileUri = clipboard.primaryClip!!.getItemAt(0).uri
        assertNotEquals(imageUri, fileUri)
        assertTrue(clipboard.primaryClipDescription!!.hasMimeType("application/pdf"))
        assertArrayEquals(original, context.contentResolver.openInputStream(imageUri)!!.use { it.readBytes() })
        assertArrayEquals(byteArrayOf(9, 8), context.contentResolver.openInputStream(fileUri)!!.use { it.readBytes() })
    }
}
