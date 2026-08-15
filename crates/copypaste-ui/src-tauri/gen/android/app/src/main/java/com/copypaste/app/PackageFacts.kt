package com.copypaste.app

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.PackageManager
import android.util.Log
import android.util.LruCache
import androidx.core.content.ContextCompat

/**
 * Whether a package is installed, and what it is called.
 *
 * Both are answered on hot paths: `probe` rides on every drain, once a second
 * for the life of the process, and a label is resolved for every captured clip.
 * Uncached that is 86,400 `getPackageInfo` round trips a day plus two more per
 * copy. Only what a package broadcast can retract is held here — never
 * clipboard content, and nothing whose staleness a user would see.
 *
 * A lookup that fails for any reason other than "no such package" is not
 * cached. Remembering a transient failure would turn one bad call into a
 * permanent wrong answer about whether rung 2 is available at all.
 */
object PackageFacts {
    private const val TAG = "CopyPastePackages"

    /** Labels are per source app; a phone's clipboard sees few of them. */
    private const val MAX_LABELS = 64

    private val installed = HashMap<String, Boolean>()
    private val labels = LruCache<String, Label>(MAX_LABELS)
    private var receiver: BroadcastReceiver? = null

    /** `LruCache` cannot hold a null value, and "this app has no label" is one. */
    private class Label(val text: String?)

    @Synchronized
    fun isInstalled(context: Context, packageId: String): Boolean {
        installed[packageId]?.let { return it }
        val answer = try {
            context.packageManager.getPackageInfo(packageId, 0)
            true
        } catch (_: PackageManager.NameNotFoundException) {
            false
        } catch (e: Throwable) {
            Log.w(TAG, "could not ask whether a package is installed", e)
            return false
        }
        remember(packageId) { installed[packageId] = answer }
        return answer
    }

    /** Display metadata only, and only for a package the platform named. */
    @Synchronized
    fun label(context: Context, packageId: String?): String? {
        val key = packageId?.trim()?.takeIf { it.isNotEmpty() } ?: return null
        labels.get(key)?.let { return it.text }
        val answer = try {
            val info = context.packageManager.getApplicationInfo(key, 0)
            context.packageManager
                .getApplicationLabel(info)
                .toString()
                .trim()
                .takeIf { it.isNotEmpty() }
        } catch (_: PackageManager.NameNotFoundException) {
            null
        } catch (e: Throwable) {
            Log.w(TAG, "could not resolve a source application label", e)
            return null
        }
        remember(key) { labels.put(key, Label(answer)) }
        return answer
    }

    /**
     * Nothing is remembered while the platform cannot say it changed: without
     * the receiver the answer would freeze at whatever it was first read as,
     * and an uninstalled Shizuku would keep reading as installed forever.
     */
    private inline fun remember(packageId: String, store: () -> Unit) {
        if (receiver != null) store() else forget(packageId)
    }

    @Synchronized
    fun observe(context: Context) {
        if (receiver != null) return
        val registered = object : BroadcastReceiver() {
            override fun onReceive(context: Context?, intent: Intent?) {
                forget(intent?.data?.schemeSpecificPart)
            }
        }
        val filter = IntentFilter().apply {
            addAction(Intent.ACTION_PACKAGE_ADDED)
            addAction(Intent.ACTION_PACKAGE_REMOVED)
            addAction(Intent.ACTION_PACKAGE_REPLACED)
            addAction(Intent.ACTION_PACKAGE_CHANGED)
            addDataScheme("package")
        }
        try {
            ContextCompat.registerReceiver(
                context.applicationContext,
                registered,
                filter,
                ContextCompat.RECEIVER_NOT_EXPORTED,
            )
        } catch (e: Throwable) {
            Log.w(TAG, "package changes cannot be observed; nothing will be cached", e)
            return
        }
        receiver = registered
    }

    /** Idempotent: teardown runs on every activity destroy, rotation included. */
    @Synchronized
    fun stopObserving(context: Context) {
        val registered = receiver ?: return
        receiver = null
        try {
            context.applicationContext.unregisterReceiver(registered)
        } catch (e: IllegalArgumentException) {
            Log.w(TAG, "the package receiver was already unregistered", e)
        }
        forget(null)
    }

    /** `null` forgets everything — a package event we could not attribute. */
    @Synchronized
    fun forget(packageId: String?) {
        if (packageId == null) {
            installed.clear()
            labels.evictAll()
            return
        }
        installed.remove(packageId)
        labels.remove(packageId)
    }
}
