package com.copypaste.android

import android.graphics.Bitmap
import android.util.Log
import java.io.ByteArrayOutputStream

/**
 * Utilities for generating downscaled image thumbnails at capture time.
 *
 * ## Design
 * - [scaledDimensions]: pure Kotlin — computes target width/height given a max
 *   dimension constraint. Aspect-ratio-preserving, NEVER upscales, clamps
 *   degenerate (0) inputs to 1. Testable on the JVM without Android APIs.
 * - [generateThumbnail]: Android-aware — decodes a full-res [Bitmap], scales it,
 *   compresses to WebP (LOSSY 80) or PNG fallback, recycles both bitmaps.
 *
 * ## macOS parity
 * macOS precomputes thumbnails at image capture, max dim ~680 px, WebP@680.
 * This mirrors that behaviour on Android: capture path calls [generateThumbnail]
 * and stores the result under "item_thumb_<id>" via [ClipboardRepository].
 *
 * The history list displays thumbnails ([ClipboardRepository.getThumbnailBytes])
 * and falls back to full-res ([ClipboardRepository.getImageBytes]) when absent,
 * so existing items without a thumbnail continue to render correctly.
 */
object ImageThumbnailUtils {

    private const val TAG = "ImageThumbnailUtils"

    /** Max dimension (px) for thumbnails — matches macOS WebP@680 spec. */
    const val THUMB_MAX_DIM = 680

    /** WebP quality (0–100). 80 matches macOS Plan-B thumbnail spec. */
    private const val WEBP_QUALITY = 80

    /**
     * Compute scaled (width, height) that fit within [maxDim] on the longest
     * side while preserving aspect ratio and NEVER upscaling.
     *
     * Degenerate inputs (width or height ≤ 0) are clamped to 1×1.
     *
     * Pure function — no Android API dependency; safe to call in JVM unit tests.
     */
    fun scaledDimensions(width: Int, height: Int, maxDim: Int = THUMB_MAX_DIM): Pair<Int, Int> {
        if (width <= 0 || height <= 0) return Pair(1, 1)
        val longest = maxOf(width, height)
        // Never upscale: if already within bounds, return original dimensions.
        if (longest <= maxDim) return Pair(width, height)
        val scale = maxDim.toDouble() / longest.toDouble()
        val newW = (width * scale).toInt().coerceAtLeast(1)
        val newH = (height * scale).toInt().coerceAtLeast(1)
        return Pair(newW, newH)
    }

    /**
     * Generate a thumbnail from [sourceBitmap].
     *
     * Returns the compressed bytes (WebP LOSSY on API 30+, PNG on older APIs),
     * or null on OOM or encode failure.
     *
     * [sourceBitmap] is NOT recycled by this function — the caller owns it and
     * must recycle it after storeImageBytes returns.
     *
     * The intermediate scaled bitmap IS recycled here to release native memory
     * promptly.
     */
    fun generateThumbnail(sourceBitmap: Bitmap, maxDim: Int = THUMB_MAX_DIM): ByteArray? {
        val (newW, newH) = scaledDimensions(sourceBitmap.width, sourceBitmap.height, maxDim)

        // Already small enough — compress the source directly without an
        // intermediate copy to save one allocation.
        val bitmapToCompress: Bitmap
        val createdScaled: Boolean
        if (newW == sourceBitmap.width && newH == sourceBitmap.height) {
            bitmapToCompress = sourceBitmap
            createdScaled = false
        } else {
            val scaled = try {
                Bitmap.createScaledBitmap(sourceBitmap, newW, newH, /* filter= */ true)
            } catch (e: OutOfMemoryError) {
                Log.w(TAG, "generateThumbnail: OOM creating scaled bitmap (${newW}x${newH}) — skipping thumb")
                return null
            }
            bitmapToCompress = scaled
            createdScaled = true
        }

        return try {
            ByteArrayOutputStream().use { baos ->
                // WebP LOSSY is available from API 30 (R). On older devices fall
                // back to PNG (lossless but universally supported).
                @Suppress("DEPRECATION") // PNG fallback for API < 30
                val format = if (android.os.Build.VERSION.SDK_INT >= android.os.Build.VERSION_CODES.R) {
                    Bitmap.CompressFormat.WEBP_LOSSY
                } else {
                    Bitmap.CompressFormat.PNG
                }
                bitmapToCompress.compress(format, WEBP_QUALITY, baos)
                baos.toByteArray()
            }
        } catch (t: Throwable) {
            Log.w(TAG, "generateThumbnail: compress failed: ${t.message}")
            null
        } finally {
            if (createdScaled) bitmapToCompress.recycle()
        }
    }
}
