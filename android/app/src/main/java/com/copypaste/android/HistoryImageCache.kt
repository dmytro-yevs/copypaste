package com.copypaste.android

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.util.LruCache

// ─────────────────────────────────────────────────────────────────────────────
// AB-8 — two-level LRU cache: raw bytes + decoded Bitmaps for list thumbnails
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Process-wide bounded LRU of raw (encoded) display image bytes keyed by item id,
 * mirroring the macOS `ImageThumb` cache. Capped at [IMAGE_BYTE_CACHE_MAX_BYTES]
 * (16 MiB) by summed byte length so the history list cannot blow up memory by
 * holding every image at once. The row fetches its thumbnail through this cache
 * on demand (lazy) rather than [ClipboardRepository.getItems] attaching bytes for
 * every image up front.
 */
private const val IMAGE_BYTE_CACHE_MAX_BYTES = 16 * 1024 * 1024 // 16 MiB

internal val imageByteCache = object : LruCache<String, ByteArray>(IMAGE_BYTE_CACHE_MAX_BYTES) {
    override fun sizeOf(key: String, value: ByteArray): Int = value.size
}

/**
 * Process-wide decoded-bitmap LRU keyed by item id. Avoids re-running
 * [BitmapFactory.decodeByteArray] (a heavy native allocation) every time a row
 * scrolls back into view. Sized by pixel count × 4 bytes/pixel so the cache
 * self-limits to [BITMAP_CACHE_MAX_BYTES] (8 MiB) regardless of image dimensions.
 *
 * Bitmaps are decoded at thumbnail size (see [cachedThumbnailBitmap]) so each
 * entry is small — typically ≤ 500 KiB.
 */
private const val BITMAP_CACHE_MAX_BYTES = 8 * 1024 * 1024 // 8 MiB

internal val bitmapCache = object : LruCache<String, Bitmap>(BITMAP_CACHE_MAX_BYTES) {
    override fun sizeOf(key: String, value: Bitmap): Int =
        value.byteCount.coerceAtLeast(1)
}

/**
 * Return display bytes for image item [id], served from [imageByteCache] when
 * present, otherwise fetched once via [ClipboardRepository.getDisplayImageBytes]
 * (thumbnail preferred, full-res fallback) and cached. Returns null when the item
 * has no stored image bytes.
 */
internal fun cachedDisplayImageBytes(repository: ClipboardRepository, id: String): ByteArray? {
    imageByteCache.get(id)?.let { return it }
    val bytes = repository.getDisplayImageBytes(id) ?: return null
    imageByteCache.put(id, bytes)
    return bytes
}

/**
 * Return a decoded [Bitmap] for image item [id] at thumbnail size, served from
 * [bitmapCache] when present. On a cache miss the raw bytes are fetched via
 * [cachedDisplayImageBytes] and decoded with [BitmapFactory.Options.inSampleSize]
 * so the decoded allocation is proportional to the displayed size (≤ [targetPx]
 * on the longer edge), not the original full resolution.
 *
 * Never call on the main thread — always inside a [kotlinx.coroutines.Dispatchers.IO]
 * or [kotlinx.coroutines.Dispatchers.Default] context.
 */
internal fun cachedThumbnailBitmap(
    repository: ClipboardRepository,
    id: String,
    targetPx: Int = 340,
): Bitmap? {
    bitmapCache.get(id)?.let { return it }
    val bytes = cachedDisplayImageBytes(repository, id) ?: return null
    // First pass: decode bounds only (no pixel allocation) to determine inSampleSize.
    val opts = BitmapFactory.Options().apply { inJustDecodeBounds = true }
    BitmapFactory.decodeByteArray(bytes, 0, bytes.size, opts)
    val rawW = opts.outWidth.coerceAtLeast(1)
    val rawH = opts.outHeight.coerceAtLeast(1)
    var sample = 1
    while ((rawW / (sample * 2)) >= targetPx || (rawH / (sample * 2)) >= targetPx) {
        sample *= 2
    }
    // Second pass: decode at the reduced sample size.
    val decoded = BitmapFactory.decodeByteArray(
        bytes, 0, bytes.size,
        BitmapFactory.Options().apply { inSampleSize = sample },
    ) ?: return null
    bitmapCache.put(id, decoded)
    return decoded
}

/** Evict both caches when an item is deleted so stale memory is released promptly. */
internal fun evictImageCaches(id: String) {
    imageByteCache.remove(id)
    bitmapCache.remove(id)
}

// ─────────────────────────────────────────────────────────────────────────────
// App-icon bitmap LRU — avoids re-decoding source-app icons on every scroll
// ─────────────────────────────────────────────────────────────────────────────

/**
 * Process-wide decoded-bitmap LRU for source-app icons, keyed by package name.
 * Icons are small (≤ 48×48 dp typically) so 2 MiB is ample for dozens of apps.
 * Without this cache, every text row with a [ClipboardItem.sourceApp] re-ran
 * [AppIconHelper.getAppIconBase64] + [BitmapFactory.decodeByteArray] on every
 * scroll recomposition — allocating a fresh Bitmap each time.
 */
private const val APP_ICON_CACHE_MAX_BYTES = 2 * 1024 * 1024 // 2 MiB

internal val appIconBitmapCache = object : LruCache<String, Bitmap>(APP_ICON_CACHE_MAX_BYTES) {
    override fun sizeOf(key: String, value: Bitmap): Int =
        value.byteCount.coerceAtLeast(1)
}
