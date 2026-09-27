package com.copypaste.app

import android.content.Context
import android.database.ContentObserver
import android.os.Handler
import android.os.Looper
import android.provider.Settings
import java.util.concurrent.atomic.AtomicBoolean
import rikka.shizuku.Shizuku

internal fun shouldRefreshClipboardNotice(observing: Boolean, resolved: Boolean): Boolean =
    observing && !resolved

/**
 * The secure setting is readable only through the Shizuku user service. The
 * app process keeps the last verified value and treats an unavailable read as
 * unknown, represented safely as "not suppressed", without retrying per probe.
 */
object ClipboardNoticeSetting {
    internal const val NAME = "clipboard_show_access_notifications"

    @Volatile
    private var cached: Boolean? = null
    @Volatile
    private var observing = false
    @Volatile
    private var resolved = false
    private val refreshing = AtomicBoolean(false)
    private var observer: ContentObserver? = null

    fun suppressed(context: Context): Boolean {
        refresh()
        return cached ?: false
    }

    fun invalidate() {
        cached = null
        resolved = false
        refresh()
    }

    @Synchronized
    fun observe(context: Context) {
        if (observer != null) return
        observing = true
        resolved = false
        val next = object : ContentObserver(Handler(Looper.getMainLooper())) {
            override fun onChange(selfChange: Boolean) = invalidate()
        }
        try {
            context.applicationContext.contentResolver.registerContentObserver(
                Settings.Secure.getUriFor(NAME),
                false,
                next,
            )
            observer = next
        } catch (_: Exception) {
            // Reading remains privileged and cached even where observation is unavailable.
        }
        refresh()
    }

    @Synchronized
    fun stopObserving(context: Context) {
        observer?.let { registered ->
            try {
                context.applicationContext.contentResolver.unregisterContentObserver(registered)
            } catch (_: Exception) {
            }
        }
        observer = null
        observing = false
        cached = null
        resolved = false
        refreshing.set(false)
    }

    internal fun publishForTest(value: Boolean?) {
        cached = value
        resolved = true
    }

    private fun refresh() {
        if (!shouldRefreshClipboardNotice(observing, resolved) ||
            !refreshing.compareAndSet(false, true)
        ) {
            return
        }
        ShizukuSettings.clipboardAccessNotifications { value ->
            if (observing) {
                cached = value
                resolved = true
            }
            refreshing.set(false)
        }
    }

    init {
        Shizuku.addBinderReceivedListener { invalidate() }
        Shizuku.addBinderDeadListener { invalidate() }
    }
}
