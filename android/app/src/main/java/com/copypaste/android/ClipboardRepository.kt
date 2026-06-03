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
 * Sensitive items are STORED (not dropped) at capture time in [storeItem] and
 * on sync-in in [storeItemWithLww], matching the macOS daemon. The sensitivity
 * verdict is recomputed at read time by [parseItem] and surfaced via
 * [ClipboardItem.isSensitive], which drives the masked preview / PRIVATE chip in
 * the history UI. Sensitive items are still subject to the sensitive-TTL
 * auto-wipe pass in [pruneByAge].
 */
class ClipboardRepository(context: Context) {

    /**
     * Application context retained so the delete path can keep the
     * foreground-service notification counter honest (see [deleteItem]). Using
     * the application context avoids leaking an Activity.
     */
    private val appContext: Context = context.applicationContext

    private val prefs: SharedPreferences =
        context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)

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
     * (ClipboardService, LogcatCaptureService, MainActivity) each fire
     * on the same copy, so without this guard one copy creates 2-3 duplicate
     * rows (HIGH-3). We skip a store when an identical-content item was stored
     * within [DEDUP_WINDOW_MS]. The time window preserves the legitimate
     * "same text copied again later" case — re-copying after the window stores
     * a fresh row as expected.
     *
     * The dedup state ([lastStoredKey], [lastStoredAtMs], [dedupLock]) lives in
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
            // AB-13: run the retention TTL auto-wipe on the same cadence as load
            // (cheap general-age fast-path; sensitive pass only decrypts aged rows).
            pruneByAge(key)
            val pinnedList = storedPinnedList()
            val pinnedSet = pinnedList.toHashSet()
            // Build index map: id → position in pinned list (0 = top of pinned section).
            val pinnedIndex: Map<String, Int> = pinnedList.mapIndexed { idx, id -> id to idx }.toMap()
            val ids = storedIds().takeLast(limit)
            ids.mapNotNull { id ->
                val raw = prefs.getString("item_$id", null) ?: return@mapNotNull null
                // Soft-delete tombstone: skip deleted items in the visible list
                // (cheap last-field check, before any AEAD decrypt).
                if (isDeletedBlob(raw)) return@mapNotNull null
                // A: serve from parse cache when the raw blob is unchanged — avoids a
                // full AEAD decrypt + native isSensitive() for every row on every reload.
                // Only decrypt when the blob has actually been written since last load.
                val item = synchronized(parseCacheLock) {
                    val entry = parseCache[id]
                    if (entry != null && entry.rawBlob == raw) entry.item else null
                } ?: run {
                    val parsed = parseItem(id, raw, key) ?: return@mapNotNull null
                    synchronized(parseCacheLock) {
                        parseCache[id] = ParsedEntry(raw, parsed)
                    }
                    parsed
                }
                // AB-8: image bytes are fetched lazily per-row via the two-level LRU
                // in HistoryActivity (cachedThumbnailBitmap). Never eager here.
                val isPinned = id in pinnedSet
                // For binary payloads the synced blob is the full-res image / raw file, NOT the
                // thumbnail shown in the row. Measure its stored byte size cheaply from the
                // Base64 string length (no decode) against the same 8 MiB ceiling sync enforces.
                // Text items keep the plaintextLen-derived flag set in parseItem().
                val binaryTooLarge = when {
                    item.isImage ->
                        (prefs.getString("item_img_$id", null)?.let { base64RawByteSize(it).toLong() } ?: 0L) > SYNC_MAX_BLOB_BYTES
                    item.isFile ->
                        (prefs.getString("item_file_$id", null)?.let { base64RawByteSize(it).toLong() } ?: 0L) > SYNC_MAX_BLOB_BYTES
                    else -> item.tooLargeToSync
                }
                item.copy(
                    pinned = isPinned,
                    pinnedSortIndex = if (isPinned) (pinnedIndex[id] ?: Int.MAX_VALUE) else -1,
                    tooLargeToSync = binaryTooLarge,
                )
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
     * Return the thumbnail bytes for image item [id], or null when no thumbnail
     * has been generated yet. Thumbnail bytes are stored under "item_thumb_<id>"
     * as Base64 NO_WRAP (WebP LOSSY on API 30+, PNG on older APIs).
     */
    fun getThumbnailBytes(id: String): ByteArray? {
        val b64 = prefs.getString("item_thumb_$id", null) ?: return null
        return try {
            Base64.decode(b64, Base64.NO_WRAP)
        } catch (e: Exception) {
            Log.w(TAG, "getThumbnailBytes: failed to decode thumb for $id: ${e.message}")
            null
        }
    }

    /**
     * AB-8: bytes a history ROW should render for image item [id]. Prefers the
     * stored thumbnail (small, generated at capture from a max-680-px Bitmap) and
     * falls back to full-res only when no thumbnail exists yet (lazy backfill for
     * items captured before thumbnail support). Called per-row on demand by
     * [HistoryActivity] through its bounded LRU — never eagerly in [getItems].
     */
    fun getDisplayImageBytes(id: String): ByteArray? =
        getThumbnailBytes(id) ?: getImageBytes(id)

    /**
     * Persist thumbnail bytes for item [id] under "item_thumb_<id>".
     *
     * No size gate is applied here — thumbnails are intentionally small (generated
     * from a max-680-px scaled Bitmap) so the quota overhead is negligible. The
     * caller ([ClipboardService.captureImageClip]) is responsible for only passing
     * the output of [ImageThumbnailUtils.generateThumbnail].
     */
    fun storeThumbnailBytes(id: String, bytes: ByteArray) {
        val b64 = Base64.encodeToString(bytes, Base64.NO_WRAP)
        prefs.edit().putString("item_thumb_$id", b64).apply()
        Log.d(TAG, "storeThumbnailBytes: stored ${bytes.size} bytes for $id")
    }

    /**
     * Return the raw file bytes stored for file item [id], or null.
     * File bytes are persisted under the key "item_file_<id>" as Base64 NO_WRAP.
     */
    fun getFileBytes(id: String): ByteArray? {
        val b64 = prefs.getString("item_file_$id", null) ?: return null
        return try {
            Base64.decode(b64, Base64.NO_WRAP)
        } catch (e: Exception) {
            Log.w(TAG, "getFileBytes: failed to decode file for $id: ${e.message}")
            null
        }
    }

    /**
     * Persist raw file bytes for item [id] under "item_file_<id>".
     * Rejects files larger than [Settings.maxImageSizeBytes] (reuses the same cap
     * as images — both are binary blobs subject to the same quota).
     */
    fun storeFileBytes(id: String, bytes: ByteArray) {
        val maxBytes = settings.maxImageSizeBytes
        if (bytes.size.toLong() > maxBytes) {
            Log.w(TAG, "storeFileBytes: file ${bytes.size} B exceeds cap $maxBytes — dropping")
            return
        }
        val b64 = Base64.encodeToString(bytes, Base64.NO_WRAP)
        prefs.edit().putString("item_file_$id", b64).apply()
        Log.d(TAG, "storeFileBytes: stored ${bytes.size} bytes for $id")
    }

    /**
     * Return the stored (fileName, mime) pair for file item [id], or (null, null).
     * Metadata is stored as a pipe-delimited pair under "item_filemeta_<id>".
     * An empty/absent field is returned as null.
     */
    fun getFileMeta(id: String): Pair<String?, String?> {
        val raw = prefs.getString("item_filemeta_$id", null) ?: return null to null
        val parts = raw.split("|", limit = 2)
        val fileName = parts.getOrNull(0)?.takeIf { it.isNotEmpty() }
        val mime = parts.getOrNull(1)?.takeIf { it.isNotEmpty() }
        return fileName to mime
    }

    /**
     * Persist filename and mime for file item [id] under "item_filemeta_<id>".
     * Either value may be null; stored as empty string in that case.
     */
    fun storeFileMeta(id: String, fileName: String?, mime: String?) {
        val encoded = "${fileName ?: ""}|${mime ?: ""}"
        prefs.edit().putString("item_filemeta_$id", encoded).apply()
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
        val tombstoned = synchronized(idsWriteLock) {
            val ids = storedIds()
            if (id !in ids) return@synchronized false
            val existing = prefs.getString("item_$id", null) ?: return@synchronized false
            // Already a tombstone — nothing to do.
            if (isDeletedBlob(existing)) return@synchronized false

            val pinnedList = storedPinnedList().toMutableList()
            val wasPinned = pinnedList.remove(id)

            // Write a soft-delete tombstone: bump lamportTs by 1 so a concurrent
            // LWW re-sync of the original text (with a lower lamportTs) is rejected.
            val oldLamport = try {
                val parts = existing.split("|")
                if (parts.size >= 6) parts[5].toLongOrNull() ?: 0L else 0L
            } catch (_: Exception) { 0L }
            val tombstone = encodeTombstone(existing, oldLamport + 1L)

            // Clear binary sidecars: image/file bytes are no longer needed once
            // the item is logically deleted (saves storage; tombstone keeps the id
            // in the index to prevent re-sync resurrection).
            val editor = prefs.edit()
                .putString("item_$id", tombstone)
                .remove("item_img_$id")
                .remove("item_thumb_$id")
                .remove("item_file_$id")
                .remove("item_filemeta_$id")
            if (wasPinned) {
                editor.putString(KEY_PINNED_IDS, pinnedList.joinToString(","))
            }
            editor.apply()
            true
        }
        // Keep the foreground-service notification's "captured today" count from
        // drifting above reality after a deletion: decrement by one (floored at
        // 0) and re-issue the notification so the shown number matches the store.
        // Only fires when an item was actually tombstoned.
        if (tombstoned) {
            evictParseCache(id) // A: evict stale decrypt cache entry (blob is now a tombstone)
            ClipboardService.onItemsDeleted(appContext, 1)
        }
        tombstoned
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
            val pinnedList = storedPinnedList().toMutableList()
            val pinnedBefore = pinnedList.size
            pinnedList.removeAll(toDelete)
            val pinnedChanged = pinnedList.size != pinnedBefore
            val editor = prefs.edit()
                .putString(KEY_ITEM_IDS, storedList.joinToString(","))
            for (id in toDelete) {
                editor.remove("item_$id")
                editor.remove("item_img_$id")
                editor.remove("item_thumb_$id")
                editor.remove("item_file_$id")
                editor.remove("item_filemeta_$id")
                // Remove reverse-lookup key to prevent orphan LWW ghost on re-sync.
                editor.remove("item_id_ref_$id")
            }
            if (pinnedChanged) {
                editor.putString(KEY_PINNED_IDS, pinnedList.joinToString(","))
            }
            editor.apply()
        }
        if (deletedCount > 0) {
            // A: evict deleted ids from the decrypt cache so stale entries don't linger.
            for (id in toDelete) evictParseCache(id)
            ClipboardService.onItemsDeleted(appContext, deletedCount)
        }
        Log.d(TAG, "deleteItems: removed $deletedCount items")
    }

    /**
     * Delete all UNPINNED items (text blobs + image blobs + synced-source-id set).
     * Pinned items are preserved — mirrors the macOS daemon `DELETE WHERE pinned = 0`
     * fix (HW-A13). Previously this wiped everything including pinned items;
     * the behaviour is now consistent across platforms so no user-pinned clip is
     * ever silently removed by a "clear" action.
     */
    fun clearAll() {
        var deletedCount = 0
        synchronized(idsWriteLock) {
            val pinnedSet = storedPinnedIds()
            val ids = storedIds()
            val editor = prefs.edit()
            for (id in ids) {
                if (id !in pinnedSet) {
                    editor.remove("item_$id")
                    editor.remove("item_img_$id")
                    editor.remove("item_thumb_$id")
                    editor.remove("item_file_$id")
                    editor.remove("item_filemeta_$id")
                    // Remove reverse-lookup key to prevent orphan LWW ghost on re-sync.
                    editor.remove("item_id_ref_$id")
                }
            }
            // Retain only pinned ids in the index; clear the synced-source-id set
            // (re-syncing pinned items on the next poll is safe).
            val remaining = ids.filter { it in pinnedSet }
            deletedCount = ids.size - remaining.size
            editor
                .putString(KEY_ITEM_IDS, remaining.joinToString(","))
                .remove(KEY_SYNCED_SOURCE_IDS)
                .apply()
        }
        // Reset cross-listener dedup state so a re-copy after a clear stores a
        // fresh row instead of being silently skipped as a duplicate.
        resetDedupState()
        if (deletedCount > 0) {
            evictAllParseCache() // A: full cache wipe — most entries are now gone
            ClipboardService.onItemsDeleted(appContext, deletedCount)
        }
        Log.d(TAG, "clearAll: deleted $deletedCount unpinned items (pinned items preserved)")
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
                    editor.remove("item_thumb_$id")
                    editor.remove("item_file_$id")
                    editor.remove("item_filemeta_$id")
                    // Remove reverse-lookup key to prevent orphan LWW ghost on re-sync.
                    editor.remove("item_id_ref_$id")
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
            evictAllParseCache() // A: full cache wipe — most entries are now gone
            ClipboardService.onItemsDeleted(appContext, deletedCount)
        }
        Log.d(TAG, "clearUnpinned: all unpinned items deleted")
    }

    /**
     * Pin or unpin item [id].
     * Pinned items survive the retention prune pass and have no sensitive TTL.
     * The order of pinned ids in [KEY_PINNED_IDS] reflects display order (first = top).
     * Newly pinned items are prepended so they appear at the top of the pinned section.
     */
    fun setPinned(id: String, pinned: Boolean) {
        synchronized(idsWriteLock) {
            val pinnedList = storedPinnedList().toMutableList()
            val changed = if (pinned) {
                if (id !in pinnedList) {
                    pinnedList.add(0, id) // prepend — new pins appear at the top
                    true
                } else false
            } else {
                pinnedList.remove(id)
            }
            if (changed) {
                // commit() (synchronous) so the new pinned set survives an immediate
                // force-stop (SIGKILL) — matches the project pattern from 0f1d1ef.
                prefs.edit().putString(KEY_PINNED_IDS, pinnedList.joinToString(",")).commit()
            }
        }
        Log.d(TAG, "setPinned: item $id pinned=$pinned")
    }

    /**
     * Reorder pinned items.
     *
     * [ids] must contain exactly the currently-pinned item IDs in the desired
     * new display order (first element = top of the pinned section).
     * Unknown ids are silently ignored; missing pinned ids are appended at the end
     * to avoid data loss.
     */
    fun reorderPinned(ids: List<String>) {
        synchronized(idsWriteLock) {
            val currentPinned = storedPinnedList().toMutableSet()
            // Accept only ids that are actually pinned; preserve order from caller.
            val reordered = ids.filter { it in currentPinned }.toMutableList()
            // Append any pinned ids that were not included in the caller's list.
            val missing = currentPinned.filter { it !in reordered }
            reordered.addAll(missing)
            // commit() (synchronous) so the reordered set survives an immediate
            // force-stop (SIGKILL) — matches the project pattern from 0f1d1ef.
            prefs.edit().putString(KEY_PINNED_IDS, reordered.joinToString(",")).commit()
        }
        Log.d(TAG, "reorderPinned: new order = $ids")
    }

    /**
     * Move item [id] to the top of the non-pinned (recency) section by re-stamping
     * its wall-time to now and moving it to the END of the stored id index (the end
     * is "most recent" because [getItems] does takeLast().reversed()).
     *
     * PINNED items are skipped: their position is governed solely by
     * [KEY_PINNED_IDS] / pinnedSortIndex, so re-copying a pinned clip must NOT
     * move it. Mirrors macOS `bump_item_recency` on copy (HW parity).
     *
     * Only field 0 (wall-time) of the pipe-delimited blob is rewritten; the crypto
     * fields (nonce/ciphertext) and lamport_ts are preserved verbatim so the AEAD
     * AAD binding and LWW ordering remain intact.
     */
    fun bumpToTop(id: String) {
        synchronized(idsWriteLock) {
            if (id in storedPinnedIds()) return  // pinned items keep their fixed order
            val ids = storedIds().toMutableList()
            if (!ids.remove(id)) return  // unknown id — nothing to bump
            val raw = prefs.getString("item_$id", null) ?: return
            // Soft-delete tombstone: tombstoned items must not be bumped to the top
            // of the visible history — they are logically deleted.
            if (isDeletedBlob(raw)) return
            val parts = raw.split("|")
            // v3 blob = <wallTimeMs>|<contentType>|<payloadBytes>|<nonceB64>|<ciphertextB64>|<lamportTs>|<deleted>
            if (parts.size < 6) return  // legacy/malformed — leave untouched
            val rebuilt = buildString {
                append(System.currentTimeMillis())  // field 0: fresh wall-time
                for (i in 1 until parts.size) {
                    append('|')
                    append(parts[i])
                }
            }
            ids.add(id)  // re-append → most-recent position
            prefs.edit()
                .putString("item_$id", rebuilt)
                .putString(KEY_ITEM_IDS, ids.joinToString(","))
                .commit()  // synchronous: survives an immediate force-stop (SIGKILL)
        }
        Log.d(TAG, "bumpToTop: re-stamped item $id to most-recent")
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
        wallTimeMs: Long = System.currentTimeMillis(),
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
        // E7: key on content LENGTH + hash rather than a bare 32-bit hashCode().
        // A length-prefix makes an accidental collision far less likely — a
        // different clip would have to share both its length and its hashCode
        // within the window to be wrongly dropped.
        val dedupKey = "${plaintext.length}:${plaintext.hashCode()}"
        synchronized(dedupLock) {
            val now = System.currentTimeMillis()
            if (dedupKey == lastStoredKey && now - lastStoredAtMs < DEDUP_WINDOW_MS) {
                Log.d(TAG, "Duplicate clip within ${DEDUP_WINDOW_MS}ms — skipping")
                return@withContext ""
            }
            lastStoredKey = dedupKey
            lastStoredAtMs = now
        }

        // AB-6b — PARITY with macOS: do NOT drop sensitive items. macOS stores
        // them (the daemon persists every captured clip) and masks them in the
        // UI. Dropping them on Android meant macOS-captured secrets never showed
        // up here, breaking cross-device coherence. We now STORE the item; the
        // is_sensitive flag is recomputed at read time by parseItem() and drives
        // the PRIVATE chip + masked preview in HistoryActivity. (The native
        // detector threshold was aligned to >=0.70 in ABI 14 so the capture-time
        // and read-time verdicts agree.)

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

        val encoded = encodeItem(blob, textBytes.size, contentType = contentType, lamportTs = lamportTs, wallTimeMs = wallTimeMs)
        synchronized(idsWriteLock) {
            // Append the id, removing any prior occurrence first so the index
            // stays canonical (no duplicate ids). A synced item re-stored under
            // the same overrideId — e.g. after clearUnpinned wiped the
            // synced-source-id seen-set while a pinned id stayed in the index —
            // would otherwise append a second copy of the same id, which then
            // crashes the history LazyColumn ("Key … was already used").
            val ids = appendUniqueId(storedIds(), id)
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
        wallTimeMs: Long = System.currentTimeMillis(),
    ): Boolean = withContext(Dispatchers.IO) {
        if (plaintext.isBlank()) return@withContext false

        // AB-6b — PARITY with macOS: store sensitive synced items instead of
        // dropping them. A sensitive clip captured on macOS must round-trip to
        // Android and render masked, not silently vanish. Sensitivity is
        // recomputed at read time by parseItem() and drives the masked preview.

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
            val encoded = encodeItem(blob, plaintextBytes.size, lamportTs = incomingLamportTs, wallTimeMs = wallTimeMs)
            prefs.edit().putString("item_$existingStorageId", encoded).apply()
            evictParseCache(existingStorageId) // A: blob changed — evict stale decrypt entry
            Log.d(TAG, "LWW replaced item_id=$itemId storageId=$existingStorageId (lamport $storedTs→$incomingLamportTs)")
            true  // replaced successfully
        }

        // null  → duplicate (older/equal lamport), skip (nothing changed → no prune)
        // true  → replaced in-place; prune since the replace may have grown a row
        // false → item not found, fall through to new-item insert below
        when (replaced) {
            null -> return@withContext false
            true -> {
                // The replace's synchronized(idsWriteLock) block has already exited
                // above, so pruneToLimits() (which takes idsWriteLock) cannot deadlock.
                pruneToLimits()
                return@withContext true
            }
            else -> { /* false: fall through to new-item insert below */ }
        }

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
        val encoded = encodeItem(blob, plaintextBytes.size, lamportTs = incomingLamportTs, wallTimeMs = wallTimeMs)

        synchronized(idsWriteLock) {
            // TOCTOU guard: re-check inside the lock. A concurrent caller (FgsSyncLoop
            // + SupabasePollWorker both polling) may have raced through the new-item
            // path and already inserted this itemId between our first lookup (above,
            // which returned false) and now. If so, abort to avoid a duplicate row.
            if (prefs.getString("item_id_ref_$storageId", null) != null) {
                Log.d(TAG, "storeItemWithLww: duplicate detected under lock for item_id=$itemId — aborting")
                return@withContext false
            }
            val ids = appendUniqueId(storedIds(), storageId)
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
            loadFullPlaintextBlocking(id, key)
        }

    /**
     * AB-11 — full-content search. Returns the subset of [ids] whose FULL
     * decrypted text contains [query] (case-insensitive). The snippet-only filter
     * in [HistoryActivity] missed matches past the 140-char preview; this decrypts
     * each candidate and matches the whole payload.
     *
     * Image / file items have no searchable text body, so they are matched on
     * their stored snippet/label only (decrypting yields the same label). A blank
     * [query] returns all [ids] unchanged. Decryption failures are treated as
     * non-matches rather than propagating an error.
     *
     * Runs on [Dispatchers.IO]; the caller is expected to debounce.
     */
    suspend fun searchIds(ids: List<String>, query: String, key: ByteArray): Set<String> =
        withContext(Dispatchers.IO) {
            val q = query.trim()
            if (q.isEmpty()) return@withContext ids.toSet()
            ids.filterTo(HashSet()) { id ->
                val full = loadFullPlaintextBlocking(id, key)
                full != null && full.contains(q, ignoreCase = true)
            }
        }

    /**
     * Synchronous full-plaintext decrypt for use inside an already-`IO` context
     * (e.g. [searchIds]). Mirrors [loadFullPlaintext] without an extra dispatch.
     */
    private fun loadFullPlaintextBlocking(id: String, key: ByteArray): String? {
        val raw = prefs.getString("item_$id", null) ?: return null
        val parts = raw.split("|")
        val nonceB64 = parts.getOrNull(3) ?: return null
        val ctB64 = parts.getOrNull(4) ?: return null
        return try {
            val nonce = Base64.decode(nonceB64, Base64.NO_WRAP)
            val ciphertext = Base64.decode(ctB64, Base64.NO_WRAP)
            decryptForPreview(id, ciphertext, nonce, key)
        } catch (e: Exception) {
            Log.d(TAG, "loadFullPlaintextBlocking: decrypt failed for $id: ${e.message}")
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
                // Measure the SAME unit storeImageBytes caps on: raw decoded bytes,
                // not the ~1.33x-inflated Base64 string length. Deriving the raw
                // size from the Base64 length (NO_WRAP, so it is a multiple of 4
                // with '=' padding) avoids a full decode allocation per row.
                val imgBytes = prefs.getString("item_img_$id", null)?.let { base64RawByteSize(it) } ?: 0
                val thumbBytes = prefs.getString("item_thumb_$id", null)?.let { base64RawByteSize(it) } ?: 0
                val fileBytes = prefs.getString("item_file_$id", null)?.let { base64RawByteSize(it) } ?: 0
                id to (textBytes + imgBytes + thumbBytes + fileBytes)
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
                editor.remove("item_thumb_$evictId")
                editor.remove("item_file_$evictId")
                editor.remove("item_filemeta_$evictId")
                // Remove reverse-lookup key to prevent orphan LWW ghost on re-sync.
                editor.remove("item_id_ref_$evictId")
                evictParseCache(evictId) // A: evict stale decrypt cache entry
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

    /**
     * AB-13 — retention TTL auto-wipe (macOS parity).
     *
     * macOS auto-wipes by two age policies; Android had neither. This pass
     * deletes:
     *  - any UNPINNED item older than the GENERAL retention TTL
     *    ([generalTtlSecs], default [DEFAULT_GENERAL_TTL_SECS] = 30 days, mirroring
     *    the macOS `sync_ttl_secs` retention floor); and
     *  - any UNPINNED *sensitive* item older than [Settings.sensitiveTtlSecs]
     *    (default 30 s; `0` disables, exactly like macOS).
     *
     * A TTL of `0` disables that pass (the macOS "never wipe" sentinel). PINNED
     * items are never aged out — they survive until an explicit user delete,
     * matching [pruneToLimits].
     *
     * Sensitivity is only evaluated for items already past the (short) sensitive
     * TTL window AND only when the sensitive pass is enabled, so the per-row
     * decrypt cost is bounded — fresh items are never decrypted here.
     *
     * Wall-time is field 0 of the pipe-delimited blob (written by [encodeItem]),
     * so the general pass needs no decrypt at all.
     */
    private fun pruneByAge(key: ByteArray? = null) {
        val generalTtlSecs = generalTtlSecs().coerceAtLeast(0L)
        val sensitiveTtlSecs = settings.sensitiveTtlSecs.coerceAtLeast(0L)
        if (generalTtlSecs == 0L && sensitiveTtlSecs == 0L) return // both disabled

        val now = System.currentTimeMillis()
        val generalCutoffMs = if (generalTtlSecs > 0L) now - generalTtlSecs * 1000L else Long.MIN_VALUE
        val sensitiveCutoffMs = if (sensitiveTtlSecs > 0L) now - sensitiveTtlSecs * 1000L else Long.MIN_VALUE
        var deletedCount = 0

        synchronized(idsWriteLock) {
            val pinnedSet = storedPinnedIds()
            val ids = storedIds()
            val editor = prefs.edit()
            val survivors = ArrayList<String>(ids.size)

            for (id in ids) {
                if (id in pinnedSet) {
                    survivors.add(id) // pinned items never age out
                    continue
                }
                val raw = prefs.getString("item_$id", null)
                if (raw == null) {
                    // Index references a missing blob — drop the dangling id.
                    continue
                }
                val wallTimeMs = raw.substringBefore('|').toLongOrNull()
                if (wallTimeMs == null) {
                    survivors.add(id) // malformed — leave it for the normal prune
                    continue
                }

                // General retention: oldest-first absolute age cap.
                val expiredByGeneral = generalTtlSecs > 0L && wallTimeMs < generalCutoffMs

                // Sensitive retention: only decrypt+classify items already past the
                // sensitive window (cheap fast-path skips the vast majority of rows).
                val expiredBySensitive = sensitiveTtlSecs > 0L &&
                    wallTimeMs < sensitiveCutoffMs &&
                    isItemSensitive(id, raw, key)

                if (expiredByGeneral || expiredBySensitive) {
                    editor.remove("item_$id")
                    editor.remove("item_img_$id")
                    editor.remove("item_thumb_$id")
                    editor.remove("item_file_$id")
                    editor.remove("item_filemeta_$id")
                    editor.remove("item_id_ref_$id")
                    deletedCount++
                    Log.d(TAG, "pruneByAge: wiped $id (general=$expiredByGeneral, sensitive=$expiredBySensitive)")
                } else {
                    survivors.add(id)
                }
            }

            if (deletedCount > 0) {
                editor.putString(KEY_ITEM_IDS, survivors.joinToString(",")).apply()
            }
        }

        if (deletedCount > 0) {
            ClipboardService.onItemsDeleted(appContext, deletedCount)
        }
    }

    /**
     * Decrypt [raw]'s payload and classify it via the native [isSensitive].
     * Returns false (treat as non-sensitive) when no [key] is available or the
     * blob cannot be decrypted, so a missing key never wrongly wipes data.
     */
    private fun isItemSensitive(id: String, raw: String, key: ByteArray?): Boolean {
        if (key == null) return false
        val parts = raw.split("|")
        val nonceB64 = parts.getOrNull(3) ?: return false
        val ctB64 = parts.getOrNull(4) ?: return false
        return try {
            val nonce = Base64.decode(nonceB64, Base64.NO_WRAP)
            val ciphertext = Base64.decode(ctB64, Base64.NO_WRAP)
            val plain = decryptForPreview(id, ciphertext, nonce, key)
            try {
                isSensitive(plain)
            } catch (_: UnsatisfiedLinkError) {
                false
            }
        } catch (e: Exception) {
            Log.d(TAG, "isItemSensitive: decrypt failed for $id: ${e.message}")
            false
        }
    }

    /**
     * General retention TTL in seconds. Read from the same "copypaste" prefs file
     * Settings owns (key `general_ttl_secs`) so a future settings UI can drive it;
     * defaults to [DEFAULT_GENERAL_TTL_SECS] (30 days) to mirror the macOS
     * `sync_ttl_secs` retention floor. `0` disables the general age pass.
     */
    private fun generalTtlSecs(): Long =
        appContext.getSharedPreferences(SETTINGS_PREFS_NAME, Context.MODE_PRIVATE)
            .getLong(KEY_GENERAL_TTL_SECS, DEFAULT_GENERAL_TTL_SECS)
            .coerceAtLeast(0L)

    /**
     * Raw decoded byte count of a Base64 (NO_WRAP) string, computed without
     * allocating the decoded buffer. NO_WRAP emits no line breaks, so the input
     * length is a multiple of 4 and any padding is 0–2 trailing '=' chars:
     *   rawBytes = (len / 4) * 3 - paddingCount
     * Used by [pruneToLimits] so image rows are accounted in the same unit
     * ([storeImageBytes] caps on raw `bytes.size`), preventing the byte quota
     * from being over-counted by the ~1.33x Base64 inflation.
     */
    private fun base64RawByteSize(b64: String): Int {
        val len = b64.length
        if (len == 0) return 0
        val padding = when {
            b64.endsWith("==") -> 2
            b64.endsWith("=") -> 1
            else -> 0
        }
        return (len / 4) * 3 - padding
    }

    /**
     * The ordered id index, read back canonical: blanks removed and any
     * duplicate ids collapsed to their FIRST (oldest) occurrence. Persisting a
     * dup-free index is the invariant every writer relies on; reading it
     * de-duplicated also heals any index that an older build may have corrupted,
     * so the history LazyColumn never sees a repeated key.
     */
    private fun storedIds(): List<String> =
        prefs.getString(KEY_ITEM_IDS, "")
            ?.split(",")
            ?.filter { it.isNotBlank() }
            ?.distinct()
            ?: emptyList()

    /**
     * Append [id] to [current], guaranteeing it appears exactly once and at the
     * end (most-recent position). Any prior occurrence is removed first so the
     * index can never hold the same id twice — the root invariant that keeps the
     * history LazyColumn's `key = { it.id }` from crashing on a duplicate.
     */
    private fun appendUniqueId(current: List<String>, id: String): List<String> {
        val next = current.toMutableList()
        next.remove(id)
        next.add(id)
        return next
    }

    /** Ordered list of pinned ids — position 0 is displayed at the top of the pinned section. */
    private fun storedPinnedList(): List<String> =
        prefs.getString(KEY_PINNED_IDS, "")
            ?.split(",")
            ?.filter { it.isNotBlank() }
            ?: emptyList()

    private fun storedPinnedIds(): Set<String> = storedPinnedList().toHashSet()

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
     * Encode a stored item as a pipe-delimited string (v3 format, 7 fields):
     * <wallTimeMs>|<contentType>|<payloadBytes>|<nonceB64>|<ciphertextB64>|<lamportTs>|<deleted>
     *
     * The lamportTs field (index 5) was added for LWW cloud sync. Legacy rows
     * (only 5 fields) are read back with lamportTs=0.
     *
     * The deleted field (index 6) was added for local soft-delete tombstones.
     * Legacy rows (fewer than 7 fields) parse as deleted=false (back-compat).
     * A tombstone has deleted=1; its ciphertext/nonce are empty strings so the
     * encrypted payload is not retained on disk after a user delete.
     */
    private fun encodeItem(
        blob: EncryptedBlob,
        plaintextLen: Int,
        contentType: String = "text/plain",
        lamportTs: Long = 0L,
        wallTimeMs: Long = System.currentTimeMillis(),
        deleted: Boolean = false,
    ): String {
        val nonce64 = Base64.encodeToString(blob.nonce, Base64.NO_WRAP)
        val ct64 = Base64.encodeToString(blob.ciphertext, Base64.NO_WRAP)
        val deletedFlag = if (deleted) 1 else 0
        return "$wallTimeMs|$contentType|$plaintextLen|$nonce64|$ct64|$lamportTs|$deletedFlag"
    }

    /**
     * Build a tombstone blob for item [id].
     *
     * The tombstone keeps the entry in the id-index so a re-sync cannot
     * resurrect the deleted item, but clears the encrypted payload to avoid
     * retaining plaintext on disk. Field layout mirrors [encodeItem] v3:
     * <nowMs>|<contentType>|0||tombstone|<lamportTs>|1
     *
     * The nonce field is empty and the ciphertext is the literal string
     * "tombstone" (harmless; the deleted flag prevents any decrypt attempt).
     * The lamportTs is bumped so LWW on the same item_id sees this as newer
     * and will not be overwritten by a stale re-sync of the original text.
     */
    private fun encodeTombstone(existingRaw: String, bumpedLamportTs: Long): String {
        val parts = existingRaw.split("|")
        val wallTimeMs = System.currentTimeMillis()
        // Preserve contentType from the original blob so tombstones are typed.
        val contentType = parts.getOrNull(1) ?: "text/plain"
        return "$wallTimeMs|$contentType|0||tombstone|$bumpedLamportTs|1"
    }

    /**
     * Read the deleted flag from a raw blob string.
     * Field 6 (index 6) is the deleted flag: "1" = deleted, absent/other = false.
     * Back-compat: blobs with fewer than 7 fields (legacy v1/v2 format) are NOT deleted.
     */
    private fun isDeletedBlob(raw: String): Boolean {
        val idx = raw.lastIndexOf('|')
        if (idx < 0) return false
        // Only the last field is the deleted flag; avoid a full split for perf.
        val tail = raw.substring(idx + 1)
        return tail == "1"
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

        // Text payload byte size is the encoded plaintextLen field (parts[2]) — the UTF-8
        // byte length recorded at capture by encodeItem(). Already in hand here, no blob load.
        // Image/file items carry their real bytes under separate prefs keys, so getItems()
        // overrides tooLargeToSync for those after this returns.
        val plaintextLen = parts.getOrNull(2)?.toLongOrNull() ?: 0L
        val tooLargeToSync = plaintextLen > SYNC_MAX_BLOB_BYTES

        // pinned, imagePng, and image/file tooLargeToSync are populated by getItems()
        // after parseItem returns.
        return ClipboardItem(
            id = id,
            contentType = contentType,
            isSensitive = sensitive,
            wallTimeMs = wallTimeMs,
            snippet = snippet,
            tooLargeToSync = tooLargeToSync,
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

        /**
         * Sync size ceiling in bytes (8 MiB). Items whose stored payload exceeds this are
         * flagged [ClipboardItem.tooLargeToSync] and will not propagate to other devices.
         * Matches the macOS/daemon sync blob cap exactly — single source of truth, do not
         * scatter the literal.
         */
        const val SYNC_MAX_BLOB_BYTES: Long = 8L * 1024 * 1024

        /** SharedPreferences file name — single source of truth, not scattered as string literals. */
        const val PREFS_NAME = "copypaste_items"

        /**
         * Name of the SharedPreferences file that [Settings] owns ("copypaste").
         * [generalTtlSecs] reads the general retention TTL from here so the value
         * is shared with any future settings UI without coupling to [Settings]'s
         * private prefs handle. Must stay in sync with the literal in Settings.
         */
        const val SETTINGS_PREFS_NAME = "copypaste"

        /** Pref key for the general retention TTL (seconds); `0` disables. */
        const val KEY_GENERAL_TTL_SECS = "general_ttl_secs"

        /**
         * Default general retention TTL = 30 days, mirroring the macOS
         * `SYNC_TTL_SECS` (2_592_000 s) retention floor. Items older than this are
         * auto-wiped by [pruneByAge] unless pinned.
         */
        const val DEFAULT_GENERAL_TTL_SECS: Long = 30L * 24 * 60 * 60

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
        @Volatile var lastStoredKey: String = ""
        @Volatile var lastStoredAtMs: Long = 0L
        val dedupLock = Any()

        /**
         * "Expected next clip" guard for copy-from-history (HIGH-3 follow-up).
         *
         * When the user taps a row in [HistoryActivity] to copy it, the UI calls
         * setPrimaryClip with that text. The capture listeners
         * ([ClipboardService] / [LogcatCaptureService]) then observe the
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
        @Volatile var expectedClipLen: Int = 0
        @Volatile var expectedClipHasValue: Boolean = false
        @Volatile var expectedClipAtMs: Long = 0L
        val expectedClipLock = Any()

        private const val EXPECTED_CLIP_WINDOW_MS = 5_000L

        // ── Image/URI copy-from-history echo guard ────────────────────────────
        // Mirrors the text guard above, but keyed by the content:// URI string
        // written to the clipboard when the user copies an image (or file) back
        // from the history list.  The capture listeners see an image/file MIME
        // clip whose URI is our own FileProvider URI — we must not re-store it.
        // 5-second window (same as text); does NOT clear on first match so that
        // concurrent ClipboardService + LogcatCaptureService callbacks
        // for the same user tap are both suppressed.
        @Volatile private var expectedImageUri: String = ""
        @Volatile private var expectedImageUriAtMs: Long = 0L
        @Volatile private var expectedImageUriHasValue: Boolean = false
        private val expectedImageUriLock = Any()

        /**
         * Record that the next observed clipboard change carrying an image (or
         * file) URI equal to [uri] is an internal copy-from-history echo and must
         * NOT be re-captured.  Call immediately before [ClipboardManager.setPrimaryClip]
         * in the image/file copy-back path of [HistoryActivity].
         */
        fun expectImageUri(uri: android.net.Uri) {
            synchronized(expectedImageUriLock) {
                expectedImageUri = uri.toString()
                expectedImageUriAtMs = System.currentTimeMillis()
                expectedImageUriHasValue = true
            }
        }

        /**
         * Returns true when [uri] matches the pending [expectImageUri] registration
         * within [EXPECTED_CLIP_WINDOW_MS].  Does NOT clear on a match so concurrent
         * listeners both get suppressed; the window expiry self-clears after 5 s.
         */
        fun shouldSkipExpectedImageUri(uri: android.net.Uri): Boolean {
            synchronized(expectedImageUriLock) {
                if (!expectedImageUriHasValue) return false
                val now = System.currentTimeMillis()
                if (now - expectedImageUriAtMs > EXPECTED_CLIP_WINDOW_MS) {
                    expectedImageUriHasValue = false
                    return false
                }
                if (uri.toString() == expectedImageUri) return true
                return false
            }
        }

        /**
         * Record that the next observed clipboard change carrying text whose
         * (length, hash) equals [content]'s is an internal copy-from-history echo
         * and must NOT be re-captured. Call immediately before setPrimaryClip.
         *
         * The match key is the clip's length plus its [String.hashCode] rather than
         * the full string, so a very large expected clip (megabytes of text) is
         * never retained or compared in full. Length is paired with the hash so a
         * hashCode collision between two different-length clips cannot match.
         */
        fun expectClip(content: String) {
            synchronized(expectedClipLock) {
                expectedClipHash = content.hashCode()
                expectedClipLen = content.length
                expectedClipHasValue = true
                expectedClipAtMs = System.currentTimeMillis()
            }
        }

        /**
         * Returns true when [content] matches a pending [expectClip] within
         * [EXPECTED_CLIP_WINDOW_MS].
         *
         * The expectation is NOT cleared on a match — it stays active for the
         * full window so that all concurrent listeners (ClipboardService,
         * LogcatCaptureService, MainActivity) that fire for the same
         * user tap are all suppressed, not just the first one.  Without this,
         * the second listener would see [expectedClipHasValue] already cleared
         * and store a duplicate row.
         *
         * The expectation is cleared only when:
         *   - the window expires (stale expectation — genuinely new copy), or
         *   - the (length, hash) does NOT match (different clip — not our echo).
         *
         * Matching on (length, hash) instead of full-string equality avoids
         * retaining/comparing the entire clip text for large clips while keeping
         * the suppression semantics identical for matching clips.
         *
         * A later genuine re-copy of the same text after [EXPECTED_CLIP_WINDOW_MS]
         * has elapsed will not be suppressed because the window will have expired.
         */
        fun shouldSkipExpectedClip(content: String): Boolean {
            synchronized(expectedClipLock) {
                if (!expectedClipHasValue) return false
                val now = System.currentTimeMillis()
                if (now - expectedClipAtMs > EXPECTED_CLIP_WINDOW_MS) {
                    // Window expired — clear and treat as a new clip.
                    expectedClipHasValue = false
                    return false
                }
                if (content.length == expectedClipLen && content.hashCode() == expectedClipHash) {
                    // (length, hash) matches within window: suppress this echo.
                    // Do NOT clear expectedClipHasValue — other concurrent
                    // listeners firing for the same tap must also be suppressed.
                    // The window expiry above will self-clear after 5 s.
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
                lastStoredKey = ""
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

        /**
         * Process-wide SecureRandom singleton. SecureRandom is thread-safe and
         * expensive to instantiate (seeds from /dev/urandom on Android). Promoting
         * it here avoids re-instantiation on every localAesEncrypt call.
         */
        private val secureRandom = java.security.SecureRandom()

        // ── A: decrypt result cache ──────────────────────────────────────────────
        //
        // getItems() previously called parseItem()/decryptForPreview() for EVERY id
        // on EVERY reload — a full AEAD decrypt + native isSensitive() per row.
        // On a 200-item list this saturates Dispatchers.IO and produces Davey frames.
        //
        // We cache the parsed ClipboardItem keyed by storage id, invalidated only
        // when the raw blob string changes (i.e. the item was actually written).
        // getItems() reads the cheap prefs.getString("item_$id") and only decrypts
        // when the raw blob differs from the cached entry; otherwise reuses the
        // cached item. On a quiescent list (no new items since last load) this
        // reduces decryptions from N→0.
        //
        // The cache stores (rawBlob, ClipboardItem) without imagePng (that field
        // is removed). getItems() always applies cheap pinned/pinnedSortIndex/
        // tooLargeToSync overrides via .copy() after the cache lookup.

        private data class ParsedEntry(val rawBlob: String, val item: ClipboardItem)

        /** Guards [parseCache] for concurrent IO reads. */
        private val parseCacheLock = Any()

        /**
         * Maps storage id → (rawBlob, ClipboardItem). Process-wide so multiple
         * ClipboardRepository instances (VM + searchRepository + filePickLauncher)
         * share the same warm cache.
         */
        private val parseCache = HashMap<String, ParsedEntry>()

        /**
         * Evict a single id from the parse cache.
         * Call on delete / LWW-replace paths alongside evictImageCaches.
         */
        fun evictParseCache(id: String) {
            synchronized(parseCacheLock) { parseCache.remove(id) }
        }

        /** Evict ALL entries — call on clearAll / clearUnpinned. */
        fun evictAllParseCache() {
            synchronized(parseCacheLock) { parseCache.clear() }
        }

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
                secureRandom.nextBytes(it)
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
     * Decrypt ALL locally stored items into [uniffi.copypaste_android.LocalItem]
     * values for a P2P/cloud sync push.
     *
     * No arbitrary count cap is applied. The only legitimate size bound is the
     * byte-cap retention (items are pruned when local storage exceeds the
     * configured byte limit), which already runs at capture/load time. The sync
     * layer deduplicates via LWW/Lamport, so re-offering previously-synced items
     * is cheap and guarantees full convergence between devices.
     *
     * For `content_type == "file"` items: the stored plaintext is a human-readable
     * label (e.g. "[file: report.pdf]"). The actual bytes are loaded from the
     * file-bytes sidecar store ([getFileBytes]) and used as the FFI `plaintext`
     * so the peer receives the real file content. File metadata is attached via
     * [getFileMeta] into the new ABI-8 `fileName`/`mime` fields. Items whose
     * file-bytes sidecar is missing (e.g. storage failure at capture) are skipped.
     */
    suspend fun localItemsForSync(
        key: ByteArray,
    ): List<uniffi.copypaste_android.LocalItem> = withContext(Dispatchers.IO) {
        val ids = storedIds()
        ids.mapNotNull { id ->
            val raw = prefs.getString("item_$id", null) ?: return@mapNotNull null
            // Soft-delete tombstone: skip deleted items — FFI tombstone propagation
            // is a separate later task; for now tombstones must not sync outbound.
            if (isDeletedBlob(raw)) return@mapNotNull null
            try {
                val parts = raw.split("|")
                val wallTimeMs = parts[0].toLong()
                val contentType = normalizeContentTypeForSync(parts[1])
                val nonce = Base64.decode(parts[3], Base64.NO_WRAP)
                val ciphertext = Base64.decode(parts[4], Base64.NO_WRAP)
                val plain = decryptText(id, ciphertext, nonce, key)

                val isImage = contentType == "image" || contentType.startsWith("image/")
                if (contentType == "file") {
                    // For file items the raw plaintext is just a label; the peer
                    // needs the actual file bytes. Fetch from the sidecar store.
                    val fileBytes = getFileBytes(id)
                    if (fileBytes == null || fileBytes.isEmpty()) {
                        Log.d(TAG, "Skipping file item $id for sync: bytes missing or empty")
                        return@mapNotNull null
                    }
                    val (fileName, mime) = getFileMeta(id)
                    uniffi.copypaste_android.LocalItem(
                        id = id,
                        itemId = id,
                        wallTimeMs = wallTimeMs,
                        contentType = contentType,
                        plaintext = fileBytes.map { it.toUByte() },
                        fileName = fileName,
                        mime = mime,
                    )
                } else if (isImage) {
                    // AB-5: for image items the raw plaintext is the content:// URI
                    // placeholder, NOT the pixels. Attach the real image bytes from
                    // the sidecar store (mirrors the file branch) so P2P/cloud send
                    // ships actual bytes instead of a useless URI string.
                    val imageBytes = getImageBytes(id)
                    if (imageBytes == null || imageBytes.isEmpty()) {
                        Log.d(TAG, "Skipping image item $id for sync: bytes missing or empty")
                        return@mapNotNull null
                    }
                    uniffi.copypaste_android.LocalItem(
                        id = id,
                        itemId = id,
                        wallTimeMs = wallTimeMs,
                        contentType = contentType,
                        plaintext = imageBytes.map { it.toUByte() },
                        // Images carry no in-band name/MIME header (only files do).
                        fileName = null,
                        mime = null,
                    )
                } else {
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
                        fileName = null,
                        mime = null,
                    )
                }
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
