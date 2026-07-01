package com.copypaste.android

import android.content.SharedPreferences

/**
 * Collaborator extracted from the [Settings] god-file (CopyPaste-vp63.36):
 * owns the FFI-backed size/quota/ttl config knobs. Defaults are SEEDED from
 * the native `defaultConfig()` and every write is run through the native
 * `clampConfig(...)` so Android uses the SAME defaults/floors/ceilings the
 * macOS daemon enforces (crates/copypaste-core/src/config/defaults.rs).
 * [Settings] delegates every public property here verbatim (facade, zero
 * call-site churn).
 */
class ConfigKnobsStore(private val prefs: SharedPreferences) {
    /**
     * Process-wide cached macOS-parity config defaults from the native
     * `defaultConfig()`. Lazy so the FFI (or its Kotlin fallback) is invoked at
     * most once per process; the values are immutable defaults.
     */
    private val configDefaults: uniffi.copypaste_android.Config
        get() = cachedConfigDefaults ?: synchronized(configDefaultsLock) {
            cachedConfigDefaults ?: defaultConfig().also { cachedConfigDefaults = it }
        }

    /**
     * Clamp a candidate [Config]-shaped tuple of the size/quota/ttl knobs through
     * the native `clampConfig(...)`, starting from [configDefaults] and overlaying
     * only the byte/secs knobs this store owns. Returns the clamped config so each
     * setter can persist a daemon-valid value. Pure-ish: reads defaults, no writes.
     */
    private fun clampSizeKnobs(
        maxTextSizeBytes: Long = this.maxTextSizeBytes,
        maxImageSizeBytes: Long = this.maxImageSizeBytes,
        maxFileSizeBytes: Long = this.maxFileSizeBytes,
        storageQuotaBytes: Long = this.storageQuotaBytes,
        sensitiveTtlSecs: Long = this.sensitiveTtlSecs,
        collectPublicIp: Boolean = this.collectPublicIp,
        pasteAsPlainText: Boolean = this.pasteAsPlainText,
        excludedAppBundleIds: List<String> = this.excludedAppBundleIds,
    ): uniffi.copypaste_android.Config {
        val candidate = configDefaults.copy(
            maxTextSizeBytes = maxTextSizeBytes.coerceAtLeast(0L).toULong(),
            maxImageSizeBytes = maxImageSizeBytes.coerceAtLeast(0L).toULong(),
            maxFileSizeBytes = maxFileSizeBytes.coerceAtLeast(0L).toULong(),
            storageQuotaBytes = storageQuotaBytes.coerceAtLeast(0L).toULong(),
            sensitiveTtlSecs = sensitiveTtlSecs.coerceAtLeast(0L).toULong(),
            collectPublicIp = collectPublicIp,
            pasteAsPlainText = pasteAsPlainText,
            // Drop blank entries and de-dup so a clamped write never persists noise;
            // the native clampConfig may further normalise the list.
            excludedAppBundleIds = excludedAppBundleIds
                .map { it.trim() }
                .filter { it.isNotEmpty() }
                .distinct(),
        )
        return clampConfig(candidate)
    }

    /**
     * Clamp only [maxTextSizeBytes]/[maxImageSizeBytes]/[storageQuotaBytes] for
     * [Settings.saveScreenSettings], leaving the other knobs at their currently
     * stored values (mirrors the per-setter clamp behavior above).
     */
    fun clampConfigForSave(
        maxTextSizeBytes: Long,
        maxImageSizeBytes: Long,
        storageQuotaBytes: Long,
    ): uniffi.copypaste_android.Config = clampSizeKnobs(
        maxTextSizeBytes = maxTextSizeBytes,
        maxImageSizeBytes = maxImageSizeBytes,
        storageQuotaBytes = storageQuotaBytes,
    )

    /**
     * Maximum size in bytes for a text clipboard item. Items larger than this
     * are silently dropped at capture time. Default seeded from `defaultConfig()`
     * (MAX_TEXT_SIZE_BYTES). Writes are clamped via the native `clampConfig`.
     */
    var maxTextSizeBytes: Long
        get() = prefs.getLong("max_text_size_bytes", configDefaults.maxTextSizeBytes.toLong())
        set(v) = prefs.edit()
            .putLong("max_text_size_bytes", clampSizeKnobs(maxTextSizeBytes = v).maxTextSizeBytes.toLong())
            .apply()

    /**
     * Maximum size in bytes for an image clipboard item. Images larger than this
     * are silently dropped at capture time. Default seeded from `defaultConfig()`
     * (MAX_IMAGE_SIZE_BYTES). Writes are clamped via the native `clampConfig`.
     */
    var maxImageSizeBytes: Long
        get() = prefs.getLong("max_image_size_bytes", configDefaults.maxImageSizeBytes.toLong())
        set(v) = prefs.edit()
            .putLong("max_image_size_bytes", clampSizeKnobs(maxImageSizeBytes = v).maxImageSizeBytes.toLong())
            .apply()

    /**
     * Maximum size in bytes for a file clipboard item. Default seeded from
     * `defaultConfig()` (MAX_FILE_SIZE_BYTES — 100 MiB storable cap). Writes are
     * clamped via the native `clampConfig`. Added in the ABI-9 Config dict.
     */
    var maxFileSizeBytes: Long
        get() = prefs.getLong("max_file_size_bytes", configDefaults.maxFileSizeBytes.toLong())
        set(v) = prefs.edit()
            .putLong("max_file_size_bytes", clampSizeKnobs(maxFileSizeBytes = v).maxFileSizeBytes.toLong())
            .apply()

    /**
     * Total local storage quota for the clipboard database, in bytes. When the
     * database approaches this limit the oldest non-sensitive items should be
     * pruned by the repository — this is the SINGLE source of truth driving
     * retention (NOT the item-count caps). Default seeded from `defaultConfig()`
     * (STORAGE_QUOTA_BYTES — 10 GiB). Writes are clamped via `clampConfig`.
     * NOTE: 10 GiB exceeds Int range — the pref MUST be Long.
     */
    var storageQuotaBytes: Long
        get() = prefs.getLong("storage_quota_bytes", configDefaults.storageQuotaBytes.toLong())
        set(v) = prefs.edit()
            .putLong("storage_quota_bytes", clampSizeKnobs(storageQuotaBytes = v).storageQuotaBytes.toLong())
            .apply()

    /**
     * Time-to-live (seconds) before a sensitive clipboard item is auto-wiped.
     * Default seeded from `defaultConfig()` (SENSITIVE_TTL_SECS — 30 s). `0` is a
     * valid "auto-wipe disabled" sentinel and is intentionally NOT clamped up by
     * the daemon, so it survives `clampConfig`. Added in the ABI-9 Config dict.
     */
    var sensitiveTtlSecs: Long
        get() = prefs.getLong("sensitive_ttl_secs", configDefaults.sensitiveTtlSecs.toLong())
        set(v) = prefs.edit()
            .putLong("sensitive_ttl_secs", clampSizeKnobs(sensitiveTtlSecs = v).sensitiveTtlSecs.toLong())
            .apply()

    /**
     * Whether the daemon may issue a one-off STUN request to learn this device's
     * public IP (shown in the device-info card). Mirrors the macOS daemon's
     * `collect_public_ip` config field and "Discover public IP" toggle. Default
     * seeded from `defaultConfig()`. Writes go through the native `clampConfig`
     * for full parity even though this flag is not range-clamped.
     */
    var collectPublicIp: Boolean
        get() = prefs.getBoolean("collect_public_ip", configDefaults.collectPublicIp)
        set(v) = prefs.edit()
            .putBoolean("collect_public_ip", clampSizeKnobs(collectPublicIp = v).collectPublicIp)
            .apply()

    /**
     * Whether pasting strips rich formatting (RTF/HTML) and writes plain text only.
     * Mirrors the macOS daemon's `paste_as_plain_text` config field and the
     * "Paste as plain text" toggle. Default seeded from `defaultConfig()`. Writes
     * are routed through `clampConfig` for parity.
     */
    var pasteAsPlainText: Boolean
        get() = prefs.getBoolean("paste_as_plain_text", configDefaults.pasteAsPlainText)
        set(v) = prefs.edit()
            .putBoolean("paste_as_plain_text", clampSizeKnobs(pasteAsPlainText = v).pasteAsPlainText)
            .apply()

    /**
     * Bundle/package IDs of apps whose clipboard is never captured. Mirrors the
     * macOS daemon's `excluded_app_bundle_ids` config field and the editable
     * "Excluded apps" list. Persisted as a NUL-delimited string (NUL never occurs
     * in a bundle id) so arbitrary ids round-trip safely. Default seeded from
     * `defaultConfig()` (empty). Writes are trimmed/de-duped via `clampConfig`.
     */
    var excludedAppBundleIds: List<String>
        get() {
            val stored = prefs.getString(KEY_EXCLUDED_APP_BUNDLE_IDS, null)
                ?: return configDefaults.excludedAppBundleIds
            return stored.split(EXCLUDED_APP_DELIM).filter { it.isNotBlank() }
        }
        set(v) {
            val clamped = clampSizeKnobs(excludedAppBundleIds = v).excludedAppBundleIds
            prefs.edit()
                .putString(KEY_EXCLUDED_APP_BUNDLE_IDS, clamped.joinToString(EXCLUDED_APP_DELIM))
                .apply()
        }

    companion object {
        /**
         * Process-wide cache of the macOS-parity config defaults from the native
         * `defaultConfig()` (or its Kotlin fallback). Seeded once; immutable
         * defaults so a shared cache is safe across [ConfigKnobsStore] instances.
         */
        @Volatile
        private var cachedConfigDefaults: uniffi.copypaste_android.Config? = null

        private val configDefaultsLock = Any()

        // ── Excluded apps (privacy) ─────────────────────────────────────────────
        private const val KEY_EXCLUDED_APP_BUNDLE_IDS = "excluded_app_bundle_ids"

        /**
         * NUL delimiter for the joined [excludedAppBundleIds] pref string. NUL never
         * occurs in a package/bundle id, so it cannot collide with an entry.
         */
        private const val EXCLUDED_APP_DELIM = "\u0000"
    }
}
