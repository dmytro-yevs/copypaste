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
     *  - preserved by [ClipboardRepository.clearAll] (only unpinned items deleted)
     *
     * Persisted in the "pinned_ids" SharedPreferences key as a comma-joined ordered
     * list (first = top of pinned section). Populated by [ClipboardRepository.getItems].
     */
    val pinned: Boolean = false,
    /**
     * Position of this item within the pinned section (0 = top of pinned section).
     * -1 for unpinned items. Used to keep pinned items in user-defined order
     * independent of [wallTimeMs] so copying a pinned item does not move it.
     * Populated by [ClipboardRepository.getItems].
     */
    val pinnedSortIndex: Int = -1,
    /**
     * Source application package name or macOS bundle id that produced this
     * clipboard item, e.g. "com.android.chrome" or "com.google.Chrome".
     * Null when unknown (older items, synced items from another device, etc.).
     * Display via [sourceAppLabel] to get a short human-readable name.
     */
    val sourceApp: String? = null,
    /**
     * True when this item's stored payload exceeds the sync size ceiling
     * ([ClipboardRepository.SYNC_MAX_BLOB_BYTES], 8 MiB) and therefore will not be
     * propagated to other devices. Unlike macOS — which receives this flag from the
     * daemon over IPC — Android has no daemon and computes it locally in
     * [ClipboardRepository] from the item's own stored byte size against the same
     * 8 MiB ceiling the sync pipeline enforces. Drives the "won't sync — too large"
     * badge in the history row. Defaults to false for back-compat.
     */
    val tooLargeToSync: Boolean = false,
    /**
     * UUID of the device that originally captured this clipboard item.
     * Null for legacy items captured before this field was added, or for
     * items that arrived via sync without a device-id tag.
     *
     * Used by the origin-device filter strip (macOS HistoryView parity):
     * shown when more than one origin device is present, lets the user
     * filter the list to items from a specific device.
     */
    val originDeviceId: String? = null,
) {
    /** True when this item carries an image payload that can be rendered as a thumbnail. */
    val isImage: Boolean get() = contentType.startsWith("image/") || contentType == "image"

    /** True when this item is a synced file (content_type == "file"). */
    val isFile: Boolean get() = contentType == "file"

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
            pinnedSortIndex == other.pinnedSortIndex &&
            sourceApp == other.sourceApp &&
            tooLargeToSync == other.tooLargeToSync &&
            originDeviceId == other.originDeviceId
    }

    override fun hashCode(): Int {
        var result = id.hashCode()
        result = 31 * result + contentType.hashCode()
        result = 31 * result + isSensitive.hashCode()
        result = 31 * result + wallTimeMs.hashCode()
        result = 31 * result + snippet.hashCode()
        result = 31 * result + (imagePng?.contentHashCode() ?: 0)
        result = 31 * result + pinned.hashCode()
        result = 31 * result + pinnedSortIndex
        result = 31 * result + (sourceApp?.hashCode() ?: 0)
        result = 31 * result + tooLargeToSync.hashCode()
        result = 31 * result + (originDeviceId?.hashCode() ?: 0)
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
