package com.copypaste.app

import android.content.ClipData
import android.content.ClipboardManager
import android.content.ContentProvider
import android.content.ContentValues
import android.content.Context
import android.database.Cursor
import android.net.Uri
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

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [29])
class ClipboardReadTest {
    private val context: Context get() = RuntimeEnvironment.getApplication()
    private val clipboard: ClipboardManager
        get() = context.getSystemService(Context.CLIPBOARD_SERVICE) as ClipboardManager

    @After
    fun clearClipboard() {
        clipboard.clearPrimaryClip()
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
        assertNull(read.text)
    }

    @Test
    fun explicitTextIsCaptured() {
        clipboard.setPrimaryClip(ClipData.newPlainText("text", "genuine text"))

        val read = clipboardRead(context, CaptureSource.IN_APP)

        assertEquals(ReadOutcome.SUCCEEDED, read.outcome)
        assertEquals("genuine text", read.text)
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
        assertEquals("caption", read.text)
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
        assertNull(read.text)
        assertFalse(provider.streamTypesRequested)
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

    private companion object {
        const val HOSTILE_AUTHORITY = "binary.example"
    }
}
