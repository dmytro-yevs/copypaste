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
     *
     * NOTE: [data class] equality/hashCode operate on ByteArray by reference for
     * arrays, which is intentional here — the image bytes are large and reference
     * identity is sufficient for DiffUtil's [areContentsTheSame] check (a re-load
     * yields a new ByteArray instance, which signals the row needs rebinding).
     */
    val imagePng: ByteArray? = null,
    /**
     * True when the user has explicitly pinned this item. Pinned items are:
     *  - never pruned by the retention/quota pass
     *  - never auto-wiped by [ClipboardRepository.wipeExpiredSensitive]
     *  - cleared only by the explicit [ClipboardRepository.clearAll] user action
     *
     * Persisted in the "pinned_ids" SharedPreferences key as a comma-joined set
     * (same pattern as synced_source_ids). Populated by [ClipboardRepository.getItems].
     */
    val pinned: Boolean = false,
) {
    /** True when this item carries an image payload that can be rendered as a thumbnail. */
    val isImage: Boolean get() = contentType.startsWith("image/") || contentType == "image"
}
