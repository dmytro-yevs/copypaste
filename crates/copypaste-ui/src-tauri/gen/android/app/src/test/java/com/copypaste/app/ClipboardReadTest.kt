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
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import org.robolectric.shadows.ShadowContentResolver
import java.io.File

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
    fun grantedContentUriJpegIsNormalisedToPng() {
        val provider = FixtureBinaryProvider(context, imageBytes(Bitmap.CompressFormat.JPEG), "image/jpeg")
        ShadowContentResolver.registerProviderInternal(FIXTURE_AUTHORITY, provider)
        val uri = Uri.parse("content://$FIXTURE_AUTHORITY/receipt.png")
        context.grantUriPermission(context.packageName, uri, Intent.FLAG_GRANT_READ_URI_PERMISSION)
        clipboard.setPrimaryClip(clip("image/jpeg", ClipData.Item(uri)))

        val read = clipboardRead(context, CaptureSource.IN_APP)

        assertEquals(ReadOutcome.SUCCEEDED, read.outcome)
        assertEquals("image/png", read.clip?.contentType)
        assertTrue(read.clip?.bytesBase64?.startsWith("iVBOR") == true)
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
