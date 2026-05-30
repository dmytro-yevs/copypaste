package com.copypaste.android

data class ClipboardItem(
    val id: String,
    val contentType: String,
    val isSensitive: Boolean,
    val wallTimeMs: Long,
    val snippet: String = "",
    /** Raw PNG/JPEG bytes for image items; null for text items. */
    val imagePng: ByteArray? = null,
    /** True when this item is pinned (survives prune passes). */
    val pinned: Boolean = false,
) {
    /** True when the content type signals an image MIME type. */
    val isImage: Boolean get() = contentType.startsWith("image/") || contentType == "image"

    // ByteArray in a data class requires manual equals/hashCode to avoid identity comparison.
    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is ClipboardItem) return false
        return id == other.id &&
            contentType == other.contentType &&
            isSensitive == other.isSensitive &&
            wallTimeMs == other.wallTimeMs &&
            snippet == other.snippet &&
            imagePng.contentEquals(other.imagePng) &&
            pinned == other.pinned
    }

    override fun hashCode(): Int {
        var result = id.hashCode()
        result = 31 * result + contentType.hashCode()
        result = 31 * result + isSensitive.hashCode()
        result = 31 * result + wallTimeMs.hashCode()
        result = 31 * result + snippet.hashCode()
        result = 31 * result + (imagePng?.contentHashCode() ?: 0)
        result = 31 * result + pinned.hashCode()
        return result
    }
}

/** Returns null-safe contentEquals for nullable ByteArrays. */
private fun ByteArray?.contentEquals(other: ByteArray?): Boolean =
    if (this == null && other == null) true
    else if (this == null || other == null) false
    else this.contentEquals(other)
