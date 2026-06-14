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
 * The result is cached in a bounded LRU map so AppKit / PackageManager is only
 * called once per bundle ID while it remains hot.  `null` results are cached as the
 * sentinel value [ABSENT] (empty string); [getAppIconBase64] translates the sentinel
 * back to `null` on read.
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
     * Maximum number of resolved package→icon entries retained in [cache].
     *
     * Bounded to prevent unbounded growth over the process lifetime (one entry per
     * distinct source package, including negative/[ABSENT] results).  Matches the web
     * client's LRU-128 icon cache; the Rust macOS side caps at 256 (`ICON_CACHE_CAP`).
     * 128 distinct foreground apps is far beyond realistic clipboard provenance, so
     * evictions are rare in practice and cheap (one PackageManager re-resolve) when
     * they do occur.
     */
    internal const val MAX_CACHE_ENTRIES = 128

    /**
     * Build a bounded, access-ordered LRU map: the least-recently-used entry is
     * evicted once size exceeds [maxEntries].  Extracted so the eviction policy is
     * unit-testable without a [Context] (see `AppIconCacheTest`).
     *
     * [LinkedHashMap] is NOT thread-safe and access-order mutates the structure on
     * every read, so callers must guard it with a lock.
     */
    internal fun newIconCache(maxEntries: Int = MAX_CACHE_ENTRIES): LinkedHashMap<String, String> =
        object : LinkedHashMap<String, String>(
            /* initialCapacity = */ 16,
            /* loadFactor = */ 0.75f,
            /* accessOrder = */ true,
        ) {
            override fun removeEldestEntry(eldest: Map.Entry<String, String>): Boolean =
                size > maxEntries
        }

    /**
     * Bounded LRU in-memory cache.  Keys are package names; values are base64 PNG
     * strings or [ABSENT] meaning "already tried, not found".
     *
     * All access goes through [cacheLock]; [getAppIconBase64] keeps the (potentially
     * slow) [extractIcon] PackageManager call OUTSIDE the lock to avoid blocking
     * concurrent readers.  A small TOCTOU race on a miss (two threads both resolving
     * the same key) is harmless — both store the same result.
     */
    private val cacheLock = Any()

    private val cache = newIconCache()

    /**
     * Return a base64-encoded 48×48 PNG for [packageName], or `null` if the
     * package is not installed on this device.
     *
     * Thread-safe: may be called concurrently from any thread or coroutine
     * dispatcher (e.g. [kotlinx.coroutines.Dispatchers.Default]).
     */
    fun getAppIconBase64(context: Context, packageName: String): String? {
        // Fast path: already cached (including negative results). The lock also
        // refreshes LRU access-order for this key.
        synchronized(cacheLock) {
            cache[packageName]?.let { cached ->
                return if (cached == ABSENT) null else cached
            }
        }

        // Resolve OUTSIDE the lock — PackageManager I/O must not block other readers.
        val result = extractIcon(context, packageName)
        // Store the result or the ABSENT sentinel (empty string).
        synchronized(cacheLock) {
            cache[packageName] = result ?: ABSENT
        }
        return result
    }

    /**
     * Clear the in-memory cache.  Useful after package install/uninstall events
     * so that stale null entries are refreshed on the next query.
     */
    fun clearCache() {
        synchronized(cacheLock) {
            cache.clear()
        }
    }

    /** Current number of cached entries (test/diagnostic helper). */
    internal fun cacheSize(): Int = synchronized(cacheLock) { cache.size }

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
