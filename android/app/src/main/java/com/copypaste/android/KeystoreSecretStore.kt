package com.copypaste.android

import android.content.SharedPreferences
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import android.util.Log
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * Seam over the AndroidKeyStore-resident KEK (key-encryption-key) so the pure
 * wrap/unwrap-consuming logic in [KeystoreSecretStore] (and its collaborators
 * [PeerRosterStore] / [P2pIdentityStore]) can be exercised on the JVM with a
 * fake implementation — [KeyStore]/[Cipher] against the "AndroidKeyStore"
 * provider are unavailable outside a real device/emulator.
 *
 * CopyPaste-vp63.36: extracted so JUnit tests can inject a reversible fake KEK
 * and characterize secret/roster/identity migration + serialization behavior
 * without touching AndroidKeyStore.
 */
interface KekCipher {
    /** Wrap [raw] under the KEK. Returns (ciphertext, iv). */
    fun wrap(raw: ByteArray): Pair<ByteArray, ByteArray>

    /** Unwrap a ciphertext produced by [wrap]. Throws on failure (bad KEK/tamper). */
    fun unwrap(wrapped: ByteArray, iv: ByteArray): ByteArray
}

/**
 * Real [KekCipher] backed by an AndroidKeyStore-resident AES-256-GCM key.
 * The KEK never leaves the secure hardware/software keystore — only the
 * `wrap`/`unwrap` results (ciphertext + IV) pass through the JVM.
 */
class AndroidKeystoreKekCipher : KekCipher {
    override fun wrap(raw: ByteArray): Pair<ByteArray, ByteArray> {
        val cipher = Cipher.getInstance(KEK_TRANSFORMATION)
        cipher.init(Cipher.ENCRYPT_MODE, getOrCreateKek())
        val ct = cipher.doFinal(raw)
        return ct to cipher.iv
    }

    override fun unwrap(wrapped: ByteArray, iv: ByteArray): ByteArray {
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
        private const val KEYSTORE_PROVIDER = "AndroidKeyStore"
        private const val KEK_ALIAS = "copypaste_master_kek_v1"
        private const val KEK_TRANSFORMATION = "AES/GCM/NoPadding"
        private const val KEK_TAG_BITS = 128
    }
}

/**
 * Collaborator extracted from the [Settings] god-file (CopyPaste-vp63.36):
 * owns every KEK-wrapped secret domain (relay token, relay registration key,
 * cloud sync passphrase, directly-provisioned cloud sync key, Supabase
 * email/password) plus the local clipboard [encryptionKey], all funneled
 * through the same generic [readWrappedSecret]/[writeWrappedSecret]
 * migration helpers. [Settings] delegates to this store; every public
 * property keeps its original name/type/semantics (facade, zero call-site
 * churn).
 *
 * @param kek injected so tests can supply a fake, reversible cipher — the
 *   real [AndroidKeystoreKekCipher] requires a device/emulator.
 * @param b64 injected so tests can supply a real (JVM-side) base64 codec —
 *   see [Base64Codec] doc for why `android.util.Base64` itself is not usable
 *   in this project's plain-JUnit unit tests.
 */
class KeystoreSecretStore(
    private val prefs: SharedPreferences,
    private val kek: KekCipher = AndroidKeystoreKekCipher(),
    private val b64: Base64Codec = AndroidBase64Codec,
) {
    fun wrap(raw: ByteArray): Pair<ByteArray, ByteArray> = kek.wrap(raw)

    fun unwrap(wrapped: ByteArray, iv: ByteArray): ByteArray = kek.unwrap(wrapped, iv)

    /**
     * Server-issued relay bearer token (32 hex chars), persisted after a
     * successful [RelayClient.registerDevice]. See original doc on
     * [Settings.relayToken] (bd CopyPaste-44rq.53: KEK-wrapped, auto-migrates
     * a legacy plaintext value on first read).
     */
    var relayToken: String
        get() = readWrappedSecret(
            KEY_RELAY_TOKEN_WRAPPED_B64,
            KEY_RELAY_TOKEN_IV_B64,
            KEY_LEGACY_RELAY_TOKEN_PLAIN,
        )
        set(v) = writeWrappedSecret(
            KEY_RELAY_TOKEN_WRAPPED_B64,
            KEY_RELAY_TOKEN_IV_B64,
            KEY_LEGACY_RELAY_TOKEN_PLAIN,
            v,
        )

    /**
     * Base64 of a stable 32-byte relay registration identity value.
     * CopyPaste-44rq.57: KEK-wrapped, auto-migrates the legacy plaintext
     * "relay_registration_key_b64" value on first read.
     */
    var relayRegistrationKeyB64: String
        get() = readWrappedSecret(
            KEY_RELAY_REG_KEY_WRAPPED_B64,
            KEY_RELAY_REG_KEY_IV_B64,
            KEY_LEGACY_RELAY_REG_KEY_PLAIN,
        )
        set(v) = writeWrappedSecret(
            KEY_RELAY_REG_KEY_WRAPPED_B64,
            KEY_RELAY_REG_KEY_IV_B64,
            KEY_LEGACY_RELAY_REG_KEY_PLAIN,
            v,
        )

    /**
     * Shared sync passphrase for cross-device encryption (Argon2id-derived via
     * the Rust FFI `derive_cloud_sync_key`). DO NOT log or include in crash
     * reports.
     */
    var cloudSyncPassphrase: String
        get() = readWrappedSecret(
            KEY_PASSPHRASE_WRAPPED_B64,
            KEY_PASSPHRASE_IV_B64,
            KEY_LEGACY_PASSPHRASE_PLAIN,
        )
        set(v) = writeWrappedSecret(
            KEY_PASSPHRASE_WRAPPED_B64,
            KEY_PASSPHRASE_IV_B64,
            KEY_LEGACY_PASSPHRASE_PLAIN,
            v,
        )

    /**
     * Directly-provisioned 32-byte cloud sync key, KEK-wrapped at rest. Set
     * when a phone scans a configured PC's pairing QR (see original doc on
     * [Settings.cloudSyncKeyDirect]). Returns null when unset or unwrappable.
     * DO NOT log the bytes.
     */
    var cloudSyncKeyDirect: ByteArray?
        get() {
            val wrappedB64 = prefs.getString(KEY_CLOUD_SYNC_KEY_DIRECT_WRAPPED_B64, null) ?: return null
            val ivB64 = prefs.getString(KEY_CLOUD_SYNC_KEY_DIRECT_IV_B64, null) ?: return null
            return runCatching {
                unwrap(
                    wrapped = b64.decode(wrappedB64, Base64.NO_WRAP),
                    iv = b64.decode(ivB64, Base64.NO_WRAP),
                )
            }.getOrElse { e ->
                Log.w(TAG, "Failed to unwrap direct cloud sync key (${e.javaClass.simpleName})", e)
                null
            }
        }
        set(v) {
            if (v == null || v.isEmpty()) {
                prefs.edit()
                    .remove(KEY_CLOUD_SYNC_KEY_DIRECT_WRAPPED_B64)
                    .remove(KEY_CLOUD_SYNC_KEY_DIRECT_IV_B64)
                    .apply()
                return
            }
            val (wrapped, iv) = wrap(v)
            prefs.edit()
                .putString(KEY_CLOUD_SYNC_KEY_DIRECT_WRAPPED_B64, b64.encode(wrapped, Base64.NO_WRAP))
                .putString(KEY_CLOUD_SYNC_KEY_DIRECT_IV_B64, b64.encode(iv, Base64.NO_WRAP))
                .apply()
        }

    /**
     * Supabase account email for sign-in via GoTrue. CopyPaste-crh3.24:
     * KEK-wrapped at rest; auto-migrates the legacy plaintext "supabase_email"
     * value on first read.
     */
    var supabaseEmail: String
        get() = readWrappedSecret(
            KEY_SUPABASE_EMAIL_WRAPPED_B64,
            KEY_SUPABASE_EMAIL_IV_B64,
            KEY_LEGACY_SUPABASE_EMAIL_PLAIN,
        )
        set(v) = writeWrappedSecret(
            KEY_SUPABASE_EMAIL_WRAPPED_B64,
            KEY_SUPABASE_EMAIL_IV_B64,
            KEY_LEGACY_SUPABASE_EMAIL_PLAIN,
            v.trim(),
        )

    /**
     * Supabase account password for sign-in via GoTrue. DO NOT log or include
     * in crash reports.
     */
    var supabasePassword: String
        get() = readWrappedSecret(
            KEY_SUPABASE_PW_WRAPPED_B64,
            KEY_SUPABASE_PW_IV_B64,
            KEY_LEGACY_SUPABASE_PW_PLAIN,
        )
        set(v) = writeWrappedSecret(
            KEY_SUPABASE_PW_WRAPPED_B64,
            KEY_SUPABASE_PW_IV_B64,
            KEY_LEGACY_SUPABASE_PW_PLAIN,
            v,
        )

    /**
     * 256-bit AES key used for local clipboard encryption. See original doc
     * on [Settings.encryptionKey] — H4 process-wide RAM cache of the unwrapped
     * key, defensive copy handed out on every read.
     */
    val encryptionKey: ByteArray
        get() {
            cachedKey?.let { return it.copyOf() }
            synchronized(keyCacheLock) {
                cachedKey?.let { return it.copyOf() }
                val key = loadOrCreateKey()
                cachedKey = key
                return key.copyOf()
            }
        }

    /** H4: drop the cached master key (called from [Settings.clear]). */
    fun clearCachedKey() {
        synchronized(keyCacheLock) { cachedKey = null }
    }

    /**
     * Unwrap (or migrate/generate) the 32-byte encryption key. See original
     * doc on [Settings.loadOrCreateKey] (CopyPaste-gkgp: throws
     * [EncryptionKeyLostException] instead of silently regenerating a key
     * when a wrapped key exists but cannot be unwrapped).
     */
    @Throws(EncryptionKeyLostException::class)
    private fun loadOrCreateKey(): ByteArray {
        run {
            val wrappedB64 = prefs.getString(KEY_WRAPPED_KEY_B64, null)
            val ivB64 = prefs.getString(KEY_WRAPPED_KEY_IV_B64, null)
            if (wrappedB64 != null && ivB64 != null) {
                return try {
                    unwrap(
                        wrapped = b64.decode(wrappedB64, Base64.DEFAULT),
                        iv = b64.decode(ivB64, Base64.DEFAULT)
                    )
                } catch (e: Exception) {
                    // DO NOT delete the wrapped key blob — it is the only handle
                    // to the existing ciphertexts. Throwing gives the caller a
                    // chance to surface a "History unavailable" degraded state.
                    Log.e(
                        TAG,
                        "CopyPaste-gkgp: CRITICAL — encryption key unwrap failed " +
                            "(${e.javaClass.simpleName}). History is locked; " +
                            "NOT regenerating key to preserve existing data.",
                        e,
                    )
                    throw EncryptionKeyLostException(
                        "Encryption key unwrap failed (${e.javaClass.simpleName}): ${e.message}",
                        e,
                    )
                }
            }

            // Legacy migration: a previous build persisted the raw key in
            // plain SharedPreferences. Read it, wrap, then scrub the plain
            // copy so an attacker reading the prefs file post-upgrade cannot
            // recover the key.
            val legacyPlain = prefs.getString(KEY_LEGACY_PLAIN_KEY_B64, null)
            val key = if (legacyPlain != null) {
                Log.i(TAG, "Migrating plain encryption key into AndroidKeyStore wrap")
                b64.decode(legacyPlain, Base64.DEFAULT)
            } else {
                // True first run — no key of any kind exists.
                ByteArray(32).also { java.security.SecureRandom().nextBytes(it) }
            }

            val (wrapped, iv) = wrap(key)
            val editor = prefs.edit()
                .putString(KEY_WRAPPED_KEY_B64, b64.encode(wrapped, Base64.DEFAULT))
                .putString(KEY_WRAPPED_KEY_IV_B64, b64.encode(iv, Base64.DEFAULT))
            if (legacyPlain != null) {
                editor.remove(KEY_LEGACY_PLAIN_KEY_B64)
            }
            editor.apply()
            return key
        }
    }

    /**
     * Read a KEK-wrapped UTF-8 string secret stored under [wrappedKey]/[ivKey],
     * migrating any pre-existing plaintext value held under [legacyPlainKey].
     * See original doc on [Settings.readWrappedSecret] for the resolution order.
     */
    private fun readWrappedSecret(
        wrappedKey: String,
        ivKey: String,
        legacyPlainKey: String,
    ): String {
        val wrappedB64 = prefs.getString(wrappedKey, null)
        val ivB64 = prefs.getString(ivKey, null)
        if (wrappedB64 != null && ivB64 != null) {
            return runCatching {
                String(
                    unwrap(
                        wrapped = b64.decode(wrappedB64, Base64.DEFAULT),
                        iv = b64.decode(ivB64, Base64.DEFAULT),
                    ),
                    Charsets.UTF_8,
                )
            }.getOrElse { e ->
                Log.w(TAG, "Failed to unwrap secret '$wrappedKey' (${e.javaClass.simpleName})", e)
                ""
            }
        }

        // Migration: a previous build persisted this secret in plain prefs.
        val legacyPlain = prefs.getString(legacyPlainKey, null)
        if (legacyPlain != null && legacyPlain.isNotEmpty()) {
            Log.i(TAG, "Migrating plain secret '$legacyPlainKey' into AndroidKeyStore wrap")
            writeWrappedSecret(wrappedKey, ivKey, legacyPlainKey, legacyPlain)
            return legacyPlain
        }
        return ""
    }

    /**
     * Wrap [value] with the KEK and persist under [wrappedKey]/[ivKey], scrubbing
     * any legacy plaintext under [legacyPlainKey]. An empty [value] clears all
     * three keys (logical "unset").
     */
    private fun writeWrappedSecret(
        wrappedKey: String,
        ivKey: String,
        legacyPlainKey: String,
        value: String,
    ) {
        if (value.isEmpty()) {
            prefs.edit()
                .remove(wrappedKey)
                .remove(ivKey)
                .remove(legacyPlainKey)
                .apply()
            return
        }
        val (wrapped, iv) = wrap(value.toByteArray(Charsets.UTF_8))
        prefs.edit()
            .putString(wrappedKey, b64.encode(wrapped, Base64.DEFAULT))
            .putString(ivKey, b64.encode(iv, Base64.DEFAULT))
            .remove(legacyPlainKey)
            .apply()
    }

    companion object {
        private const val TAG = "KeystoreSecretStore"

        /**
         * H4: process-wide cache of the unwrapped 32-byte encryption key. Lives
         * in the companion (not an instance field) because the app constructs
         * many short-lived [Settings]/[KeystoreSecretStore] objects — caching
         * per instance would still re-unwrap on each new object. RAM-only,
         * dies with the process.
         */
        @Volatile
        private var cachedKey: ByteArray? = null

        private val keyCacheLock = Any()

        private const val KEY_WRAPPED_KEY_B64 = "encryption_key_wrapped_b64"
        private const val KEY_WRAPPED_KEY_IV_B64 = "encryption_key_iv_b64"
        private const val KEY_LEGACY_PLAIN_KEY_B64 = "encryption_key_b64"

        // ── KEK-wrapped cloud secrets (passphrase + Supabase password) ──────────
        // Plaintext pref keys retained for read-time migration only.
        private const val KEY_LEGACY_PASSPHRASE_PLAIN = "cloud_sync_passphrase"
        private const val KEY_PASSPHRASE_WRAPPED_B64 = "cloud_sync_passphrase_wrapped_b64"
        private const val KEY_PASSPHRASE_IV_B64 = "cloud_sync_passphrase_iv_b64"

        private const val KEY_LEGACY_SUPABASE_PW_PLAIN = "supabase_password"
        private const val KEY_SUPABASE_PW_WRAPPED_B64 = "supabase_password_wrapped_b64"
        private const val KEY_SUPABASE_PW_IV_B64 = "supabase_password_iv_b64"

        // CopyPaste-crh3.24: the Supabase account email is PII and was the only
        // cloud secret still stored as raw plaintext. KEK-wrap it like the
        // password (legacy plain key "supabase_email" auto-migrates on first read).
        private const val KEY_LEGACY_SUPABASE_EMAIL_PLAIN = "supabase_email"
        private const val KEY_SUPABASE_EMAIL_WRAPPED_B64 = "supabase_email_wrapped_b64"
        private const val KEY_SUPABASE_EMAIL_IV_B64 = "supabase_email_iv_b64"

        // bd CopyPaste-44rq.53: KEK-wrapped relay bearer token.
        private const val KEY_LEGACY_RELAY_TOKEN_PLAIN = "relay_token"
        private const val KEY_RELAY_TOKEN_WRAPPED_B64 = "relay_token_wrapped_b64"
        private const val KEY_RELAY_TOKEN_IV_B64 = "relay_token_iv_b64"

        // CopyPaste-44rq.57: KEK-wrapped relay registration key (was plaintext).
        private const val KEY_LEGACY_RELAY_REG_KEY_PLAIN = "relay_registration_key_b64"
        private const val KEY_RELAY_REG_KEY_WRAPPED_B64 = "relay_reg_key_wrapped_b64"
        private const val KEY_RELAY_REG_KEY_IV_B64 = "relay_reg_key_iv_b64"

        // KEK-wrapped, directly-provisioned 32-byte cloud sync key. Carried over
        // QR pairing (see PairActivity) so a scanning phone can decrypt cloud rows
        // without the user re-typing the passphrase. Raw bytes never persisted.
        private const val KEY_CLOUD_SYNC_KEY_DIRECT_WRAPPED_B64 = "cloud_sync_key_direct_wrapped_b64"
        private const val KEY_CLOUD_SYNC_KEY_DIRECT_IV_B64 = "cloud_sync_key_direct_iv_b64"
    }
}
