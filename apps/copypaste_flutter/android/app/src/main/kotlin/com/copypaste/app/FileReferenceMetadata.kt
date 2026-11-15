package com.copypaste.app

import android.content.ContentResolver
import android.net.Uri
import android.provider.OpenableColumns

/** File selection and clipboard capture query metadata without opening a body. */
internal data class FileReferenceMetadata(val name: String, val size: Long?) {
    companion object {
        fun read(resolver: ContentResolver, uri: Uri, fallbackName: String): FileReferenceMetadata {
            var name = fallbackName
            var size: Long? = null
            resolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME, OpenableColumns.SIZE), null, null, null)?.use { cursor ->
                if (cursor.moveToFirst()) {
                    val nameColumn = cursor.getColumnIndex(OpenableColumns.DISPLAY_NAME)
                    val sizeColumn = cursor.getColumnIndex(OpenableColumns.SIZE)
                    if (nameColumn >= 0 && !cursor.isNull(nameColumn)) name = cursor.getString(nameColumn)
                    if (sizeColumn >= 0 && !cursor.isNull(sizeColumn)) size = cursor.getLong(sizeColumn).takeIf { it >= 0 }
                }
            }
            return FileReferenceMetadata(name, size)
        }
    }
}
