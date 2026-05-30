package com.copypaste.android

import android.content.Context
import android.content.SharedPreferences
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import android.util.Log
import java.security.KeyStore
import java.security.SecureRandom
import java.util.UUID
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/** Which sync transport backend to use when sync is enabled. */
enum class SyncBackend {
    /** Original custom relay server (pair-based, local-network-friendly). */
    RELAY,
    /** Supabase PostgREST + GoTrue auth (cross-device, cloud-based, end-to-end encrypted). */
    SUPABASE,
}

class Settings(context: Context) {
    private val prefs: SharedPreferences = context.getSharedPreferences("copypaste", Context.MODE_PRIVATE)

    /**
     * HIGH-7: subscribe to live updates of any pref. The SharedPreferences
     * getter is already process-local synchronous (each read returns the
     * current in-memory value), but writes from a UI coroutine and reads
     * from the service's IO coroutine can interleave such that the service
     * acts on a stale snapshot it captured into a local val.
     *
     * Callers that need to react to changes (e.g. ClipboardService toggling
     * its behaviour the moment the user flips a switch in SettingsActivity)
     * should subscribe via this helper and unsubscribe in onDestroy /
     * coroutine cancellation. The returned [SharedPreferences.OnSharedPreferenceChangeListener]
     * must be retained by the caller — SharedPreferences holds a weak
     * reference to it.
     */
    fun observe(
        listener: SharedPreferences.OnSharedPreferenceChangeListener
    ): SharedPreferences.OnSharedPreferenceChangeListener {
        prefs.registerOnSharedPreferenceChangeListener(listener)
        return listener
    }

    fun stopObserving(listener: SharedPreferences.OnSharedPreferenceChangeListener) {
        prefs.unregisterOnSharedPreferenceChangeListener(listener)
    }

    var relayUrl: String
        get() = prefs.getString("relay_url", "http://localhost:8080") ?: "http://localhost:8080"
        set(v) = prefs.edit().putString("relay_url", v).apply()

    var syncEnabled: Boolean
        get() = prefs.getBoolean("sync_enabled", false)
        set(v) = prefs.edit().putBoolean("sync_enabled", v).apply()

    // ── Supabase cloud sync ─────────────────────────────────────────────────

    /**
     * Supabase project URL, e.g. `https://abc.supabase.co`.
     * Must use HTTPS. When blank, Supabase sync is disabled.
     * Mirrors `CloudConfig::supabase_url` on the macOS daemon side.
     */
    var supabaseUrl: String
        get() = prefs.getString("supabase_url", "") ?: ""
        set(v) = prefs.edit().putString("supabase_url", v.trimEnd('/')).apply()

    /**
     * Supabase anonymous/public API key (`anon` role key from the project dashboard).
     * Used as the `apikey` header on every REST request.
     * Mirrors `CloudConfig::anon_key` on the macOS daemon side.
     */
    var supabaseAnonKey: String
        get() = prefs.getString("supabase_anon_key", "") ?: ""
        set(v) = prefs.edit().putString("supabase_anon_key", v).apply()

    /**
     * Shared sync passphrase for cross-device encryption.
     *
     * This value is run through Argon2id (via the Rust FFI [derive_cloud_sync_key])
     * to produce a 32-byte symmetric key used with XChaCha20-Poly1305 AEAD.
     * The SAME passphrase entered on macOS and Android will derive the SAME key,
     * enabling bidirectional decryption of cloud blobs.
     *
     * Security: persisted in SharedPreferences (protected by the device lock screen
     * on Android 6+). For higher security, clear this field when the app is
     * backgrounded and re-prompt on next launch.
     *
     * DO NOT log or include in crash reports.
     */
    var cloudSyncPassphrase: String
        get() = prefs.getString("cloud_sync_passphrase", "") ?: ""
        set(v) = prefs.edit().putString("cloud_sync_passphrase", v).apply()

    /**
     * Which sync backend to use when [syncEnabled] is true.
     * - [SyncBackend.RELAY]    — custom relay server (original, local-network-friendly)
     * - [SyncBackend.SUPABASE] — Supabase PostgREST (cross-device, cloud-based)
     */
    var syncBackend: SyncBackend
        get() = when (prefs.getString("sync_backend", SyncBackend.RELAY.name)) {
            SyncBackend.SUPABASE.name -> SyncBackend.SUPABASE
            else -> SyncBackend.RELAY
        }
        set(v) = prefs.edit().putString("sync_backend", v.name).apply()

    /**
     * Supabase account email for sign-in via GoTrue.
     * Optional: when blank the anonKey is used as bearer (no Row Level Security).
     */
    var supabaseEmail: String
        get() = prefs.getString("supabase_email", "") ?: ""
        set(v) = prefs.edit().putString("supabase_email", v.trim()).apply()

    /**
     * Supabase account password for sign-in via GoTrue.
     * Stored in SharedPreferences (protected by device lock on Android 6+).
     * DO NOT log or include in crash reports.
     */
    var supabasePassword: String
        get() = prefs.getString("supabase_password", "") ?: ""
        set(v) = prefs.edit().putString("supabase_password", v).apply()

    /** Returns true when Supabase sync is fully configured: URL, anon key, and passphrase. */
    val isSupabaseConfigured: Boolean
        get() = supabaseUrl.startsWith("https://") &&
                supabaseAnonKey.isNotBlank() &&
                cloudSyncPassphrase.isNotBlank()

    /** Returns true when Supabase email+password are both non-empty. */
    val hasSupabaseCredentials: Boolean
        get() = supabaseEmail.isNotBlank() && supabasePassword.isNotBlank()

    /**
     * Compound keyset cursor for the Supabase ascending poll.
     *
     * Both fields are advanced together for EVERY row in a batch (including
     * self-echo and blank rows) BEFORE any `continue`, so a batch that
     * contains only own-device rows still advances the cursor and does not
     * re-fetch the same window on the next poll.
     *
     * Mirror of the `(last_wall_time, last_id)` cursor in the macOS daemon's
     * cloud.rs `build_poll_url`. PostgREST keyset filter:
     *   or=(wall_time.gt.W,and(wall_time.eq.W,id.gt.ID))
     * with order=wall_time.asc,id.asc.
     */
    var lastSupabasePollWallTime: Long
        get() = prefs.getLong("supabase_last_poll_wall_time", 0L)
        set(v) = prefs.edit().putLong("supabase_last_poll_wall_time", v).apply()

    /**
     * Row `id` (UUID string) of the last processed Supabase poll row.
     * Combined with [lastSupabasePollWallTime] to form the compound keyset
     * cursor — prevents burst-loss when >20 rows share the same wall_time.
     * Empty string means "no rows seen yet" (initial state).
     */
    var lastSupabasePollId: String
        get() = prefs.getString("supabase_last_poll_id", "") ?: ""
        set(v) = prefs.edit().putString("supabase_last_poll_id", v).apply()

    val deviceId: String
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
     *
     * Storage: the raw 32 random bytes are wrapped with an AndroidKeyStore-
     * resident AES-256-GCM KEK (the KEK never leaves the secure hardware /
     * software keystore — only its `wrap` and `unwrap` results pass through
     * the JVM). The wrapped blob and its IV are persisted in
     * SharedPreferences as base64.
     *
     * Migration: any pre-existing `encryption_key_b64` (plain key from a
     * previous build) is read once, immediately wrapped with the KEK, and
     * the plain value is removed from SharedPreferences. This preserves
     * already-stored clipboard items across the upgrade.
     */
    val encryptionKey: ByteArray
        get() {
            // H4: the unwrapped key is cached in RAM after the first AndroidKeyStore
            // GCM unwrap. Without this cache, every call (each clipboard store, each
            // sync poll, each FGS tick) performed a fresh AES-GCM keystore unwrap —
            // a hardware/software keystore round-trip that is needlessly expensive
            // on a hot path. The cache is process-local, never persisted, and dies
            // with the process. We hand out a defensive copy so a caller mutating
            // the returned array cannot corrupt the cached master key.
            cachedKey?.let { return it.copyOf() }
            synchronized(keyCacheLock) {
                cachedKey?.let { return it.copyOf() }
                val key = loadOrCreateKey()
                cachedKey = key
                return key.copyOf()
            }
        }

    /**
     * Unwrap (or migrate/generate) the 32-byte encryption key. Callers go through
     * the cached [encryptionKey] accessor; this does the actual keystore work and
     * is invoked at most once per process under [keyCacheLock].
     */
    private fun loadOrCreateKey(): ByteArray {
        run {
            // Already migrated → unwrap and return.
            val wrappedB64 = prefs.getString(KEY_WRAPPED_KEY_B64, null)
            val ivB64 = prefs.getString(KEY_WRAPPED_KEY_IV_B64, null)
            if (wrappedB64 != null && ivB64 != null) {
                runCatching {
                    return unwrapKey(
                        wrapped = Base64.decode(wrappedB64, Base64.DEFAULT),
                        iv = Base64.decode(ivB64, Base64.DEFAULT)
                    )
                }.onFailure { e ->
                    // KEK lost (e.g. user cleared keystore, or backup/restore
                    // to a different device). Best we can do is regenerate;
                    // already-stored ciphertexts will become unreadable, but
                    // the alternative (return random key silently or crash)
                    // is worse.
                    Log.w(TAG, "Failed to unwrap encryption key (${e.javaClass.simpleName}); regenerating", e)
                    prefs.edit()
                        .remove(KEY_WRAPPED_KEY_B64)
                        .remove(KEY_WRAPPED_KEY_IV_B64)
                        .apply()
                }
            }

            // Legacy migration: a previous build persisted the raw key in
            // plain SharedPreferences. Read it, wrap, then scrub the plain
            // copy so an attacker reading the prefs file post-upgrade cannot
            // recover the key.
            val legacyPlain = prefs.getString(KEY_LEGACY_PLAIN_KEY_B64, null)
            val key = if (legacyPlain != null) {
                Log.i(TAG, "Migrating plain encryption key into AndroidKeyStore wrap")
                Base64.decode(legacyPlain, Base64.DEFAULT)
            } else {
                ByteArray(32).also { SecureRandom().nextBytes(it) }
            }

            val (wrapped, iv) = wrapKey(key)
            val editor = prefs.edit()
                .putString(KEY_WRAPPED_KEY_B64, Base64.encodeToString(wrapped, Base64.DEFAULT))
                .putString(KEY_WRAPPED_KEY_IV_B64, Base64.encodeToString(iv, Base64.DEFAULT))
            if (legacyPlain != null) {
                editor.remove(KEY_LEGACY_PLAIN_KEY_B64)
            }
            editor.apply()
            return key
        }
    }

    /**
     * Pinned fingerprint of the most recently paired P2P peer, or empty when no
     * device has been paired. Stored after a successful [PairActivity] sync so a
     * future sync can target the same peer without re-scanning a QR.
     */
    var pairedPeerFingerprint: String
        get() = prefs.getString("paired_peer_fingerprint", "") ?: ""
        set(v) = prefs.edit().putString("paired_peer_fingerprint", v).apply()

    /**
     * Sync-listener address (host:port) reported by the most recently paired
     * peer during bootstrap. The macOS daemon now advertises a real LAN-routable
     * host:port (not loopback), so this is dialable from a real phone on the same
     * Wi-Fi by [FgsSyncLoop]'s background dialer. It may still be loopback when
     * the Mac has no LAN interface at all (e.g. emulator-only setups).
     */
    var pairedPeerSyncAddr: String
        get() = prefs.getString("paired_peer_sync_addr", "") ?: ""
        set(v) = prefs.edit().putString("paired_peer_sync_addr", v).apply()

    /**
     * 32-byte PAKE session key derived during the last successful [PairActivity]
     * bootstrap. Required (alongside [pairedPeerFingerprint] and
     * [pairedPeerSyncAddr]) for the background P2P dialer to re-establish a sync
     * session with the paired peer without re-scanning a QR.
     *
     * Storage mirrors [encryptionKey]: the raw bytes are wrapped with the
     * AndroidKeyStore-resident KEK so the secret never sits in SharedPreferences
     * in cleartext. Reading returns an empty array when unset or unwrappable.
     *
     * DO NOT log or include in crash reports.
     */
    var pairedPeerSessionKey: ByteArray
        get() {
            val wrappedB64 = prefs.getString(KEY_SESSION_WRAPPED_B64, null) ?: return ByteArray(0)
            val ivB64 = prefs.getString(KEY_SESSION_IV_B64, null) ?: return ByteArray(0)
            return runCatching {
                unwrapKey(
                    wrapped = Base64.decode(wrappedB64, Base64.DEFAULT),
                    iv = Base64.decode(ivB64, Base64.DEFAULT),
                )
            }.getOrElse { e ->
                Log.w(TAG, "Failed to unwrap paired-peer session key (${e.javaClass.simpleName})", e)
                ByteArray(0)
            }
        }
        set(v) {
            if (v.isEmpty()) {
                prefs.edit()
                    .remove(KEY_SESSION_WRAPPED_B64)
                    .remove(KEY_SESSION_IV_B64)
                    .apply()
                return
            }
            val (wrapped, iv) = wrapKey(v)
            prefs.edit()
                .putString(KEY_SESSION_WRAPPED_B64, Base64.encodeToString(wrapped, Base64.DEFAULT))
                .putString(KEY_SESSION_IV_B64, Base64.encodeToString(iv, Base64.DEFAULT))
                .apply()
        }

    fun clear() {
        // H4: drop the cached master key so a re-created key after clear() is
        // not shadowed by a stale RAM copy.
        synchronized(keyCacheLock) { cachedKey = null }
        prefs.edit().clear().apply()
    }

    // ── AndroidKeyStore KEK helpers ─────────────────────────────────────────

    /**
     * Wrap [raw] with the KeyStore-resident KEK. Returns (ciphertext, iv).
     * The IV is generated by the KeyStore provider (12 bytes for GCM).
     */
    private fun wrapKey(raw: ByteArray): Pair<ByteArray, ByteArray> {
        val cipher = Cipher.getInstance(KEK_TRANSFORMATION)
        cipher.init(Cipher.ENCRYPT_MODE, getOrCreateKek())
        val ct = cipher.doFinal(raw)
        return ct to cipher.iv
    }

    private fun unwrapKey(wrapped: ByteArray, iv: ByteArray): ByteArray {
        val cipher = Cipher.getInstance(KEK_TRANSFORMATION)
        cipher.init(Cipher.DECRYPT_MODE, getOrCreateKek(), GCMParameterSpec(KEK_TAG_BITS, iv))
        return cipher.doFinal(wrapped)
    }

    private fun getOrCreateKek(): SecretKey {
        val keystore = KeyStore.getInstance(KEYSTORE_PROVIDER).apply { load(null) }
        (keystore.getKey(KEK_ALIAS, null) as? SecretKey)?.let { return it }

        val keygen = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, KEYSTORE_PROVIDER)
        keygen.init(
            KeyGenParameterSpec.Builder(
                KEK_ALIAS,
                KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT
            )
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256)
                // No user-auth requirement — the service runs headless. The
                // KEK is bound to the device's secure storage but does not
                // require an unlocked screen to use.
                .setRandomizedEncryptionRequired(true)
                .build()
        )
        return keygen.generateKey()
    }

    companion object {
        private const val TAG = "Settings"

        /**
         * H4: process-wide cache of the unwrapped 32-byte encryption key.
         *
         * Lives in the companion (not a [Settings] instance field) because the
         * app constructs many short-lived [Settings] objects — caching per
         * instance would still re-unwrap on each new object. The cache is
         * RAM-only (never written to prefs/disk) and dies with the process.
         */
        @Volatile
        private var cachedKey: ByteArray? = null

        private val keyCacheLock = Any()

        private const val KEYSTORE_PROVIDER = "AndroidKeyStore"
        private const val KEK_ALIAS = "copypaste_master_kek_v1"
        private const val KEK_TRANSFORMATION = "AES/GCM/NoPadding"
        private const val KEK_TAG_BITS = 128
        private const val KEY_WRAPPED_KEY_B64 = "encryption_key_wrapped_b64"
        private const val KEY_WRAPPED_KEY_IV_B64 = "encryption_key_iv_b64"
        private const val KEY_LEGACY_PLAIN_KEY_B64 = "encryption_key_b64"
        private const val KEY_SESSION_WRAPPED_B64 = "paired_peer_session_key_wrapped_b64"
        private const val KEY_SESSION_IV_B64 = "paired_peer_session_key_iv_b64"
    }
}
