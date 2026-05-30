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
import java.util.concurrent.ConcurrentHashMap

/**
 * Extracts and caches source-app icons for display alongside clipboard items.
 *
 * Usage (from any Activity / Fragment / ViewModel):
 *
 *   val b64: String? = AppIconHelper.getAppIconBase64(context, "com.google.android.gm")
 *   // b64 is a base64-encoded PNG (32×32 dp), or null if the package is not installed.
 *
 * The result is cached in a [ConcurrentHashMap] so AppKit / PackageManager is only
 * called once per bundle ID per process lifetime.  `null` results are cached as the
 * sentinel value [ABSENT] (empty string) because [ConcurrentHashMap] does not permit
 * null values; [getAppIconBase64] translates the sentinel back to `null` on read.
 */
object AppIconHelper {

    private const val TAG = "AppIconHelper"

    /** Target icon edge size in dp-equivalent pixels. */
    private const val ICON_SIZE_PX = 48

    /**
     * Sentinel stored in [cache] to mean "already tried, package not installed /
     * icon unavailable".  Translated back to `null` by [getAppIconBase64].
     * An empty string is safe here because real base64-PNG output is never empty.
     */
    private const val ABSENT = ""

    /**
     * Thread-safe in-memory cache.  Keys are package names; values are base64 PNG
     * strings or [ABSENT] meaning "already tried, not found".
     *
     * [ConcurrentHashMap] allows concurrent reads from [Dispatchers.Default] without
     * explicit locking.  A small TOCTOU race on a cache miss (two threads both seeing
     * a miss and both calling [extractIcon] for the same key) is harmless — both will
     * store the same result and the extra PackageManager call is cheap.
     */
    private val cache = ConcurrentHashMap<String, String>()

    /**
     * Return a base64-encoded 48×48 PNG for [packageName], or `null` if the
     * package is not installed on this device.
     *
     * Thread-safe: may be called concurrently from any thread or coroutine
     * dispatcher (e.g. [kotlinx.coroutines.Dispatchers.Default]).
     */
    fun getAppIconBase64(context: Context, packageName: String): String? {
        // Fast path: already cached (including negative results).
        cache[packageName]?.let { cached ->
            return if (cached == ABSENT) null else cached
        }

        val result = extractIcon(context, packageName)
        // Store the result or the ABSENT sentinel (ConcurrentHashMap rejects null values).
        cache[packageName] = result ?: ABSENT
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
