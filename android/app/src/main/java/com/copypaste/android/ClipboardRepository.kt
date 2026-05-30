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
            // WARN: AES-GCM fallback is only safe during development/testing.
            // In production, the native .so MUST be present so items use
            // XChaCha20-Poly1305 (compatible with the macOS daemon). A local
            // AES-GCM-encrypted item CANNOT be synced to or from the desktop.
            Log.w(TAG, "UniFFI unavailable (${e.message}) — using local AES-GCM fallback (NOT sync-compatible)")
            localAesEncrypt(plaintext.toByteArray(Charsets.UTF_8), key)
        } catch (_: UnsatisfiedLinkError) {
            // Defensive — the bindings throw IllegalStateException, but a
            // future change could surface UnsatisfiedLinkError directly.
            Log.w(TAG, "UniFFI unavailable (UnsatisfiedLinkError) — using local AES-GCM fallback (NOT sync-compatible)")
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
                    // Also remove the reverse-lookup entry for evicted items.
                    editor.remove("item_id_ref_$droppedId")
                }
            }
            editor
                .putString("item_$id", encoded)
                .putString("item_ids", ids.joinToString(","))
                // Reverse-lookup: item_id → storage_id for LWW cloud sync.
                // For locally-captured items the storage id IS the item_id.
                .putString("item_id_ref_$id", id)
                .apply()
        }

        Log.d(TAG, "Stored item $id (${plaintext.length} chars)")
        true
    }

    /**
     * Store a cloud-synced item with Last-Writer-Wins semantics (Task 5).
     *
     * [itemId] is the stable UUID from the `item_id` column (same across devices).
     * [incomingLamportTs] is the lamport_ts from the cloud row (Unix-ms on both
     * sides, so the compare is valid cross-platform).
     *
     * Behaviour:
     * - If [itemId] is not yet stored locally → store as a new item (same as
     *   [storeItem]).
     * - If [itemId] already exists locally AND [incomingLamportTs] is strictly
     *   greater than the stored lamport_ts → replace the stored row in-place
     *   (re-encrypt with [key], keep the same storage id in the index).
     * - Otherwise (equal or older lamport_ts) → skip as a dup.
     *
     * Returns true when a new row was inserted or an existing row was replaced.
     */
    suspend fun storeItemWithLww(
        plaintext: String,
        key: ByteArray,
        itemId: String,
        incomingLamportTs: Long,
    ): Boolean = withContext(Dispatchers.IO) {
        if (plaintext.isBlank()) return@withContext false

        val sensitive = try { isSensitive(plaintext) } catch (_: UnsatisfiedLinkError) { false }
        if (sensitive) return@withContext false

        // Look up whether this item_id already has a storage entry.
        // The reverse-lookup key is "item_id_ref_<itemId>" → storageId.
        val existingStorageId = prefs.getString("item_id_ref_$itemId", null)

        if (existingStorageId != null) {
            // LWW: only replace when incoming lamport_ts is strictly newer.
            val storedTs = storedLamportTs(existingStorageId)
            if (incomingLamportTs <= storedTs) {
                Log.d(TAG, "LWW: skipping dup item_id=$itemId (stored=$storedTs, incoming=$incomingLamportTs)")
                return@withContext false
            }
            // Replace in-place: re-encrypt and overwrite the stored value.
            val blob = try {
                encryptText(existingStorageId, plaintext.toByteArray(Charsets.UTF_8), key)
            } catch (e: IllegalStateException) {
                Log.w(TAG, "LWW replace: UniFFI unavailable — using local AES-GCM fallback (NOT sync-compatible)")
                ClipboardRepository.localAesEncrypt(plaintext.toByteArray(Charsets.UTF_8), key)
            } catch (_: UnsatisfiedLinkError) {
                Log.w(TAG, "LWW replace: UnsatisfiedLinkError — using local AES-GCM fallback (NOT sync-compatible)")
                ClipboardRepository.localAesEncrypt(plaintext.toByteArray(Charsets.UTF_8), key)
            }
            val encoded = encodeItem(blob, plaintext.length, incomingLamportTs)
            prefs.edit().putString("item_$existingStorageId", encoded).apply()
            Log.d(TAG, "LWW replaced item_id=$itemId storageId=$existingStorageId (lamport $storedTs→$incomingLamportTs)")
            return@withContext true
        }

        // New item: generate a fresh storage id and store normally.
        val storageId = itemId // Use the stable item_id as the storage key for easy lookup.
        val blob = try {
            encryptText(storageId, plaintext.toByteArray(Charsets.UTF_8), key)
        } catch (e: IllegalStateException) {
            Log.w(TAG, "storeItemWithLww: UniFFI unavailable — using local AES-GCM fallback (NOT sync-compatible)")
            ClipboardRepository.localAesEncrypt(plaintext.toByteArray(Charsets.UTF_8), key)
        } catch (_: UnsatisfiedLinkError) {
            Log.w(TAG, "storeItemWithLww: UnsatisfiedLinkError — using local AES-GCM fallback (NOT sync-compatible)")
            ClipboardRepository.localAesEncrypt(plaintext.toByteArray(Charsets.UTF_8), key)
        }
        val encoded = encodeItem(blob, plaintext.length, incomingLamportTs)

        synchronized(idsWriteLock) {
            val ids = storedIds().toMutableList().also { it.add(storageId) }
            val editor = prefs.edit()
            val maxItems = settings.maxHistoryItems.coerceAtLeast(1)
            if (ids.size > maxItems) {
                val dropCount = ids.size - maxItems
                repeat(dropCount) {
                    val droppedId = ids.removeAt(0)
                    editor.remove("item_$droppedId")
                    editor.remove("item_id_ref_$droppedId")
                }
            }
            editor
                .putString("item_$storageId", encoded)
                .putString("item_ids", ids.joinToString(","))
                .putString("item_id_ref_$storageId", storageId)
                .apply()
        }
        Log.d(TAG, "storeItemWithLww: stored new item_id=$itemId as storageId=$storageId")
        true
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

    private fun storedIds(): List<String> =
        prefs.getString("item_ids", "")
            ?.split(",")
            ?.filter { it.isNotBlank() }
            ?: emptyList()

    /**
     * Encode a stored item as a pipe-delimited string (v2 format, 6 fields):
     * <wallTimeMs>|<contentType>|<snippetLen>|<nonceB64>|<ciphertextB64>|<lamportTs>
     *
     * The lamportTs field (index 5) was added for LWW cloud sync. Legacy rows
     * (only 5 fields) are read back with lamportTs=0, meaning they will be
     * replaced by any incoming cloud row with a positive lamport_ts.
     */
    private fun encodeItem(blob: EncryptedBlob, plaintextLen: Int, lamportTs: Long = 0L): String {
        val nonce64 = Base64.encodeToString(blob.nonce, Base64.NO_WRAP)
        val ct64 = Base64.encodeToString(blob.ciphertext, Base64.NO_WRAP)
        val ts = System.currentTimeMillis()
        return "$ts|text/plain|$plaintextLen|$nonce64|$ct64|$lamportTs"
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

    /**
     * Read the stored lamport_ts for the item at [storageId].
     * Returns 0 when the item does not exist or has no lamport_ts (legacy format).
     */
    private fun storedLamportTs(storageId: String): Long {
        val raw = prefs.getString("item_$storageId", null) ?: return 0L
        return try {
            val parts = raw.split("|")
            if (parts.size >= 6) parts[5].toLong() else 0L
        } catch (_: Exception) {
            0L
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
