package com.copypaste.android

import android.content.Context
import android.content.SharedPreferences
import android.util.Base64
import android.util.Log
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.util.UUID
import javax.crypto.Cipher
import javax.crypto.spec.GCMParameterSpec
import javax.crypto.spec.SecretKeySpec

/**
 * Persists clipboard items to SharedPreferences.
 *
 * Each item is stored as a JSON-like string under key "item_<uuid>" so it
 * survives process death without requiring Room or a .so binary.
 * An ordered index of ids is kept under "item_ids" (comma-separated).
 *
 * Encryption is attempted via UniFFI [encryptText]; on [UnsatisfiedLinkError]
 * (e.g. during unit tests or before .so is built) it falls back to
 * [localAesEncrypt] which uses AES-256-GCM via the Android KeyStore provider.
 */
class ClipboardRepository(context: Context) {

    private val prefs: SharedPreferences =
        context.getSharedPreferences("copypaste_items", Context.MODE_PRIVATE)

    suspend fun getItems(limit: Int = 50): List<ClipboardItem> = withContext(Dispatchers.IO) {
        val ids = storedIds().takeLast(limit)
        ids.mapNotNull { id ->
            val raw = prefs.getString("item_$id", null) ?: return@mapNotNull null
            parseItem(id, raw)
        }.reversed()
    }

    suspend fun deleteItem(id: String): Boolean = withContext(Dispatchers.IO) {
        val ids = storedIds().toMutableList()
        if (!ids.remove(id)) return@withContext false
        prefs.edit()
            .remove("item_$id")
            .putString("item_ids", ids.joinToString(","))
            .apply()
        true
    }

    /**
     * Encrypt [plaintext] with [key] and persist. Returns false when the text
     * is sensitive (checked via UniFFI or skipped when unavailable).
     */
    suspend fun storeItem(plaintext: String, key: ByteArray): Boolean = withContext(Dispatchers.IO) {
        if (plaintext.isBlank()) return@withContext false

        val sensitive = try { isSensitive(plaintext) } catch (_: UnsatisfiedLinkError) { false }
        if (sensitive) return@withContext false

        val blob = try {
            encryptText(plaintext.toByteArray(Charsets.UTF_8), key)
        } catch (_: UnsatisfiedLinkError) {
            Log.d(TAG, "UniFFI unavailable — using local AES-GCM fallback")
            localAesEncrypt(plaintext.toByteArray(Charsets.UTF_8), key)
        }

        val id = UUID.randomUUID().toString()
        val encoded = encodeItem(blob, plaintext.length)
        val ids = storedIds().toMutableList().also { it.add(id) }

        prefs.edit()
            .putString("item_$id", encoded)
            .putString("item_ids", ids.joinToString(","))
            .apply()

        Log.d(TAG, "Stored item $id (${plaintext.length} chars)")
        true
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

    private fun storedIds(): List<String> =
        prefs.getString("item_ids", "")
            ?.split(",")
            ?.filter { it.isNotBlank() }
            ?: emptyList()

    /**
     * Encode a stored item as a pipe-delimited string:
     * <wallTimeMs>|<contentType>|<snippetLen>|<nonceB64>|<ciphertextB64>
     */
    private fun encodeItem(blob: EncryptedBlob, plaintextLen: Int): String {
        val nonce64 = Base64.encodeToString(blob.nonce, Base64.NO_WRAP)
        val ct64 = Base64.encodeToString(blob.ciphertext, Base64.NO_WRAP)
        val ts = System.currentTimeMillis()
        return "$ts|text/plain|$plaintextLen|$nonce64|$ct64"
    }

    private fun parseItem(id: String, raw: String): ClipboardItem? {
        return try {
            val parts = raw.split("|")
            ClipboardItem(
                id = id,
                contentType = parts[1],
                isSensitive = false,
                wallTimeMs = parts[0].toLong(),
                snippet = "(${parts[2]} chars)"
            )
        } catch (e: Exception) {
            Log.w(TAG, "Failed to parse item $id: ${e.message}")
            null
        }
    }

    companion object {
        private const val TAG = "ClipboardRepository"
        private const val AES_TRANSFORMATION = "AES/GCM/NoPadding"
        private const val GCM_TAG_BITS = 128
        private const val GCM_NONCE_BYTES = 12

        /**
         * AES-256-GCM encryption using only javax.crypto — no native dep.
         * Used as fallback when UniFFI .so is not yet loaded.
         */
        fun localAesEncrypt(plaintext: ByteArray, key: ByteArray): EncryptedBlob {
            val nonce = ByteArray(GCM_NONCE_BYTES).also {
                java.security.SecureRandom().nextBytes(it)
            }
            val cipher = Cipher.getInstance(AES_TRANSFORMATION)
            cipher.init(
                Cipher.ENCRYPT_MODE,
                SecretKeySpec(key.copyOf(32), "AES"),
                GCMParameterSpec(GCM_TAG_BITS, nonce)
            )
            val ciphertext = cipher.doFinal(plaintext)
            return EncryptedBlob(nonce = nonce, ciphertext = ciphertext)
        }
    }
}
