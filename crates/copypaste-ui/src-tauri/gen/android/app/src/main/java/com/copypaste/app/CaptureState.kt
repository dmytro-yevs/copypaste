package com.copypaste.app

import android.content.Context

/**
 * The user's explicit decision to keep background capture armed.
 *
 * A foreground service may be recreated with a null intent after the process
 * is killed. Its listener is in-memory, so the service needs a tiny durable
 * record of that decision before it can safely re-register it.
 */
object CaptureState {
    private const val PREFS = "capture-state"
    private const val ARMED = "armed"
    private const val LOST_TITLE = "lost-title"
    private const val LOST_BODY = "lost-body"

    data class Armed(val lostTitle: String, val lostBody: String)

    fun armed(context: Context): Armed? {
        val prefs = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
        if (!prefs.getBoolean(ARMED, false)) return null
        return Armed(
            prefs.getString(LOST_TITLE, DEFAULT_LOST_TITLE) ?: DEFAULT_LOST_TITLE,
            prefs.getString(LOST_BODY, DEFAULT_LOST_BODY) ?: DEFAULT_LOST_BODY,
        )
    }

    fun persistArmed(context: Context, lostTitle: String, lostBody: String) {
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            .edit()
            .putBoolean(ARMED, true)
            .putString(LOST_TITLE, lostTitle)
            .putString(LOST_BODY, lostBody)
            .apply()
    }

    fun clear(context: Context) {
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)
            .edit()
            .clear()
            .apply()
    }

    // Used only if Android restarts a service written by an older app version
    // that did not persist the Rust-provided text.
    private const val DEFAULT_LOST_TITLE = "Background capture stopped."
    private const val DEFAULT_LOST_BODY =
        "Start Shizuku, then turn on background capture again."
}
