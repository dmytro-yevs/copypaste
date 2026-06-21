package com.copypaste.android

import android.graphics.BitmapFactory
import android.util.Log

/**
 * Shared helper for generating and storing thumbnails for images that arrive
 * via the three sync-inbound paths:
 *
 *   1. [FgsSyncLoop] cloud-poll branch (~:329)
 *   2. [FgsSyncLoop] P2P dial branch (~:451)
 *   3. [SupabaseRealtimeClient] WS-push branch (~:513)
 *
 * Factored out to avoid triplication. Thumbnail failure is explicitly non-fatal
 * — the image is already stored full-res; a missing thumbnail only means the
 * history list will fall back to the full-res bytes for display.
 *
 * ## Why a separate object (not an extension on ClipboardRepository)?
 * [ClipboardRepository] intentionally has no dependency on Android Bitmap APIs.
 * [ImageThumbnailUtils.generateThumbnail] requires a [android.graphics.Bitmap],
 * so the decode + generate step lives here, outside the repository layer.
 *
 * ## Testability
 * [generateAndStore] accepts a [storeThumbnail] callback so unit tests can
 * pass a no-op lambda and verify the null-safety / non-throwing contract on
 * the JVM without needing a real [android.graphics.BitmapFactory].
 */
object SyncThumbnailHelper {

    private const val TAG = "SyncThumbnailHelper"

    /**
     * Compute the largest power-of-two [BitmapFactory.Options.inSampleSize] such
     * that the decoded bitmap's longest side is at most [targetDim] px.
     *
     * The Android docs recommend powers of two for [inSampleSize]; other values
     * are rounded down to the nearest power of two by the decoder anyway.
     *
     * Pure function — no Android runtime — safe to call in JVM unit tests.
     *
     * @param rawWidth   Width reported by the bounds-only pass (inJustDecodeBounds).
     * @param rawHeight  Height reported by the bounds-only pass (inJustDecodeBounds).
     * @param targetDim  Maximum dimension of the decoded bitmap (pixels).
     * @return A power-of-two sample size ≥ 1.
     */
    internal fun computeInSampleSize(rawWidth: Int, rawHeight: Int, targetDim: Int): Int {
        if (rawWidth <= 0 || rawHeight <= 0 || targetDim <= 0) return 1
        val longest = maxOf(rawWidth, rawHeight)
        if (longest <= targetDim) return 1
        // Find the largest power-of-two divisor that still leaves the decoded
        // image at least targetDim px on the longest side (to keep quality for
        // the subsequent software scale-to-exact inside generateThumbnail).
        var sampleSize = 1
        while ((longest / (sampleSize * 2)) >= targetDim) {
            sampleSize *= 2
        }
        return sampleSize
    }

    /**
     * Decode [imageBytes] as a Bitmap, generate a thumbnail via
     * [ImageThumbnailUtils.generateThumbnail], and pass the result to
     * [storeThumbnail].
     *
     * Returns `true` when a thumbnail was generated and stored, `false` on any
     * failure (decode error, OOM, encode error). Never throws.
     *
     * The decoded [android.graphics.Bitmap] is recycled before returning
     * regardless of outcome.
     *
     * ## Two-pass decode (CopyPaste-44rq.34)
     * Large synced images (e.g. 4000×3000 px) were previously decoded at full
     * resolution into heap before being software-scaled to 680 px. This caused
     * OOM on devices with limited memory and wasted CPU time.
     *
     * The fix adds a bounds-only pass ([BitmapFactory.Options.inJustDecodeBounds])
     * to read the image dimensions without allocating pixel memory, then computes
     * an [BitmapFactory.Options.inSampleSize] that pre-downsamples in the codec
     * to ≥ [ImageThumbnailUtils.THUMB_MAX_DIM] px (power-of-two).
     * [ImageThumbnailUtils.generateThumbnail] performs the final precise scale.
     *
     * Output dimensions and visual quality are unchanged — only peak heap usage
     * during thumbnail generation is reduced.
     *
     * @param imageBytes  Raw PNG/JPEG bytes of the full-res image.
     * @param storeThumbnail  Callback to persist the generated thumbnail bytes
     *   (e.g. `repository::storeThumbnailBytes` partially applied with the id).
     */
    fun generateAndStore(
        imageBytes: ByteArray,
        storeThumbnail: (ByteArray) -> Unit,
    ): Boolean {
        if (imageBytes.isEmpty()) return false

        // Pass 1: bounds-only — read image dimensions without allocating pixels.
        val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeByteArray(imageBytes, 0, imageBytes.size, bounds)

        // Pass 2: decode at reduced resolution using computed inSampleSize.
        val opts = BitmapFactory.Options().apply {
            inSampleSize = computeInSampleSize(
                bounds.outWidth,
                bounds.outHeight,
                ImageThumbnailUtils.THUMB_MAX_DIM,
            )
        }
        val bitmap = try {
            BitmapFactory.decodeByteArray(imageBytes, 0, imageBytes.size, opts)
        } catch (t: Throwable) {
            Log.w(TAG, "generateAndStore: BitmapFactory decode threw: ${t.message}")
            null
        } ?: return false  // null = unrecognised format; non-fatal

        return try {
            val thumbBytes = ImageThumbnailUtils.generateThumbnail(bitmap)
            if (thumbBytes != null) {
                storeThumbnail(thumbBytes)
                true
            } else {
                false
            }
        } catch (t: Throwable) {
            Log.w(TAG, "generateAndStore: thumbnail generation failed (non-fatal): ${t.message}")
            false
        } finally {
            bitmap.recycle()
        }
    }
}
