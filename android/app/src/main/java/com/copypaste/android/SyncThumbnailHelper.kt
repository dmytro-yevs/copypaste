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
     * @param imageBytes  Raw PNG/JPEG bytes of the full-res image.
     * @param storeThumbnail  Callback to persist the generated thumbnail bytes
     *   (e.g. `repository::storeThumbnailBytes` partially applied with the id).
     */
    fun generateAndStore(
        imageBytes: ByteArray,
        storeThumbnail: (ByteArray) -> Unit,
    ): Boolean {
        if (imageBytes.isEmpty()) return false

        val bitmap = try {
            BitmapFactory.decodeByteArray(imageBytes, 0, imageBytes.size)
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
