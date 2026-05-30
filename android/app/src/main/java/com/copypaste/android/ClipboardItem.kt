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
) {
    /** True when this item carries an image payload that can be rendered as a thumbnail. */
    val isImage: Boolean get() = contentType.startsWith("image/") || contentType == "image"
}
