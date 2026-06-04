package com.copypaste.android

import androidx.compose.runtime.Immutable

/**
 * Immutable value type for a single clipboard history row.
 *
 * @Immutable tells the Compose compiler that all public properties are stable
 * and will never change after construction, enabling it to skip recomposing
 * rows whose inputs have not changed.  This is safe because every field is a
 * val of a stable Kotlin primitive type (String, Long, Boolean, Int, or null).
 *
 * imagePng has been REMOVED.  Image bytes are fetched lazily per-row through
 * the process-wide two-level LRU in HistoryActivity (cachedThumbnailBitmap),
 * so they never need to live in this value type.  The prior ByteArray field
 * forced a custom equals/hashCode AND made Compose treat the type as *unstable*
 * (arrays use identity equality), defeating per-row skipping entirely.
 */
@Immutable
data class ClipboardItem(
    val id: String,
    val contentType: String,
    val isSensitive: Boolean,
    val wallTimeMs: Long,
    val snippet: String = "",
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
     * Stable device id (UUID) of the device that originally captured this clipboard item.
     * Null for legacy items that pre-date origin tracking, and for items captured
     * locally before the first sync key was established.
     *
     * Stored as pipe-delimited field 6 (index 6) in the blob:
     * <wallTimeMs>|<contentType>|<plaintextLen>|<nonceB64>|<ctB64>|<lamportTs>|<originDeviceId>
     *
     * Drives per-row device attribution badges and device-filter UI
     * (parity with macOS HistoryView DeviceBadge / device filter).
     */
    val originDeviceId: String? = null,
) {
    /** True when this item carries an image payload that can be rendered as a thumbnail. */
    val isImage: Boolean get() = contentTypeIsImage(contentType)

    /** True when this item is a synced file (content_type == "file"). */
    val isFile: Boolean get() = contentTypeIsFile(contentType)

    /** True when this item carries a plain-text payload (includes "url"). */
    val isText: Boolean get() = contentTypeIsText(contentType)

    // No custom equals/hashCode: imagePng was removed, so all fields are val
    // primitives, String, Boolean, Int, or nullable String (stable Kotlin value
    // types incl. originDeviceId) — data-class structural equality is correct and
    // the Compose compiler can trust @Immutable for per-row skip decisions.
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
