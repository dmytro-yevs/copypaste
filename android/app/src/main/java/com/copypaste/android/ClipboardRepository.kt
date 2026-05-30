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
 * Each item is stored as a pipe-delimited blob under key "item_<uuid>" so it
 * survives process death without requiring Room or a .so binary.
 * An ordered index of ids is kept under "item_ids" (comma-separated).
 *
 * Encryption is attempted via UniFFI [encryptText]; on [UnsatisfiedLinkError]
 * (e.g. during unit tests or before .so is built) it falls back to
 * [localAesEncrypt] which uses AES-256-GCM via the Android KeyStore provider.
 *
 * ## Retention & quota enforcement
 *
 * After every insert the prune pass ([pruneToLimits]) evicts the oldest UNPINNED
 * items until the total stored payload bytes are within [Settings.storageQuotaBytes].
 * There is NO count cap — only a size/byte cap (mirrors the macOS desktop policy).
 *
 * PINNED items (tracked in [KEY_PINNED_IDS]) are never evicted by the prune pass
 * and have no TTL. They survive until the user explicitly clears them via
 * [clearAll] or [deleteItem] / [deleteItems].
 *
 * ## Sensitive items
 *
 * Sensitive items are DROPPED at capture time in [storeItem] — Android never
 * persists them.
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

    /**
     * Guard for read-modify-write on the comma-joined "synced_source_ids" set
     * (LOW-2). Both Supabase poll callers can run concurrently (FGS loop +
     * WorkManager worker), so the seen-set must be mutated under a lock to avoid
     * a lost update that would let a duplicate row through.
     */
    private val seenSourceIdsLock = Any()

    /**
     * Subscribe to changes in the backing item store. Any write from the
     * foreground service, the accessibility service, or another in-process
     * writer mutates the shared "copypaste_items" prefs and fires [listener].
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

    /**
     * Load history items for display, most-recent-first.
     *
     * Each stored blob is DECRYPTED with [key] so the row shows a real preview.
     * The [ClipboardItem.pinned] field is populated from the persisted [KEY_PINNED_IDS] set.
     * Image bytes are attached when available (stored separately under "item_img_<id>").
     */
    suspend fun getItems(key: ByteArray, limit: Int = 200): List<ClipboardItem> =
        withContext(Dispatchers.IO) {
            val pinnedSet = storedPinnedIds()
            val ids = storedIds().takeLast(limit)
            ids.mapNotNull { id ->
                val raw = prefs.getString("item_$id", null) ?: return@mapNotNull null
                val item = parseItem(id, raw, key) ?: return@mapNotNull null
                // Attach image bytes when available — non-null only for image/* content types.
                val withImage = if (item.isImage) item.copy(imagePng = getImageBytes(id)) else item
                withImage.copy(pinned = id in pinnedSet)
            }.reversed()
        }

    /**
     * Return the raw PNG/JPEG bytes stored for image item [id], or null.
     * Image bytes are persisted under the key "item_img_<id>" as Base64 NO_WRAP.
     */
    fun getImageBytes(id: String): ByteArray? {
        val b64 = prefs.getString("item_img_$id", null) ?: return null
        return try {
            Base64.decode(b64, Base64.NO_WRAP)
        } catch (e: Exception) {
            Log.w(TAG, "getImageBytes: failed to decode image for $id: ${e.message}")
            null
        }
    }

    /**
     * Persist raw image bytes for item [id].
     * Rejects images larger than [Settings.maxImageSizeBytes].
     */
    fun storeImageBytes(id: String, bytes: ByteArray) {
        val maxBytes = settings.maxImageSizeBytes
        if (bytes.size.toLong() > maxBytes) {
            Log.w(TAG, "storeImageBytes: image ${bytes.size} B exceeds maxImageSizeBytes $maxBytes — dropping")
            return
        }
        val b64 = Base64.encodeToString(bytes, Base64.NO_WRAP)
        prefs.edit().putString("item_img_$id", b64).apply()
        Log.d(TAG, "storeImageBytes: stored ${bytes.size} bytes for $id")
    }

    suspend fun deleteItem(id: String): Boolean = withContext(Dispatchers.IO) {
        synchronized(idsWriteLock) {
            val ids = storedIds().toMutableList()
            if (!ids.remove(id)) return@synchronized false
            val pinnedSet = storedPinnedIds().toMutableSet()
            val wasPinned = pinnedSet.remove(id)
            val editor = prefs.edit()
                .remove("item_$id")
                .remove("item_img_$id")
                .putString(KEY_ITEM_IDS, ids.joinToString(","))
            if (wasPinned) {
                editor.putString(KEY_PINNED_IDS, pinnedSet.joinToString(","))
            }
            editor.apply()
            true
        }
    }

    /**
     * Bulk-delete items by [ids]. Items not present in the index are silently
     * skipped. Pinned state is cleaned up for any deleted ids.
     */
    fun deleteItems(ids: List<String>) {
        if (ids.isEmpty()) return
        val toDelete = ids.toSet()
        synchronized(idsWriteLock) {
            val storedList = storedIds().toMutableList()
            storedList.removeAll(toDelete)
            val pinnedSet = storedPinnedIds().toMutableSet()
            val pinnedChanged = pinnedSet.removeAll(toDelete)
            val editor = prefs.edit()
                .putString(KEY_ITEM_IDS, storedList.joinToString(","))
            for (id in toDelete) {
                editor.remove("item_$id")
                editor.remove("item_img_$id")
            }
            if (pinnedChanged) {
                editor.putString(KEY_PINNED_IDS, pinnedSet.joinToString(","))
            }
            editor.apply()
        }
        Log.d(TAG, "deleteItems: removed ${toDelete.size} items")
    }

    /**
     * Delete ALL items (text + image blobs + synced-source-id set + pinned set).
     * Called from "Clear All" action.
     */
    fun clearAll() {
        synchronized(idsWriteLock) {
            val ids = storedIds()
            val editor = prefs.edit()
            for (id in ids) {
                editor.remove("item_$id")
                editor.remove("item_img_$id")
            }
            editor
                .remove(KEY_ITEM_IDS)
                .remove(KEY_SYNCED_SOURCE_IDS)
                .remove(KEY_PINNED_IDS)
                .apply()
        }
        Log.d(TAG, "clearAll: all items deleted")
    }

    /**
     * Delete all UNPINNED items (text + image blobs). Pinned items remain.
     * Called from "Clear Unpinned" action.
     */
    fun clearUnpinned() {
        synchronized(idsWriteLock) {
            val pinnedSet = storedPinnedIds()
            val ids = storedIds()
            val editor = prefs.edit()
            for (id in ids) {
                if (id !in pinnedSet) {
                    editor.remove("item_$id")
                    editor.remove("item_img_$id")
                }
            }
            val remaining = ids.filter { it in pinnedSet }
            editor
                .putString(KEY_ITEM_IDS, remaining.joinToString(","))
                .remove(KEY_SYNCED_SOURCE_IDS)
                .apply()
        }
        Log.d(TAG, "clearUnpinned: all unpinned items deleted")
    }

    /**
     * Pin or unpin item [id].
     * Pinned items survive the retention prune pass.
     */
    fun setPinned(id: String, pinned: Boolean) {
        synchronized(idsWriteLock) {
            val pinnedSet = storedPinnedIds().toMutableSet()
            val changed = if (pinned) pinnedSet.add(id) else pinnedSet.remove(id)
            if (changed) {
                prefs.edit().putString(KEY_PINNED_IDS, pinnedSet.joinToString(",")).apply()
            }
        }
        Log.d(TAG, "setPinned: item $id pinned=$pinned")
    }

    /**
     * Encrypt [plaintext] with [key] and persist, returning the STABLE row id of
     * the stored item — or an empty string when nothing was stored (blank text,
     * oversized text, sensitive content, or a recent local duplicate).
     *
     * [contentType] defaults to "text/plain". Pass the actual MIME type for image
     * items (e.g. "image/png") so history can distinguish and attach image bytes.
     *
     * After inserting, calls [pruneToLimits] to enforce the storage-quota cap
     * (SIZE only — no count cap).
     */
    suspend fun storeItem(
        plaintext: String,
        key: ByteArray,
        sourceId: String? = null,
        contentType: String = "text/plain",
    ): String = withContext(Dispatchers.IO) {
        if (plaintext.isBlank()) return@withContext ""

        // ── Size enforcement: reject oversized text before any crypto work.
        val textBytes = plaintext.toByteArray(Charsets.UTF_8)
        val maxTextBytes = settings.maxTextSizeBytes
        if (textBytes.size.toLong() > maxTextBytes) {
            Log.w(TAG, "storeItem: text ${textBytes.size} B exceeds maxTextSizeBytes $maxTextBytes — dropping")
            return@withContext ""
        }

        // ── LOW-2: source-id dedup for incoming synced items.
        if (sourceId != null) {
            synchronized(seenSourceIdsLock) {
                val seen = storedSourceIds()
                if (!isNewSourceId(sourceId, seen)) {
                    Log.d(TAG, "Synced item $sourceId already stored — skipping")
                    return@withContext ""
                }
                recordSourceId(sourceId, seen)
            }
        }

        // ── HIGH-3: cross-listener dedup (identical content within DEDUP_WINDOW_MS).
        val hash = plaintext.hashCode()
        synchronized(dedupLock) {
            val now = System.currentTimeMillis()
            if (hash == lastStoredHash && now - lastStoredAtMs < DEDUP_WINDOW_MS) {
                Log.d(TAG, "Duplicate clip within ${DEDUP_WINDOW_MS}ms — skipping")
                return@withContext ""
            }
            lastStoredHash = hash
            lastStoredAtMs = now
        }

        val sensitive = try {
            isSensitive(plaintext)
        } catch (_: UnsatisfiedLinkError) {
            false
        }
        if (sensitive) return@withContext ""

        val id = UUID.randomUUID().toString()
        val blob = try {
            encryptText(id, textBytes, key)
        } catch (e: IllegalStateException) {
            Log.d(TAG, "UniFFI unavailable (${e.message}) — using local AES-GCM fallback")
            localAesEncrypt(textBytes, key)
        } catch (_: UnsatisfiedLinkError) {
            Log.d(TAG, "UniFFI unavailable (UnsatisfiedLinkError) — using local AES-GCM fallback")
            localAesEncrypt(textBytes, key)
        }

        val encoded = encodeItem(blob, textBytes.size, contentType = contentType)
        synchronized(idsWriteLock) {
            val ids = storedIds().toMutableList().also { it.add(id) }
            prefs.edit()
                .putString("item_$id", encoded)
                .putString(KEY_ITEM_IDS, ids.joinToString(","))
                .apply()
        }

        Log.d(TAG, "Stored item $id (${textBytes.size} bytes, contentType=$contentType)")

        // Prune to size-only quota after insert (count cap REMOVED — macOS parity).
        pruneToLimits()
        id
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

    /**
     * Enforce the storage-quota cap by evicting the oldest UNPINNED items.
     *
     * Only the byte quota is enforced — there is no count cap (mirrors desktop policy).
     * "Stored payload bytes" approximates the UTF-8 byte length of each blob string
     * (text) plus the stored Base64 length for image bytes.
     *
     * PINNED items are counted in total bytes but never evicted.
     */
    private fun pruneToLimits() {
        val quotaBytes = settings.storageQuotaBytes.coerceAtLeast(0L)

        synchronized(idsWriteLock) {
            val pinnedSet = storedPinnedIds()
            val ids = storedIds().toMutableList()

            val blobSizes: Map<String, Int> = ids.associate { id ->
                val textBytes = prefs.getString("item_$id", null)?.toByteArray(Charsets.UTF_8)?.size ?: 0
                val imgBytes = prefs.getString("item_img_$id", null)?.length ?: 0
                id to (textBytes + imgBytes)
            }

            var totalBytes = blobSizes.values.sumOf { it.toLong() }
            val unpinned = ids.filter { it !in pinnedSet }.toMutableList()

            val editor = prefs.edit()
            var didEvict = false

            while (unpinned.isNotEmpty()) {
                val quotaExceeded = quotaBytes > 0 && totalBytes > quotaBytes
                if (!quotaExceeded) break

                val evictId = unpinned.removeAt(0)
                ids.remove(evictId)
                val sz = blobSizes[evictId] ?: 0
                totalBytes -= sz
                editor.remove("item_$evictId")
                editor.remove("item_img_$evictId")
                didEvict = true
                Log.d(TAG, "pruneToLimits: evicted $evictId (blob ${sz}B, totalNow=${totalBytes}B)")
            }

            if (didEvict) {
                editor.putString(KEY_ITEM_IDS, ids.joinToString(",")).apply()
            }
        }
    }

    private fun storedIds(): List<String> =
        prefs.getString(KEY_ITEM_IDS, "")
            ?.split(",")
            ?.filter { it.isNotBlank() }
            ?: emptyList()

    private fun storedPinnedIds(): Set<String> =
        prefs.getString(KEY_PINNED_IDS, "")
            ?.split(",")
            ?.filter { it.isNotBlank() }
            ?.toHashSet()
            ?: emptySet()

    private fun storedSourceIds(): LinkedHashSet<String> =
        LinkedHashSet(
            prefs.getString(KEY_SYNCED_SOURCE_IDS, "")
                ?.split(",")
                ?.filter { it.isNotBlank() }
                ?: emptyList()
        )

    private fun recordSourceId(sourceId: String, seen: LinkedHashSet<String>) {
        seen.add(sourceId)
        while (seen.size > MAX_SEEN_SOURCE_IDS) {
            val oldest = seen.iterator().next()
            seen.remove(oldest)
        }
        prefs.edit().putString(KEY_SYNCED_SOURCE_IDS, seen.joinToString(",")).apply()
    }

    /**
     * Encode a stored item as a pipe-delimited string:
     * <wallTimeMs>|<contentType>|<payloadBytes>|<nonceB64>|<ciphertextB64>
     */
    private fun encodeItem(
        blob: EncryptedBlob,
        plaintextLen: Int,
        contentType: String = "text/plain",
    ): String {
        val nonce64 = Base64.encodeToString(blob.nonce, Base64.NO_WRAP)
        val ct64 = Base64.encodeToString(blob.ciphertext, Base64.NO_WRAP)
        val ts = System.currentTimeMillis()
        return "$ts|$contentType|$plaintextLen|$nonce64|$ct64"
    }

    private fun parseItem(id: String, raw: String, key: ByteArray): ClipboardItem? {
        val parts = try {
            raw.split("|")
        } catch (e: Exception) {
            Log.w(TAG, "Failed to parse item $id: ${e.message}")
            return null
        }
        val wallTimeMs = parts.getOrNull(0)?.toLongOrNull() ?: return null
        val contentType = parts.getOrNull(1) ?: return null
        val nonceB64 = parts.getOrNull(3)
        val ctB64 = parts.getOrNull(4)

        val plaintext: String? = if (nonceB64 != null && ctB64 != null) {
            try {
                val nonce = Base64.decode(nonceB64, Base64.NO_WRAP)
                val ciphertext = Base64.decode(ctB64, Base64.NO_WRAP)
                decryptForPreview(id, ciphertext, nonce, key)
            } catch (e: Exception) {
                Log.d(TAG, "Preview decrypt failed for $id: ${e.message}")
                null
            }
        } else {
            null
        }

        val sensitive = plaintext != null && try {
            isSensitive(plaintext)
        } catch (_: UnsatisfiedLinkError) {
            false
        }

        val snippet = if (plaintext == null) UNABLE_TO_PREVIEW else previewFromPlaintext(plaintext)

        // pinned and imagePng are populated by getItems() after parseItem returns.
        return ClipboardItem(
            id = id,
            contentType = contentType,
            isSensitive = sensitive,
            wallTimeMs = wallTimeMs,
            snippet = snippet,
        )
    }

    private fun decryptForPreview(
        id: String,
        ciphertext: ByteArray,
        nonce: ByteArray,
        key: ByteArray,
    ): String {
        val bytes = try {
            decryptText(id, ciphertext, nonce, key)
        } catch (_: Exception) {
            localAesDecrypt(ciphertext, nonce, key)
        }
        return String(bytes, Charsets.UTF_8)
    }

    companion object {
        private const val TAG = "ClipboardRepository"

        fun normalizeContentTypeForSync(stored: String): String =
            if (stored == "text" || stored.startsWith("text/")) "text" else stored

        const val KEY_ITEM_IDS = "item_ids"
        const val KEY_SYNCED_SOURCE_IDS = "synced_source_ids"
        const val KEY_PINNED_IDS = "pinned_ids"

        const val MAX_SEEN_SOURCE_IDS = 1_000

        private const val DEDUP_WINDOW_MS = 2_000L

        fun isNewSourceId(sourceId: String, seen: Set<String>): Boolean =
            sourceId !in seen

        const val PREVIEW_MAX_CHARS = 140
        const val UNABLE_TO_PREVIEW = "(unable to preview)"

        private const val AES_TRANSFORMATION = "AES/GCM/NoPadding"
        private const val GCM_TAG_BITS = 128
        private const val GCM_NONCE_BYTES = 12

        fun previewFromPlaintext(text: String): String {
            val collapsed = text.replace(Regex("\\s+"), " ").trim()
            if (collapsed.isEmpty()) return ""
            return if (collapsed.length > PREVIEW_MAX_CHARS) {
                collapsed.take(PREVIEW_MAX_CHARS).trimEnd() + "…"
            } else {
                collapsed
            }
        }

        fun localAesDecrypt(ciphertext: ByteArray, nonce: ByteArray, key: ByteArray): ByteArray {
            val cipher = Cipher.getInstance(AES_TRANSFORMATION)
            cipher.init(
                Cipher.DECRYPT_MODE,
                SecretKeySpec(key.copyOf(32), "AES"),
                GCMParameterSpec(GCM_TAG_BITS, nonce)
            )
            return cipher.doFinal(ciphertext)
        }

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
     * values for a P2P sync push.
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
                val contentType = normalizeContentTypeForSync(parts[1])
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
     * Pull incoming relay items and store locally.
     */
    suspend fun syncItems(syncManager: SyncManager, encryptionKey: ByteArray): List<String> =
        withContext(Dispatchers.IO) {
            val decrypted = syncManager.syncIncoming(encryptionKey)
            decrypted.forEach { plaintext -> storeItem(plaintext, encryptionKey) }
            decrypted
        }
}
