package com.copypaste.android

import android.content.Context
import android.content.SharedPreferences
import java.util.UUID

class Settings(context: Context) {
    private val prefs: SharedPreferences = context.getSharedPreferences("copypaste", Context.MODE_PRIVATE)

    var relayUrl: String
        get() = prefs.getString("relay_url", "http://localhost:8080") ?: "http://localhost:8080"
        set(v) = prefs.edit().putString("relay_url", v).apply()

    var syncEnabled: Boolean
        get() = prefs.getBoolean("sync_enabled", false)
        set(v) = prefs.edit().putBoolean("sync_enabled", v).apply()

    var deviceId: String
        get() {
            val stored = prefs.getString("device_id", null)
            if (stored != null) return stored
            val new = UUID.randomUUID().toString()
            prefs.edit().putString("device_id", new).apply()
            return new
        }

    var showSensitiveWarnings: Boolean
        get() = prefs.getBoolean("show_sensitive_warnings", true)
        set(v) = prefs.edit().putBoolean("show_sensitive_warnings", v).apply()

    var maxHistoryItems: Int
        get() = prefs.getInt("max_history_items", 1000)
        set(v) = prefs.edit().putInt("max_history_items", v).apply()

    fun clear() = prefs.edit().clear().apply()
}
