package com.copypaste.android

data class ClipboardItem(
    val id: String,
    val contentType: String,
    val isSensitive: Boolean,
    val wallTimeMs: Long,
    val snippet: String = "",
    /**
     * Raw PNG/JPEG bytes of the image thumbnail, non-null only when [contentType]
     * is an image MIME type (e.g. "image/png", "image/jpeg").
     *
     * Populated by [ClipboardRepository.getItems] when image data is stored
     * under the separate "item_img_<id>" SharedPreferences key. Images are kept
     * out of the main pipe-delimited item blob to avoid ballooning the index
     * string. When null the row shows a generic image-type icon instead.
     */
    val imagePng: ByteArray? = null,
    /**
     * True when the user has explicitly pinned this item. Pinned items are:
     *  - never pruned by the retention/quota pass
     *  - cleared only by the explicit [ClipboardRepository.clearAll] user action
     *
     * Persisted in the "pinned_ids" SharedPreferences key as a comma-joined ordered list.
     * Populated by [ClipboardRepository.getItems].
     */
    val pinned: Boolean = false,
    /**
     * Position of this item within the pinned section (0 = top).
     * -1 for unpinned items. Used to sort pinned items in user-defined order
     * rather than by recency. Populated by [ClipboardRepository.getItems].
     */
    val pinnedSortIndex: Int = -1,
    /**
     * Source application package name or macOS bundle id that produced this
     * clipboard item, e.g. "com.android.chrome" or "com.google.Chrome".
     * Null when unknown (older items, synced items from another device, etc.).
     * Display via [sourceAppLabel] to get a short human-readable name.
     */
    val sourceApp: String? = null,
) {
    /** True when this item carries an image payload that can be rendered as a thumbnail. */
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
            pinned == other.pinned &&
            sourceApp == other.sourceApp
    }

    override fun hashCode(): Int {
        var result = id.hashCode()
        result = 31 * result + contentType.hashCode()
        result = 31 * result + isSensitive.hashCode()
        result = 31 * result + wallTimeMs.hashCode()
        result = 31 * result + snippet.hashCode()
        result = 31 * result + (imagePng?.contentHashCode() ?: 0)
        result = 31 * result + pinned.hashCode()
        result = 31 * result + (sourceApp?.hashCode() ?: 0)
        return result
    }
}

/**
 * Derive a short human-readable label from a bundle/package id.
 * "com.google.chrome" → "Chrome", "com.apple.Safari" → "Safari".
 * Returns null when the id is null or blank.
 */
fun sourceAppLabel(bundleId: String?): String? {
    if (bundleId.isNullOrBlank()) return null
    val last = bundleId.split(".").lastOrNull() ?: return null
    return last.replaceFirstChar { it.uppercaseChar() }
}

/** Returns null-safe contentEquals for nullable ByteArrays. */
private fun ByteArray?.contentEquals(other: ByteArray?): Boolean =
    if (this == null && other == null) true
    else if (this == null || other == null) false
    else this.contentEquals(other)
