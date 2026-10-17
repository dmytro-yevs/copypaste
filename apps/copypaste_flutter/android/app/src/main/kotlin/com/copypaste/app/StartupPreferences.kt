package com.copypaste.app

import android.content.Context

object StartupPreferences {
    private const val STORE = "copypaste_startup"
    private const val ENABLED = "enabled"

    fun enabled(context: Context): Boolean =
        context.getSharedPreferences(STORE, Context.MODE_PRIVATE).getBoolean(ENABLED, true)

    fun setEnabled(context: Context, enabled: Boolean): Boolean =
        context.getSharedPreferences(STORE, Context.MODE_PRIVATE)
            .edit().putBoolean(ENABLED, enabled).commit()
}
