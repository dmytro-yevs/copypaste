package com.copypaste.android

import android.util.Log
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/**
 * Sync, export, import, and inbound-tombstone helpers for [ClipboardRepository].
 *
 * Extracted from [ClipboardRepository] (CopyPaste-ra15.4). These extension functions
 * access [ClipboardRepository]'s internal fields via the extension receiver and call
 * [ClipboardBlobCodec] directly (bypassing the thin private-wrapper aliases in the class).
 */

private const val TAG = "ClipboardRepository"

/**
 * Decrypt ALL locally stored items into [uniffi.copypaste_android.LocalItem] values
 * for a P2P/cloud sync push.
 *
 * Implementation of [ClipboardRepository.localItemsForSync]. See the caller for full docs.
 */
@OptIn(kotlin.ExperimentalUnsignedTypes::class)
internal suspend fun ClipboardRepository.localItemsForSyncImpl(
    key: ByteArray,
): List<uniffi.copypaste_android.LocalItem> = withContext(Dispatchers.IO) {
    val ids = storedIds()
    // Snapshot pin state once: storedPinnedList() is ordered (index = sort position).
    val pinnedList = storedPinnedList()
    val pinnedSet = pinnedList.toHashSet()
    // pin_order: position in the pinned list (0 = top of pinned section) as a
    // 1-based f64 so the macOS daemon can sort correctly. None for unpinned.
    val pinnedOrderMap: Map<String, Double> =
        pinnedList.mapIndexed { idx, pid -> pid to (idx + 1).toDouble() }.toMap()

    // ── Pass 1: parse and snapshot all non-tombstone rows. ────────────────
    // Collect row metadata and the raw crypto fields needed for batch decrypt.
    // Tombstones are emitted directly (no decrypt needed).
    data class ParsedRow(
        val id: String,
        val isPinned: Boolean,
        val pinOrder: Double?,
        val wallTimeMs: Long,
        val contentType: String,
        val parts: List<String>,
        // null means the nonce/ciphertext fields were missing (malformed row).
        val encryptedItem: uniffi.copypaste_android.EncryptedItem?,
    )

    val tombstones = mutableListOf<uniffi.copypaste_android.LocalItem>()
    val parsedRows = mutableListOf<ParsedRow>()

    for (id in ids) {
        val raw = prefs.getString("item_$id", null) ?: continue
        val isPinned = id in pinnedSet
        val pinOrder: Double? = pinnedOrderMap[id]

        // ABI 15: include soft-delete tombstones so they propagate to peers.
        // Tombstones carry deleted=true with empty plaintext (no decrypt needed).
        if (ClipboardBlobCodec.isDeletedBlob(raw)) {
            try {
                val parts = raw.split("|")
                val wallTimeMs = parts[0].toLong()
                val contentType = ClipboardRepository.normalizeContentTypeForSync(
                    parts.getOrNull(1) ?: "text",
                )
                tombstones.add(
                    uniffi.copypaste_android.LocalItem(
                        id = id,
                        itemId = id,
                        wallTimeMs = wallTimeMs,
                        contentType = contentType,
                        plaintext = emptyList(),
                        fileName = null,
                        mime = null,
                        deleted = true,
                        pinned = false,      // tombstones are never pinned
                        pinOrder = null,
                    )
                )
            } catch (e: Exception) {
                Log.d(TAG, "Skipping tombstone $id for sync (parse failed): ${e.message}")
            }
            continue
        }

        try {
            val parts = raw.split("|")
            val wallTimeMs = parts[0].toLong()
            val contentType = ClipboardRepository.normalizeContentTypeForSync(parts[1])
            val nonceB64 = parts.getOrNull(3)
            val ctB64 = parts.getOrNull(4)
            val encryptedItem = if (nonceB64 != null && ctB64 != null &&
                nonceB64.isNotEmpty() && ctB64.isNotEmpty()
            ) {
                val nonce = android.util.Base64.decode(nonceB64, android.util.Base64.NO_WRAP)
                val ciphertext = android.util.Base64.decode(ctB64, android.util.Base64.NO_WRAP)
                uniffi.copypaste_android.EncryptedItem(
                    itemId = id,
                    ciphertext = ciphertext.asUByteArray().asList(),
                    nonce = nonce.asUByteArray().asList(),
                    keyVersion = ClipboardBlobCodec.keyVersionFromParts(parts),
                )
            } else {
                null
            }
            parsedRows.add(ParsedRow(id, isPinned, pinOrder, wallTimeMs, contentType, parts, encryptedItem))
        } catch (e: Exception) {
            Log.w(TAG, "localItemsForSync: skipping item $id for sync (parse failed): ${e.message}")
        }
    }

    // ── Pass 2: ONE batch FFI call to decrypt all parseable items. ────────
    // Items whose crypto fields were missing or malformed have encryptedItem=null
    // and will be handled per-branch below (image/file items need no text decrypt).
    val encryptedItems = parsedRows.mapNotNull { it.encryptedItem }
    val batchResult = try {
        decryptTextBatch(encryptedItems, key)
    } catch (e: IllegalStateException) {
        // Native library absent: stub-mode returns empty result, log once.
        Log.w(TAG, "localItemsForSync: native library unavailable for batch decrypt — " +
            "all text items will be skipped")
        uniffi.copypaste_android.DecryptBatchResult(items = emptyList(), skipped = encryptedItems.size.toUInt())
    }

    // Log aggregate skip count once instead of per-item WARN spam.
    if (batchResult.skipped > 0u) {
        Log.i(TAG, "localItemsForSync: skipped ${batchResult.skipped} undecryptable legacy items")
    }

    // Build a map from itemId → plaintext bytes for O(1) lookup below.
    val plaintextMap: Map<String, ByteArray> = batchResult.items.associate { decrypted ->
        decrypted.itemId to ByteArray(decrypted.plaintext.size) { i -> decrypted.plaintext[i].toByte() }
    }

    // ── Pass 3: assemble LocalItem list. ─────────────────────────────────
    val localItems = parsedRows.mapNotNull { row ->
        val (id, isPinned, pinOrder, wallTimeMs, contentType, _, _) = row
        val isImage = contentTypeIsImage(contentType)
        try {
            if (contentTypeIsFile(contentType)) {
                // For file items the raw plaintext is just a label; the peer
                // needs the actual file bytes. Fetch from the sidecar store.
                // Decrypt result is irrelevant for file items — sidecar bytes are used.
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
                    plaintext = fileBytes.asUByteArray().asList(),
                    fileName = fileName,
                    mime = mime,
                    deleted = false,
                    pinned = isPinned,
                    pinOrder = pinOrder,
                )
            } else if (isImage) {
                // AB-5: for image items the raw plaintext is the content:// URI
                // placeholder, NOT the pixels. Attach the real image bytes from
                // the sidecar store (mirrors the file branch) so P2P/cloud send
                // ships actual bytes instead of a useless URI string.
                // Decrypt result is irrelevant for image items — sidecar bytes are used.
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
                    plaintext = imageBytes.asUByteArray().asList(),
                    // Images carry no in-band name/MIME header (only files do).
                    fileName = null,
                    mime = null,
                    deleted = false,
                    pinned = isPinned,
                    pinOrder = pinOrder,
                )
            } else {
                // Text item: look up the plaintext from the batch result.
                // Skip gracefully when decrypt failed (legacy AAD / wrong key).
                val plain = plaintextMap[id]
                if (plain == null) {
                    // Already counted in batchResult.skipped — no per-item log here.
                    return@mapNotNull null
                }
                uniffi.copypaste_android.LocalItem(
                    id = id,
                    // STABLE cross-device identity. The row id is minted ONCE at
                    // capture (or carried from an incoming item) and persisted,
                    // so reusing it as item_id lets the daemon dedup/LWW-merge
                    // this clip instead of seeing a fresh item on every dial.
                    itemId = id,
                    wallTimeMs = wallTimeMs,
                    contentType = contentType,
                    plaintext = plain.asUByteArray().asList(),
                    fileName = null,
                    mime = null,
                    deleted = false,
                    pinned = isPinned,
                    pinOrder = pinOrder,
                )
            }
        } catch (e: Exception) {
            // WARN (not DEBUG): a skipped item means a gap in the sync payload.
            Log.w(TAG, "localItemsForSync: skipping item $id for sync " +
                "(unexpected error): ${e.message}")
            null
        }
    }

    // Combine tombstones + live items, preserving original recency order (reversed).
    (tombstones + localItems).reversed()
}

/**
 * Apply an inbound soft-delete tombstone (from relay, P2P, or cloud) with LWW semantics.
 *
 * Implementation of [ClipboardRepository.applyInboundTombstoneWithLww].
 * See the caller for full documentation.
 */
internal suspend fun ClipboardRepository.applyInboundTombstoneWithLwwImpl(
    itemId: String,
    lamportTs: Long,
): Boolean = withContext(Dispatchers.IO) {
    synchronized(idsWriteLock) {
        // Resolve the local storage id for this cross-device item_id.
        val storageId = prefs.getString("item_id_ref_$itemId", null)
        if (storageId == null) {
            // CopyPaste-bfiu: delete-before-create — insert a ghost tombstone so
            // a later arriving create for this item_id loses LWW. The ghost uses
            // itemId as the storageId (same as storeItemWithLww convention) and is
            // written into the id-index so item_id_ref lookup finds it. The UI
            // filters it out via isDeletedBlob.
            val nowMs = System.currentTimeMillis()
            // Ghost tombstone blob format mirrors encodeTombstone without an existing blob:
            // <nowMs>|text/plain|0||tombstone|<lamportTs>|1|
            val ghostBlob = "$nowMs|text/plain|0||tombstone|$lamportTs|1|"
            val ids = appendUniqueId(storedIds(), itemId)
            ClipboardItemCache.cachedIds = ids
            prefs.edit()
                .putString("item_$itemId", ghostBlob)
                .putString(ClipboardRepository.KEY_ITEM_IDS, ids.joinToString(","))
                .putString("item_id_ref_$itemId", itemId)
                .apply()
            Log.d(TAG, "applyInboundTombstone: inserted ghost tombstone for unknown item_id=$itemId (delete-before-create)")
            return@synchronized true
        }
        val existing = prefs.getString("item_$storageId", null)
            ?: return@synchronized false
        // Already a tombstone — only replace if incoming ts is strictly newer.
        val storedTs = try {
            val parts = existing.split("|")
            if (parts.size >= 6) parts[5].toLongOrNull() ?: 0L else 0L
        } catch (_: Exception) { 0L }
        if (lamportTs <= storedTs) {
            Log.d(TAG, "applyInboundTombstone: skipping (stored=$storedTs >= incoming=$lamportTs) for item_id=$itemId")
            return@synchronized false
        }
        // Write the tombstone at the incoming lamportTs so future LWW comparisons
        // use the remote delete's timestamp (not a local bump of the old ts).
        val tombstone = ClipboardBlobCodec.encodeTombstone(existing, lamportTs)
        val pinnedList = storedPinnedList().toMutableList()
        val wasPinned = pinnedList.remove(storageId)
        val editor = prefs.edit()
            .putString("item_$storageId", tombstone)
            .remove("item_img_$storageId")
            .remove("item_thumb_$storageId")
            .remove("item_file_$storageId")
            .remove("item_filemeta_$storageId")
        if (wasPinned) {
            editor.putString(ClipboardRepository.KEY_PINNED_IDS, pinnedList.joinToString(","))
        }
        editor.apply()
        ClipboardItemCache.evictParseCache(storageId)
        Log.d(TAG, "applyInboundTombstone: tombstoned item_id=$itemId storageId=$storageId (lamport $storedTs→$lamportTs)")
        true
    }
}

/**
 * Export TEXT clipboard items as a JSON string.
 *
 * Implementation of [ClipboardRepository.exportHistory]. See the caller for full docs.
 */
internal suspend fun ClipboardRepository.exportHistoryImpl(
    encryptionKey: ByteArray,
    includeSensitive: Boolean = false,
): String = withContext(Dispatchers.IO) {
    // Load all items (no pagination cap) using the existing getItems path which
    // handles decryption, sensitivity detection, and pinned ordering.
    val allItems = getItems(key = encryptionKey, limit = Int.MAX_VALUE, offset = 0)
    val exportedAt = System.currentTimeMillis()

    val arr = org.json.JSONArray()
    for (item in allItems) {
        // Only export text items.
        if (!item.isText) continue
        // CopyPaste-crh3.40: skip sensitive items unless the user opted in.
        if (item.isSensitive && !includeSensitive) continue

        val fullText = runCatching { loadFullPlaintext(item.id, encryptionKey) }.getOrNull()
            ?: item.snippet

        val obj = org.json.JSONObject()
        obj.put("id", item.id)
        obj.put("content_type", "text")
        obj.put("snippet", item.snippet)
        obj.put("full_text", fullText)
        obj.put("wall_time_ms", item.wallTimeMs)
        obj.put("pinned", item.pinned)
        arr.put(obj)
    }

    val root = org.json.JSONObject()
    root.put("version", 1)
    root.put("exported_at", exportedAt)
    root.put("items", arr)
    root.toString(2) // pretty-print with 2-space indent
}

/**
 * Import items from an export JSON string.
 *
 * Implementation of [ClipboardRepository.importHistory]. See the caller for full docs.
 */
internal suspend fun ClipboardRepository.importHistoryImpl(
    json: String,
    encryptionKey: ByteArray,
    settings: Settings,
): Int = withContext(Dispatchers.IO) {
    val root = org.json.JSONObject(json)
    val version = root.getInt("version")
    require(version == 1) {
        "Unsupported export version $version (expected 1)"
    }
    val arr = root.getJSONArray("items")
    val existingIds = storedIds().toHashSet()
    var imported = 0

    for (i in 0 until arr.length()) {
        val obj = arr.getJSONObject(i)
        val id = obj.getString("id")
        // Skip if already present locally.
        if (id in existingIds) continue

        val fullText = obj.optString("full_text").ifBlank {
            obj.optString("snippet")
        }
        if (fullText.isBlank()) continue

        // CopyPaste-myh8.9: skip file-type items whose declared byte size exceeds
        // Settings.maxFileSizeBytes. Mirrors the "skip without counting" pattern
        // already used above for blank full_text — there is no separate
        // partial/failure counter in this format, so a skip simply does not
        // increment [imported] (the sole outcome value returned to the caller).
        val contentType = obj.optString("content_type", "text")
        if (contentType != "text") {
            val sizeBytes = obj.optLong("size_bytes", fullText.toByteArray(Charsets.UTF_8).size.toLong())
            if (sizeBytes > settings.maxFileSizeBytes) {
                Log.w(
                    TAG,
                    "importHistory: skipping oversized $contentType item $id " +
                        "($sizeBytes B > ${settings.maxFileSizeBytes} B cap)",
                )
                continue
            }
        }

        val wallTimeMs = obj.optLong("wall_time_ms", System.currentTimeMillis())
        val pinned = obj.optBoolean("pinned", false)

        // Encrypt and store via the standard path.
        // Use overrideId = id so the stable cross-device item ID is preserved
        // (allows subsequent syncs to deduplicate correctly via item_id_ref).
        val stored = storeItem(
            plaintext = fullText,
            key = encryptionKey,
            overrideId = id,
            wallTimeMs = wallTimeMs,
        )

        if (stored.isNotBlank() && pinned) {
            // Restore pinned state.
            setPinned(stored, true)
        }

        if (stored.isNotBlank()) {
            imported++
            existingIds += stored // track to avoid re-inserting within the loop
        }
    }
    imported
}
