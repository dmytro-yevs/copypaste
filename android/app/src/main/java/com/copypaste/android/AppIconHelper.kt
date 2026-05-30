package com.copypaste.android

import android.content.Context
import android.content.pm.PackageManager
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.drawable.AdaptiveIconDrawable
import android.graphics.drawable.BitmapDrawable
import android.graphics.drawable.Drawable
import android.util.Base64
import android.util.Log
import java.io.ByteArrayOutputStream

/**
 * Extracts and caches source-app icons for display alongside clipboard items.
 *
 * Usage (from any Activity / Fragment / ViewModel):
 *
 *   val b64: String? = AppIconHelper.getAppIconBase64(context, "com.google.android.gm")
 *   // b64 is a base64-encoded PNG (32×32 dp), or null if the package is not installed.
 *
 * The result is cached in a plain HashMap so AppKit / PackageManager is only called
 * once per bundle ID per process lifetime.  `null` values are also cached so that
 * absent packages do not cause repeated PackageManager queries.
 */
object AppIconHelper {

    private const val TAG = "AppIconHelper"

    /** Target icon edge size in dp-equivalent pixels. */
    private const val ICON_SIZE_PX = 48

    /**
     * In-memory cache.  Keys are package names; values are base64 PNG strings
     * (or null meaning "already tried, package not installed / icon unavailable").
     */
    private val cache = HashMap<String, String?>()

    /**
     * Return a base64-encoded 48×48 PNG for [packageName], or `null` if the
     * package is not installed on this device.
     *
     * Thread-safety: this method is synchronous and NOT thread-safe.  Call it
     * from the main thread or from a dedicated icon-loading coroutine with
     * appropriate synchronisation if needed.
     */
    fun getAppIconBase64(context: Context, packageName: String): String? {
        // Fast path: already cached (including negative results).
        if (cache.containsKey(packageName)) {
            return cache[packageName]
        }

        val result = extractIcon(context, packageName)
        cache[packageName] = result
        return result
    }

    /**
     * Clear the in-memory cache.  Useful after package install/uninstall events
     * so that stale null entries are refreshed on the next query.
     */
    fun clearCache() {
        cache.clear()
    }

    // -------------------------------------------------------------------------
    // Private helpers
    // -------------------------------------------------------------------------

    private fun extractIcon(context: Context, packageName: String): String? {
        val pm = context.packageManager

        // Resolve the Drawable for the package icon.
        val drawable: Drawable = try {
            pm.getApplicationIcon(packageName)
        } catch (e: PackageManager.NameNotFoundException) {
            Log.d(TAG, "getAppIconBase64: package not installed: $packageName")
            return null
        } catch (e: Exception) {
            Log.w(TAG, "getAppIconBase64: unexpected error for $packageName", e)
            return null
        }

        // Convert the Drawable to a Bitmap at the target size.
        val bitmap = drawableToBitmap(drawable) ?: return null

        // Encode as PNG → base64.
        return try {
            val out = ByteArrayOutputStream()
            bitmap.compress(Bitmap.CompressFormat.PNG, 100, out)
            Base64.encodeToString(out.toByteArray(), Base64.NO_WRAP)
        } catch (e: Exception) {
            Log.w(TAG, "getAppIconBase64: PNG encoding failed for $packageName", e)
            null
        }
    }

    /**
     * Render [drawable] to a [Bitmap] at [ICON_SIZE_PX] × [ICON_SIZE_PX].
     *
     * Handles the three common Drawable subtypes:
     * - [BitmapDrawable] — resampled to the target size.
     * - [AdaptiveIconDrawable] — drawn into a Canvas (API 26+).
     * - Everything else — drawn into a Canvas.
     */
    private fun drawableToBitmap(drawable: Drawable): Bitmap? {
        return try {
            if (drawable is BitmapDrawable && drawable.bitmap != null) {
                val src = drawable.bitmap
                if (src.width == ICON_SIZE_PX && src.height == ICON_SIZE_PX) {
                    src
                } else {
                    Bitmap.createScaledBitmap(src, ICON_SIZE_PX, ICON_SIZE_PX, true)
                }
            } else {
                val bitmap = Bitmap.createBitmap(
                    ICON_SIZE_PX, ICON_SIZE_PX, Bitmap.Config.ARGB_8888
                )
                val canvas = Canvas(bitmap)
                drawable.setBounds(0, 0, canvas.width, canvas.height)
                drawable.draw(canvas)
                bitmap
            }
        } catch (e: Exception) {
            Log.w(TAG, "drawableToBitmap: failed", e)
            null
        }
    }
}
