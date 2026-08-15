package com.copypaste.app

import android.content.Context
import android.database.ContentObserver
import android.os.Handler
import android.os.Looper
import android.provider.Settings
import android.util.Log

/**
 * `Settings.Secure.CLIPBOARD_SHOW_ACCESS_NOTIFICATIONS`, read once and then
 * only when it changes.
 *
 * The probe carries this value and the probe rides on every drain, so an
 * uncached read is a ContentResolver query 86,400 times a day. Another app or
 * the user can change it, which is what the observer is for; our own write goes
 * through the shell uid and calls [invalidate] before reporting, because a
 * suppression we only assumed happened is the same class of lie as reporting
 * capture that is not running.
 */
object ClipboardNoticeSetting {
    private const val TAG = "CopyPasteNotice"

    internal const val NAME = "clipboard_show_access_notifications"

    @Volatile
    private var cached: Boolean? = null
    private var observer: ContentObserver? = null

    fun suppressed(context: Context): Boolean {
        cached?.let { return it }
        val value = try {
            Settings.Secure.getInt(context.contentResolver, NAME, 1) == 0
        } catch (e: Throwable) {
            Log.w(TAG, "the clipboard notice setting could not be read", e)
            return false
        }
        // Only remembered while the platform can say it changed. Without the
        // observer this would freeze at its first reading and the setup screen
        // would report a suppression the user had since undone.
        if (observer != null) cached = value
        return value
    }

    fun invalidate() {
        cached = null
    }

    @Synchronized
    fun observe(context: Context) {
        if (observer != null) return
        val registered = object : ContentObserver(Handler(Looper.getMainLooper())) {
            override fun onChange(selfChange: Boolean) = invalidate()
        }
        try {
            context.applicationContext.contentResolver.registerContentObserver(
                Settings.Secure.getUriFor(NAME),
                false,
                registered,
            )
        } catch (e: Throwable) {
            Log.w(TAG, "the clipboard notice setting cannot be observed", e)
            return
        }
        observer = registered
    }

    /**
     * Idempotent: this is process-wide state that outlives any one activity, so
     * it can be stopped when it was never started.
     */
    @Synchronized
    fun stopObserving(context: Context) {
        val registered = observer ?: return
        observer = null
        try {
            context.applicationContext.contentResolver.unregisterContentObserver(registered)
        } catch (e: Throwable) {
            Log.w(TAG, "the clipboard notice observer was already unregistered", e)
        }
        invalidate()
    }
}
