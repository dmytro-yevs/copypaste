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
 * items until the total stored payload bytes are within [Settings.storageQuotaBytes].
 * There is NO count cap — only a size/byte cap (mirrors the macOS desktop policy).
 *
 * PINNED items (tracked in [KEY_PINNED_IDS]) are never evicted by the prune pass
 * and have no TTL. They survive until the user explicitly clears them via
 * [clearAll] (which deletes everything) or [deleteItem] / [deleteItems].
 *
 * ## Sensitive items
 *
 * Sensitive items are DROPPED at capture time in [storeItem] and
 * [storeItemWithLww] — Android never persists them.
 */
class ClipboardRepository(context: Context) {

    /**
     * Application context retained so the delete path can keep the
     * foreground-service notification counter honest (see [deleteItem]). Using
     * the application context avoids leaking an Activity.
     */
    private val appContext: Context = context.applicationContext

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
     *
     * The dedup state ([lastStoredHash], [lastStoredAtMs], [dedupLock]) lives in
     * the [companion object] so it is shared process-wide across every
     * repository instance. All three listener owners run in the same process and
     * each builds its own [ClipboardRepository]; per-instance state let the same
     * physical copy slip past three independent guards, producing dup×3 rows,
     * notifications and sync pushes.
     */

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
        val removed = synchronized(idsWriteLock) {
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
        // Keep the foreground-service notification's "captured today" count from
        // drifting above reality after a deletion: decrement by one (floored at
        // 0) and re-issue the notification so the shown number matches the store.
        // Only fires when an item was actually removed.
        if (removed) {
            ClipboardService.onItemsDeleted(appContext, 1)
        }
        removed
    }

    /**
     * Bulk-delete items by [ids]. Items not present in the index are silently
     * skipped. Pinned state is cleaned up for any deleted ids. Image blobs are
     * removed alongside the item entry.
     */
    fun deleteItems(ids: List<String>) {
        if (ids.isEmpty()) return
        val toDelete = ids.toSet()
        var deletedCount = 0
        synchronized(idsWriteLock) {
            val storedList = storedIds().toMutableList()
            val before = storedList.size
            storedList.removeAll(toDelete)
            deletedCount = before - storedList.size
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
        if (deletedCount > 0) {
            ClipboardService.onItemsDeleted(appContext, deletedCount)
        }
        Log.d(TAG, "deleteItems: removed $deletedCount items")
    }

    /**
     * Delete ALL items (text blobs + image blobs + synced-source-id set + pinned
     * set). This is an explicit user action — even pinned items are removed.
     */
    fun clearAll() {
        var totalDeleted = 0
        synchronized(idsWriteLock) {
            val ids = storedIds()
            totalDeleted = ids.size
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
        // Reset cross-listener dedup state so a re-copy after a full clear stores
        // a fresh row instead of being silently skipped as a duplicate.
        resetDedupState()
        if (totalDeleted > 0) {
            ClipboardService.onItemsDeleted(appContext, totalDeleted)
        }
        Log.d(TAG, "clearAll: all items deleted")
    }

    /**
     * Delete all UNPINNED items (text blobs + image blobs). Pinned items remain.
     * The synced-source-id set is also cleared (re-syncing pinned items is fine).
     */
    fun clearUnpinned() {
        var deletedCount = 0
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
            // Retain only pinned ids in the index; clear source-id seen-set.
            val remaining = ids.filter { it in pinnedSet }
            deletedCount = ids.size - remaining.size
            editor
                .putString(KEY_ITEM_IDS, remaining.joinToString(","))
                .remove(KEY_SYNCED_SOURCE_IDS)
                .apply()
        }
        if (deletedCount > 0) {
            ClipboardService.onItemsDeleted(appContext, deletedCount)
        }
        Log.d(TAG, "clearUnpinned: all unpinned items deleted")
    }

    /**
     * Pin or unpin item [id].
     * Pinned items survive the retention prune pass and have no sensitive TTL.
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
     * oversized text, sensitive content, a recent local duplicate, or — for synced
     * items — already stored under the same [sourceId]).
     *
     * After inserting, calls [pruneToLimits] to enforce the storage-quota cap
     * (SIZE only — no count cap).
     */
    suspend fun storeItem(
        plaintext: String,
        key: ByteArray,
        sourceId: String? = null,
        overrideId: String? = null,
        contentType: String = "text/plain",
        lamportTs: Long = 0L,
    ): String = withContext(Dispatchers.IO) {
        if (plaintext.isBlank()) return@withContext ""

        // ── Size enforcement: reject oversized text before any crypto work.
        val textBytes = plaintext.toByteArray(Charsets.UTF_8)
        val maxTextBytes = settings.maxTextSizeBytes
        if (textBytes.size.toLong() > maxTextBytes) {
            Log.w(TAG, "storeItem: text ${textBytes.size} B exceeds maxTextSizeBytes $maxTextBytes — dropping")
            return@withContext ""
        }

        // The id that dedup keys on: an explicit [sourceId] wins; otherwise the
        // incoming [overrideId] (which IS the stable remote id) is the source id.
        val dedupSourceId = sourceId ?: overrideId

        // ── LOW-2: source-id dedup for incoming synced items.
        if (dedupSourceId != null) {
            synchronized(seenSourceIdsLock) {
                val seen = storedSourceIds()
                if (!isNewSourceId(dedupSourceId, seen)) {
                    Log.d(TAG, "Synced item $dedupSourceId already stored — skipping")
                    return@withContext ""
                }
                recordSourceId(dedupSourceId, seen)
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

        // STABLE identity: reuse an incoming item's stable id verbatim; mint a
        // fresh UUID only for a locally-captured clip. This is the value bound
        // into the AEAD AAD and reused on every later push/sync.
        val id = overrideId?.takeIf { it.isNotBlank() } ?: UUID.randomUUID().toString()
        val blob = try {
            encryptText(id, textBytes, key)
        } catch (e: IllegalStateException) {
            Log.w(TAG, "UniFFI unavailable (${e.message}) — using local AES-GCM fallback (NOT sync-compatible)")
            localAesEncrypt(textBytes, key)
        } catch (_: UnsatisfiedLinkError) {
            Log.w(TAG, "UniFFI unavailable (UnsatisfiedLinkError) — using local AES-GCM fallback (NOT sync-compatible)")
            localAesEncrypt(textBytes, key)
        }

        val encoded = encodeItem(blob, textBytes.size, contentType = contentType, lamportTs = lamportTs)
        synchronized(idsWriteLock) {
            val ids = storedIds().toMutableList().also { it.add(id) }
            prefs.edit()
                .putString("item_$id", encoded)
                .putString(KEY_ITEM_IDS, ids.joinToString(","))
                // Reverse-lookup: item_id → storage_id for LWW cloud sync.
                // For locally-captured items the storage id IS the item_id.
                .putString("item_id_ref_$id", id)
                .apply()
        }

        Log.d(TAG, "Stored item $id (${textBytes.size} bytes, contentType=$contentType)")

        // Prune to size-only quota after insert.
        pruneToLimits()
        id
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

        // ── REPLACE PATH: close the TOCTOU between the existingStorageId
        // lookup + storedLamportTs read and the final putString write.
        //
        // Previously the lookup and the lamport comparison happened OUTSIDE
        // idsWriteLock, so a concurrent deleteItem (which holds idsWriteLock
        // while it removes "item_<id>" and rewrites the index) could delete
        // the row between our read and our locked write, resurrecting a ghost
        // blob under a storage key that no longer appears in the index.
        //
        // Fix: encrypt into a local variable FIRST (encryption is expensive and
        // has no shared state — doing it inside the lock would increase
        // contention unnecessarily), then enter idsWriteLock for the entire
        // read-decide-write sequence: lookup → lamport compare → putString.
        // There is no re-entrant idsWriteLock acquisition inside the block
        // (no call to deleteItem / storedIds / storeItem), so no deadlock.

        val plaintextBytes = plaintext.toByteArray(Charsets.UTF_8)

        val replaced = synchronized(idsWriteLock) {
            val existingStorageId = prefs.getString("item_id_ref_$itemId", null)
                ?: return@synchronized false  // not yet stored → fall through to new-item path

            // LWW: only replace when incoming lamport_ts is strictly newer.
            val storedTs = storedLamportTs(existingStorageId)
            if (incomingLamportTs <= storedTs) {
                Log.d(TAG, "LWW: skipping dup item_id=$itemId (stored=$storedTs, incoming=$incomingLamportTs)")
                return@synchronized null  // null = "skip, do not store as new item either"
            }

            // Replace in-place: re-encrypt and overwrite the stored blob.
            val blob = try {
                encryptText(existingStorageId, plaintextBytes, key)
            } catch (e: IllegalStateException) {
                Log.w(TAG, "LWW replace: UniFFI unavailable — using local AES-GCM fallback (NOT sync-compatible)")
                localAesEncrypt(plaintextBytes, key)
            } catch (_: UnsatisfiedLinkError) {
                Log.w(TAG, "LWW replace: UnsatisfiedLinkError — using local AES-GCM fallback (NOT sync-compatible)")
                localAesEncrypt(plaintextBytes, key)
            }
            val encoded = encodeItem(blob, plaintextBytes.size, lamportTs = incomingLamportTs)
            prefs.edit().putString("item_$existingStorageId", encoded).apply()
            Log.d(TAG, "LWW replaced item_id=$itemId storageId=$existingStorageId (lamport $storedTs→$incomingLamportTs)")
            true  // replaced successfully
        }

        // null  → duplicate (older/equal lamport), skip
        // true  → replaced, return immediately
        // false → item not found, fall through to new-item insert below
        if (replaced != false) return@withContext replaced == true

        // New item: generate a fresh storage id and store normally.
        val storageId = itemId // Use the stable item_id as the storage key for easy lookup.
        val blob = try {
            encryptText(storageId, plaintextBytes, key)
        } catch (e: IllegalStateException) {
            Log.w(TAG, "storeItemWithLww: UniFFI unavailable — using local AES-GCM fallback (NOT sync-compatible)")
            localAesEncrypt(plaintextBytes, key)
        } catch (_: UnsatisfiedLinkError) {
            Log.w(TAG, "storeItemWithLww: UnsatisfiedLinkError — using local AES-GCM fallback (NOT sync-compatible)")
            localAesEncrypt(plaintextBytes, key)
        }
        val encoded = encodeItem(blob, plaintextBytes.size, lamportTs = incomingLamportTs)

        synchronized(idsWriteLock) {
            val ids = storedIds().toMutableList().also { it.add(storageId) }
            prefs.edit()
                .putString("item_$storageId", encoded)
                .putString(KEY_ITEM_IDS, ids.joinToString(","))
                .putString("item_id_ref_$storageId", storageId)
                .apply()
        }
        Log.d(TAG, "storeItemWithLww: stored new item_id=$itemId as storageId=$storageId")
        pruneToLimits()
        true
    }

    /**
     * Return the id of the most recently stored item, or null when the index is
     * empty. Used by image capture callers that need the id that [storeItem] just
     * wrote so they can call [storeImageBytes] under the same key.
     *
     * Safe to call immediately after [storeItem] returns true because storeItem
     * appends the new id at the END of the comma-joined index before returning.
     * The caller runs on [Dispatchers.IO] and storeItem holds [idsWriteLock] for
     * the entire append, so by the time storeItem returns the id is visible here.
     */
    fun lastStoredId(): String? = storedIds().lastOrNull()

    /**
     * Decrypt and return the FULL plaintext for item [id], or null when the item
     * does not exist or cannot be decrypted.
     *
     * Used by the copy-to-clipboard path in [HistoryActivity] to ensure the user
     * copies the complete original text, not the 140-char [ClipboardItem.snippet].
     */
    suspend fun loadFullPlaintext(id: String, key: ByteArray): String? =
        withContext(Dispatchers.IO) {
            val raw = prefs.getString("item_$id", null) ?: return@withContext null
            val parts = raw.split("|")
            val nonceB64 = parts.getOrNull(3) ?: return@withContext null
            val ctB64 = parts.getOrNull(4) ?: return@withContext null
            return@withContext try {
                val nonce = Base64.decode(nonceB64, Base64.NO_WRAP)
                val ciphertext = Base64.decode(ctB64, Base64.NO_WRAP)
                decryptForPreview(id, ciphertext, nonce, key)
            } catch (e: Exception) {
                Log.w(TAG, "loadFullPlaintext: decrypt failed for $id: ${e.message}")
                null
            }
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
        var evictedCount = 0

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
                evictedCount++
                Log.d(TAG, "pruneToLimits: evicted $evictId (blob ${sz}B, totalNow=${totalBytes}B)")
            }

            if (didEvict) {
                editor.putString(KEY_ITEM_IDS, ids.joinToString(",")).apply()
            }
        }

        if (evictedCount > 0) {
            ClipboardService.onItemsDeleted(appContext, evictedCount)
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
     * Encode a stored item as a pipe-delimited string (v2 format, 6 fields):
     * <wallTimeMs>|<contentType>|<payloadBytes>|<nonceB64>|<ciphertextB64>|<lamportTs>
     *
     * The lamportTs field (index 5) was added for LWW cloud sync. Legacy rows
     * (only 5 fields) are read back with lamportTs=0.
     */
    private fun encodeItem(
        blob: EncryptedBlob,
        plaintextLen: Int,
        contentType: String = "text/plain",
        lamportTs: Long = 0L,
    ): String {
        val nonce64 = Base64.encodeToString(blob.nonce, Base64.NO_WRAP)
        val ct64 = Base64.encodeToString(blob.ciphertext, Base64.NO_WRAP)
        val ts = System.currentTimeMillis()
        return "$ts|$contentType|$plaintextLen|$nonce64|$ct64|$lamportTs"
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

        fun normalizeContentTypeForSync(stored: String): String =
            if (stored == "text" || stored.startsWith("text/")) "text" else stored

        const val KEY_ITEM_IDS = "item_ids"
        const val KEY_SYNCED_SOURCE_IDS = "synced_source_ids"
        const val KEY_PINNED_IDS = "pinned_ids"

        const val MAX_SEEN_SOURCE_IDS = 1_000

        private const val DEDUP_WINDOW_MS = 2_000L

        /**
         * Process-wide dedup state shared across all [ClipboardRepository] instances.
         * Multiple listener owners (FGS, a11y service, activity) each build their own
         * instance; per-instance state lets the same physical copy slip past all three
         * guards independently. All accesses must be under [dedupLock].
         */
        @Volatile var lastStoredHash: Int = 0
        @Volatile var lastStoredAtMs: Long = 0L
        val dedupLock = Any()

        /**
         * "Expected next clip" guard for copy-from-history (HIGH-3 follow-up).
         *
         * When the user taps a row in [HistoryActivity] to copy it, the UI calls
         * setPrimaryClip with that text. The capture listeners
         * ([ClipboardService] / [ClipboardAccessibilityService]) then observe the
         * SAME text as a fresh clipboard change and would re-capture it as a NEW
         * row (outside the [DEDUP_WINDOW_MS] window when the original was copied
         * long ago) — producing a duplicate row AND a redundant cloud re-push.
         *
         * [HistoryActivity] calls [expectClip] with the content-hash right BEFORE
         * setPrimaryClip; [shouldSkipExpectedClip] consumes that expectation in
         * the capture path and skips the re-capture exactly once. The expectation
         * is single-shot ([expectedClipHash] is cleared on the first match) and
         * also expires after [EXPECTED_CLIP_WINDOW_MS] so a stale expectation
         * never silently drops a genuinely new copy of the same text.
         *
         * Process-wide ([companion object]) for the same reason as the dedup
         * state: the UI activity sets it but the capture listeners (separate
         * [ClipboardRepository] instances in the same process) consume it.
         */
        @Volatile var expectedClipHash: Int = 0
        @Volatile var expectedClipHasValue: Boolean = false
        @Volatile var expectedClipAtMs: Long = 0L
        val expectedClipLock = Any()

        private const val EXPECTED_CLIP_WINDOW_MS = 5_000L

        /**
         * Record that the next observed clipboard change carrying text whose
         * hash equals [content].hashCode() is an internal copy-from-history echo
         * and must NOT be re-captured. Call immediately before setPrimaryClip.
         */
        fun expectClip(content: String) {
            synchronized(expectedClipLock) {
                expectedClipHash = content.hashCode()
                expectedClipHasValue = true
                expectedClipAtMs = System.currentTimeMillis()
            }
        }

        /**
         * Returns true (and consumes the expectation) when [content] matches a
         * pending [expectClip] within [EXPECTED_CLIP_WINDOW_MS]. Single-shot: the
         * expectation is cleared on the first match (or once expired) so only the
         * immediate echo is suppressed, never a later genuine re-copy.
         */
        fun shouldSkipExpectedClip(content: String): Boolean {
            synchronized(expectedClipLock) {
                if (!expectedClipHasValue) return false
                val now = System.currentTimeMillis()
                if (now - expectedClipAtMs > EXPECTED_CLIP_WINDOW_MS) {
                    expectedClipHasValue = false
                    return false
                }
                if (content.hashCode() == expectedClipHash) {
                    expectedClipHasValue = false  // consume — single-shot
                    return true
                }
                return false
            }
        }

        /**
         * Zero the cross-listener dedup window. Call after [clearAll] so a re-copy
         * of the same text immediately after a clear is stored as a fresh row rather
         * than silently skipped as a recent duplicate.
         */
        fun resetDedupState() {
            synchronized(dedupLock) {
                lastStoredHash = 0
                lastStoredAtMs = 0L
            }
            synchronized(expectedClipLock) {
                expectedClipHasValue = false
            }
        }

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
                    // STABLE cross-device identity. The row id is minted ONCE at
                    // capture (or carried from an incoming item) and persisted,
                    // so reusing it as item_id lets the daemon dedup/LWW-merge
                    // this clip instead of seeing a fresh item on every dial.
                    itemId = id,
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
     * DEAD CODE — relay incoming sync is disabled.
     * Use [FgsSyncLoop.poll] (via [SyncManager.pollFromSupabase]) for incoming cloud sync.
     * @throws UnsupportedOperationException always — to surface accidental callers.
     */
    @Deprecated(
        message = "Relay incoming sync is disabled: items were encrypted with the sender's " +
            "local per-device key that no other device holds, making every fetched payload " +
            "undecryptable. Use FgsSyncLoop (Supabase poll) for incoming cloud sync.",
        replaceWith = ReplaceWith("syncManager.pollFromSupabase()"),
        level = DeprecationLevel.ERROR,
    )
    @Suppress("UnusedParameter") // params kept for binary-compat; function is intentionally dead
    suspend fun syncItems(syncManager: SyncManager, encryptionKey: ByteArray): List<String> {
        throw UnsupportedOperationException(
            "relay cloud backend is disabled — use Supabase for cross-device cloud sync"
        )
    }
}
