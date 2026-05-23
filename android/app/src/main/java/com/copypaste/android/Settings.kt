package com.copypaste.android

import android.content.Context
import android.content.SharedPreferences
import android.util.Base64
import java.security.SecureRandom
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

    /**
     * When true (default), preview text for items flagged as sensitive is
     * replaced with bullet placeholders in the history list. Tap-to-reveal
     * briefly unmasks the item (handled in the UI layer).
     */
    var maskSensitiveContent: Boolean
        get() = prefs.getBoolean("mask_sensitive_content", true)
        set(v) = prefs.edit().putBoolean("mask_sensitive_content", v).apply()

    /**
     * When true (default), the foreground service is actively monitoring the
     * clipboard. Toggled by the notification's Pause/Resume action; consumed
     * by [ClipboardService] before storing each detected change.
     */
    var captureEnabled: Boolean
        get() = prefs.getBoolean("capture_enabled", true)
        set(v) = prefs.edit().putBoolean("capture_enabled", v).apply()

    var maxHistoryItems: Int
        get() = prefs.getInt("max_history_items", 1000)
        set(v) = prefs.edit().putInt("max_history_items", v).apply()

    /**
     * 256-bit AES key used for local clipboard encryption.
     * Generated once on first access and persisted in SharedPreferences.
     * In production this should be stored in Android Keystore; this is a
     * safe-enough fallback until the Keystore integration lands.
     */
    val encryptionKey: ByteArray
        get() {
            val stored = prefs.getString("encryption_key_b64", null)
            if (stored != null) return Base64.decode(stored, Base64.DEFAULT)
            val key = ByteArray(32).also { SecureRandom().nextBytes(it) }
            prefs.edit().putString("encryption_key_b64", Base64.encodeToString(key, Base64.DEFAULT)).apply()
            return key
        }

    fun clear() = prefs.edit().clear().apply()
}
