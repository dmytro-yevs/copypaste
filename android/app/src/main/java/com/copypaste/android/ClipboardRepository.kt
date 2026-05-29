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

    /** Read fresh each store so a UI change to the cap takes effect immediately. */
    private val settings = Settings(context)

    /**
     * Guard for read-modify-write on the comma-joined "item_ids" index.
     * SharedPreferences is process-wide, so without this lock two coroutines
     * (UI delete + service insert) can both read the same baseline list and
     * the loser's update silently drops the winner's entry. See HIGH-8.
     */
    private val idsWriteLock = Any()

    /**
     * In-memory dedup window. Multiple OnPrimaryClipChangedListener owners
     * (ClipboardService, ClipboardAccessibilityService, MainActivity) each fire
     * on the same copy, so without this guard one copy creates 2-3 duplicate
     * rows (HIGH-3). We skip a store when an identical-content item was stored
     * within [DEDUP_WINDOW_MS]. The time window preserves the legitimate
     * "same text copied again later" case — re-copying after the window stores
     * a fresh row as expected.
     */
    @Volatile
    private var lastStoredHash: Int = 0

    @Volatile
    private var lastStoredAtMs: Long = 0L

    private val dedupLock = Any()

    suspend fun getItems(limit: Int = 50): List<ClipboardItem> = withContext(Dispatchers.IO) {
        val ids = storedIds().takeLast(limit)
        ids.mapNotNull { id ->
            val raw = prefs.getString("item_$id", null) ?: return@mapNotNull null
            parseItem(id, raw)
        }.reversed()
    }

    suspend fun deleteItem(id: String): Boolean = withContext(Dispatchers.IO) {
        synchronized(idsWriteLock) {
            val ids = storedIds().toMutableList()
            if (!ids.remove(id)) return@synchronized false
            prefs.edit()
                .remove("item_$id")
                .putString("item_ids", ids.joinToString(","))
                .apply()
            true
        }
    }

    /**
     * Encrypt [plaintext] with [key] and persist. Returns false when the text
     * is sensitive (checked via UniFFI or skipped when unavailable).
     *
     * The new UUID is generated BEFORE encryption so it can be bound into the
     * AEAD AAD on the v0.3 schema (see [encryptText]). The same id is also
     * used as the SharedPreferences storage key.
     */
    suspend fun storeItem(plaintext: String, key: ByteArray): Boolean = withContext(Dispatchers.IO) {
        if (plaintext.isBlank()) return@withContext false

        // ── HIGH-3: cross-listener dedup. The same physical copy fires the
        // clip-changed listener in every owner (FGS, a11y service, activity).
        // Skip if identical content was stored within the recent window so a
        // single copy yields a single row, while a later re-copy still stores.
        val hash = plaintext.hashCode()
        synchronized(dedupLock) {
            val now = System.currentTimeMillis()
            if (hash == lastStoredHash && now - lastStoredAtMs < DEDUP_WINDOW_MS) {
                Log.d(TAG, "Duplicate clip within ${DEDUP_WINDOW_MS}ms — skipping")
                return@withContext false
            }
            lastStoredHash = hash
            lastStoredAtMs = now
        }

        val sensitive = try {
            isSensitive(plaintext)
        } catch (_: UnsatisfiedLinkError) {
            false
        }
        if (sensitive) return@withContext false

        val id = UUID.randomUUID().toString()
        val blob = try {
            encryptText(id, plaintext.toByteArray(Charsets.UTF_8), key)
        } catch (e: IllegalStateException) {
            Log.d(TAG, "UniFFI unavailable (${e.message}) — using local AES-GCM fallback")
            localAesEncrypt(plaintext.toByteArray(Charsets.UTF_8), key)
        } catch (_: UnsatisfiedLinkError) {
            // Defensive — the bindings throw IllegalStateException, but a
            // future change could surface UnsatisfiedLinkError directly.
            Log.d(TAG, "UniFFI unavailable (UnsatisfiedLinkError) — using local AES-GCM fallback")
            localAesEncrypt(plaintext.toByteArray(Charsets.UTF_8), key)
        }

        val encoded = encodeItem(blob, plaintext.length)
        // ── HIGH-8: synchronize the read-modify-write so concurrent writers
        // cannot clobber each other's entries in the comma-joined index.
        synchronized(idsWriteLock) {
            val ids = storedIds().toMutableList().also { it.add(id) }

            // ── CRITICAL-1: enforce Settings.maxHistoryItems. Without this the
            // ids index and the per-item "item_<id>" prefs entries grew forever
            // (getItems only ever read the last 50, so the overflow was invisible
            // yet kept bloating the prefs file). Drop the oldest ids past the cap
            // and remove their backing entries in the same edit.
            val editor = prefs.edit()
            val maxItems = settings.maxHistoryItems.coerceAtLeast(1)
            if (ids.size > maxItems) {
                val dropCount = ids.size - maxItems
                repeat(dropCount) {
                    val droppedId = ids.removeAt(0)
                    editor.remove("item_$droppedId")
                }
            }
            editor
                .putString("item_$id", encoded)
                .putString("item_ids", ids.joinToString(","))
                .apply()
        }

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

        /** Window in which an identical-content store is treated as a duplicate. */
        private const val DEDUP_WINDOW_MS = 2_000L

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

    /**
     * Decrypt all locally stored items into [uniffi.copypaste_android.LocalItem]
     * values for a P2P sync push. Each stored blob is decrypted with [key] using
     * the item's id as AEAD AAD (the same id used at encrypt time).
     *
     * Items that fail to decrypt (e.g. produced by the local AES-GCM fallback
     * when the .so was absent, which UniFFI cannot read back) are skipped rather
     * than aborting the whole sync. Returns most-recent-first, capped at [limit].
     */
    suspend fun localItemsForSync(
        key: ByteArray,
        limit: Int = 200,
    ): List<uniffi.copypaste_android.LocalItem> = withContext(Dispatchers.IO) {
        val ids = storedIds().takeLast(limit)
        ids.mapNotNull { id ->
            val raw = prefs.getString("item_$id", null) ?: return@mapNotNull null
            try {
                val parts = raw.split("|")
                val wallTimeMs = parts[0].toLong()
                val contentType = parts[1]
                val nonce = Base64.decode(parts[3], Base64.NO_WRAP)
                val ciphertext = Base64.decode(parts[4], Base64.NO_WRAP)
                val plain = decryptText(id, ciphertext, nonce, key)
                uniffi.copypaste_android.LocalItem(
                    id = id,
                    wallTimeMs = wallTimeMs,
                    contentType = contentType,
                    plaintext = plain.map { it.toUByte() },
                )
            } catch (e: Exception) {
                Log.d(TAG, "Skipping item $id for sync (decrypt/parse failed): ${e.message}")
                null
            }
        }.reversed()
    }

    /**
     * Pull incoming relay items, decrypt each via UniFFI decryptText, and store
     * non-sensitive plaintext locally. Returns the list of decrypted strings that
     * were successfully received (storing may still be a no-op until the .so lands).
     */
    suspend fun syncItems(syncManager: SyncManager, encryptionKey: ByteArray): List<String> =
        withContext(Dispatchers.IO) {
            val decrypted = syncManager.syncIncoming(encryptionKey)
            decrypted.forEach { plaintext -> storeItem(plaintext, encryptionKey) }
            decrypted
        }
}
