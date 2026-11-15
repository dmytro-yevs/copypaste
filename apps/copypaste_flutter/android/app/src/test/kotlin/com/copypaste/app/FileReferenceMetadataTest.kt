package com.copypaste.app

import android.content.ContentProvider
import android.content.ContentValues
import android.database.Cursor
import android.database.MatrixCursor
import android.net.Uri
import android.os.ParcelFileDescriptor
import android.provider.OpenableColumns
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.RuntimeEnvironment
import org.robolectric.annotation.Config
import org.robolectric.shadows.ShadowContentResolver

@RunWith(RobolectricTestRunner::class)
@Config(sdk = [36])
class FileReferenceMetadataTest {
    @Test fun largeClipboardFileUsesOnlyProviderMetadata() {
        val provider = MetadataProvider()
        ShadowContentResolver.registerProviderInternal("reference-test", provider)
        val resolver = RuntimeEnvironment.getApplication().contentResolver
        val uri = Uri.parse("content://reference-test/archive")
        assertEquals(FileReferenceMetadata("archive.bin", 15L * 1024 * 1024 * 1024), FileReferenceMetadata.read(resolver, uri, "fallback"))
        provider.size = null
        assertNull(FileReferenceMetadata.read(resolver, uri, "fallback").size)
        assertEquals(0, provider.opens)
    }

    private class MetadataProvider : ContentProvider() {
        var size: Long? = 15L * 1024 * 1024 * 1024
        var opens = 0
        override fun onCreate() = true
        override fun query(uri: Uri, projection: Array<out String>?, selection: String?, selectionArgs: Array<out String>?, sortOrder: String?): Cursor =
            MatrixCursor(arrayOf(OpenableColumns.DISPLAY_NAME, OpenableColumns.SIZE)).apply { addRow(arrayOf<Any?>("archive.bin", size)) }
        override fun openFile(uri: Uri, mode: String): ParcelFileDescriptor {
            opens++
            throw AssertionError("File registration must never open the body")
        }
        override fun getType(uri: Uri) = "application/octet-stream"
        override fun insert(uri: Uri, values: ContentValues?): Uri? = null
        override fun delete(uri: Uri, selection: String?, selectionArgs: Array<out String>?) = 0
        override fun update(uri: Uri, values: ContentValues?, selection: String?, selectionArgs: Array<out String>?) = 0
    }
}
