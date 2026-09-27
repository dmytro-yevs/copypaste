package com.copypaste.app

import android.content.ClipData
import android.content.ClipboardManager
import android.content.ContentProvider
import android.content.ContentValues
import android.content.Context
import android.content.Intent
import android.database.Cursor
import android.graphics.Bitmap
import android.net.Uri
import android.os.ParcelFileDescriptor
import android.util.Base64
import android.os.Looper
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import org.robolectric.shadows.ShadowContentResolver
import java.io.File
import java.util.concurrent.TimeUnit
import org.robolectric.Shadows.shadowOf

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [29])
class ClipboardReadTest {
    private val context: Context get() = RuntimeEnvironment.getApplication()
    private val clipboard: ClipboardManager
        get() = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager

    @After
    fun clearClipboard() {
        clipboard.clearPrimaryClip()
        CaptureExclusions.replace(false, emptyList())
    }

    @Test
    fun imageOnlyClipIsAcknowledgedWithoutText() {
        clipboard.setPrimaryClip(
            clip(
                "image/png",
                ClipData.Item(Uri.parse("content://camera.example/capture.png")),
            ),
        )

        val read = clipboardRead(context, CaptureSource.IN_APP)

        assertEquals(ReadOutcome.EMPTY, read.outcome)
        assertNull(read.clip)
    }

    @Test
    fun explicitTextIsCaptured() {
        clipboard.setPrimaryClip(ClipData.newPlainText("text", "genuine text"))

        val read = clipboardRead(context, CaptureSource.IN_APP)

        assertEquals(ReadOutcome.SUCCEEDED, read.outcome)
        assertEquals("genuine text", read.clip?.text)
    }

    @Test
    fun configuredExclusionsFailClosedWithoutRuntimeSourceAttribution() {
        CaptureExclusions.replace(true, listOf("com.password.manager"))
        clipboard.setPrimaryClip(ClipData.newPlainText("text", "genuine text"))

        val read = clipboardRead(context, CaptureSource.BACKGROUND)

        assertEquals(ReadOutcome.EMPTY, read.outcome)
        assertNull(read.clip)
    }

    @Test
    fun explicitTextIsPreservedWhenTheItemAlsoHasAUri() {
        val item = ClipData.Item(
            "caption",
            null,
            null,
            Uri.parse("content://camera.example/capture.png"),
        )
        clipboard.setPrimaryClip(clip("text/plain", item))

        val read = clipboardRead(context, CaptureSource.IN_APP)

        assertEquals(ReadOutcome.SUCCEEDED, read.outcome)
        assertEquals("caption", read.clip?.text)
    }

    @Test
    fun binaryUriIsNeverOpenedOrCoercedToText() {
        val provider = HostileBinaryProvider()
        ShadowContentResolver.registerProviderInternal(HOSTILE_AUTHORITY, provider)
        clipboard.setPrimaryClip(
            clip(
                "image/png",
                ClipData.Item(Uri.parse("content://$HOSTILE_AUTHORITY/payload")),
            ),
        )

        val read = clipboardRead(context, CaptureSource.IN_APP)

        assertEquals(ReadOutcome.EMPTY, read.outcome)
        assertNull(read.clip)
        assertFalse(provider.streamTypesRequested)
    }

    @Test
    fun jpegFixtureIsNormalisedToPng() {
        val png = normaliseImage(imageBytes(Bitmap.CompressFormat.JPEG))

        assertTrue(png?.copyOfRange(0, 8)?.contentEquals(PNG_SIGNATURE) == true)
    }

    @Test
    fun decodedImageBudgetRejectsDimensionsBeforePixelDecode() {
        assertTrue(exceedsDecodedImageBudget(5_000, 5_000))
    }

    @Test
    fun binaryWritePublishesReadableUriWithItsDeclaredMime() {
        val bytes = byteArrayOf(0x89.toByte(), 0x50, 0x4e, 0x47)
        assertTrue(writeBinaryClipboard(
            context,
            ClipboardWriteRequest(Base64.encodeToString(bytes, Base64.NO_WRAP), "image/png", "photo.png"),
        ))

        val uri = clipboard.primaryClip!!.getItemAt(0).uri
        assertEquals("image/png", context.contentResolver.getType(uri))
        assertTrue(context.contentResolver.openInputStream(uri)!!.use { it.readBytes().contentEquals(bytes) })
    }

    @Test
    fun startupPurgesStaleClipboardStaging() {
        val directory = stagingDirectory(context).apply { mkdirs() }
        val stale = File(directory, "stale.png").apply {
            writeBytes(byteArrayOf(1))
            setLastModified(System.currentTimeMillis() - STAGING_MAX_AGE_MS - 1)
        }

        ClipboardStaging.initialize(context, android.os.Handler(Looper.getMainLooper()))

        assertFalse(stale.exists())
    }

    @Test
    fun scheduledExpiryPurgesWithoutAnotherCopy() {
        val directory = stagingDirectory(context).apply { mkdirs() }
        val stale = File(directory, "later.png").apply {
            writeBytes(byteArrayOf(1))
            setLastModified(System.currentTimeMillis() - STAGING_MAX_AGE_MS - 1)
        }
        val handler = android.os.Handler(Looper.getMainLooper())
        ClipboardStaging.schedule(context, handler)

        shadowOf(Looper.getMainLooper()).idleFor(STAGING_MAX_AGE_MS + 1, TimeUnit.MILLISECONDS)

        assertFalse(stale.exists())
        ClipboardStaging.stop(handler)
    }

    @Test
    fun stagingCapacityIsExactlyEightFiles() {
        val directory = stagingDirectory(context).apply { mkdirs() }
        repeat(9) { index -> File(directory, "$index.png").writeBytes(byteArrayOf(1)) }

        purgeClipboardStaging(directory)

        assertEquals(8, directory.listFiles()!!.size)
    }

    private fun clip(mimeType: String, item: ClipData.Item): ClipData =
        ClipData("clip", arrayOf(mimeType), item)

    private class HostileBinaryProvider : ContentProvider() {
        var streamTypesRequested = false

        override fun onCreate(): Boolean = true

        override fun getType(uri: Uri): String = "image/png"

        override fun getStreamTypes(uri: Uri, mimeTypeFilter: String): Array<String> {
            streamTypesRequested = true
            throw AssertionError("binary clipboard URI was requested as text")
        }

        override fun query(
            uri: Uri,
            projection: Array<out String>?,
            selection: String?,
            selectionArgs: Array<out String>?,
            sortOrder: String?,
        ): Cursor? = null

        override fun insert(uri: Uri, values: ContentValues?): Uri? = null

        override fun delete(uri: Uri, selection: String?, selectionArgs: Array<out String>?): Int = 0

        override fun update(
            uri: Uri,
            values: ContentValues?,
            selection: String?,
            selectionArgs: Array<out String>?,
        ): Int = 0
    }

    private class FixtureBinaryProvider(
        private val context: Context,
        private val bytes: ByteArray,
        private val mimeType: String,
    ) : ContentProvider() {
        override fun onCreate(): Boolean = true

        override fun getType(uri: Uri): String = mimeType

        override fun openFile(uri: Uri, mode: String): ParcelFileDescriptor {
            val fixture = File(context.cacheDir, "clipboard-image-fixture")
            fixture.writeBytes(bytes)
            return ParcelFileDescriptor.open(fixture, ParcelFileDescriptor.MODE_READ_ONLY)
        }

        override fun query(
            uri: Uri,
            projection: Array<out String>?,
            selection: String?,
            selectionArgs: Array<out String>?,
            sortOrder: String?,
        ): Cursor? = null

        override fun insert(uri: Uri, values: ContentValues?): Uri? = null

        override fun delete(uri: Uri, selection: String?, selectionArgs: Array<out String>?): Int = 0

        override fun update(
            uri: Uri,
            values: ContentValues?,
            selection: String?,
            selectionArgs: Array<out String>?,
        ): Int = 0
    }

    private companion object {
        const val HOSTILE_AUTHORITY = "binary.example"
        const val FIXTURE_AUTHORITY = "fixture.example"
        val PNG_SIGNATURE = byteArrayOf(0x89.toByte(), 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a)

        fun imageBytes(format: Bitmap.CompressFormat): ByteArray {
            val bitmap = Bitmap.createBitmap(2, 2, Bitmap.Config.ARGB_8888)
            return try {
                java.io.ByteArrayOutputStream().use { output ->
                    check(bitmap.compress(format, 90, output))
                    output.toByteArray()
                }
            } finally {
                bitmap.recycle()
            }
        }
    }
}
