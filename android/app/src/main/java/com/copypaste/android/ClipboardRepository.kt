package com.copypaste.android

import android.content.Context
import android.content.SharedPreferences

/**
 * Persists clipboard items to SharedPreferences.
 *
 * Each item is stored as a pipe-delimited blob under key "item_<uuid>" so it
 * survives process death without requiring Room or a .so binary.
 * An ordered index of ids is kept under "item_ids" (comma-separated).
 *
 * Encryption is performed via UniFFI [encryptText] (XChaCha20-Poly1305, ADR-001).
 * On [UnsatisfiedLinkError] or [IllegalStateException] (native library absent),
 * the store operation FAILS rather than falling back to [localAesEncrypt]
 * (AES-256-GCM): the fallback produced items that peers and the daemon could not
 * decrypt, causing silent sync failures. A one-shot sentinel notification is posted
 * instead so the user knows encryption is unavailable. [localAesDecrypt] is kept
 * for reading any legacy AES-GCM items that were stored before this fix.
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
 *
 * ## Decomposition (CopyPaste-vp63.33)
 *
 * This class is a thin state-holder + delegator facade. Read/Write-LWW/Delete/
 * binary-sidecar/id-index logic lives in sibling extension files — same pattern
 * as the earlier CopyPaste-ra15.4 Pin/Prune/Sync extraction: see
 * [ClipboardRepositoryRead.kt], [ClipboardRepositoryWrite.kt] (+ the shared
 * fail-closed [encryptOrFailClosed] gate), [ClipboardRepositoryDelete.kt],
 * [ClipboardBinaryStore.kt], [ClipboardRepositoryIndex.kt].
 */
class ClipboardRepository(context: Context) {

    /**
     * Application context retained so the delete path can keep the
     * foreground-service notification counter honest (see [deleteItem]). Using
     * the application context avoids leaking an Activity.
     */
    internal val appContext: Context = context.applicationContext

    internal val prefs: SharedPreferences =
        context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)

    /** Read fresh each store so a UI change to the cap takes effect immediately. */
    internal val settings = Settings(context)

    /**
     * Guard for read-modify-write on the comma-joined "item_ids" index.
     * SharedPreferences is process-wide, so without this lock two coroutines
     * (UI delete + service insert) can both read the same baseline list and
     * the loser's update silently drops the winner's entry. See HIGH-8.
     */
    internal val idsWriteLock = Any()

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
    internal val seenSourceIdsLock = Any()

    /**
     * Set to true the first time a native-library failure is posted as a
     * user-visible notification so we don't flood the notification shade on
     * every store call. Reset on app restart (in-memory only).
     *
     * SECURITY: the native-unavailable path must never silently downgrade to
     * AES-GCM (which produces items peers cannot decrypt). Instead we throw so
     * the item is not stored and post this sentinel notification once.
     */
    @Volatile internal var nativeUnavailableNotified = false

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

    // ── Read path. Full documentation in ClipboardRepositoryRead.kt (CopyPaste-vp63.33). ─

    suspend fun getItems(
        key: ByteArray,
        limit: Int = PAGE_SIZE,
        offset: Int = 0,
    ): List<ClipboardItem> = getItemsImpl(key, limit, offset)

    fun unpinnedItemCount(): Int = unpinnedItemCountImpl()

    fun totalItemCount(): Int = totalItemCountImpl()

    suspend fun loadFullPlaintext(id: String, key: ByteArray): String? = loadFullPlaintextImpl(id, key)

    internal fun loadFullPlaintextBlocking(id: String, key: ByteArray): String? =
        loadFullPlaintextBlockingImpl(id, key)

    suspend fun searchIds(ids: List<String>, query: String, key: ByteArray): Set<String> =
        searchIdsImpl(ids, query, key)

    // ── Binary sidecars. Full documentation in ClipboardBinaryStore.kt (CopyPaste-vp63.33). ─

    fun getImageBytes(id: String): ByteArray? = getImageBytesImpl(id)

    fun getThumbnailBytes(id: String): ByteArray? = getThumbnailBytesImpl(id)

    fun getDisplayImageBytes(id: String): ByteArray? = getDisplayImageBytesImpl(id)

    fun storeThumbnailBytes(id: String, bytes: ByteArray) = storeThumbnailBytesImpl(id, bytes)

    fun getFileBytes(id: String): ByteArray? = getFileBytesImpl(id)

    fun storeFileBytes(id: String, bytes: ByteArray) = storeFileBytesImpl(id, bytes)

    fun getFileMeta(id: String): Pair<String?, String?> = getFileMetaImpl(id)

    fun storeFileMeta(id: String, fileName: String?, mime: String?) = storeFileMetaImpl(id, fileName, mime)

    fun storeImageBytes(id: String, bytes: ByteArray) = storeImageBytesImpl(id, bytes)

    // ── Delete / clear / reset. Full documentation in ClipboardRepositoryDelete.kt (CopyPaste-vp63.33). ─

    suspend fun deleteItem(id: String): Boolean = deleteItemImpl(id)

    fun deleteItems(ids: List<String>) = deleteItemsImpl(ids)

    fun clearAll() = clearAllImpl()

    fun resetDatabase(confirmed: Boolean) = resetDatabaseImpl(confirmed)

    fun clearUnpinned() = clearUnpinnedImpl()

    /**
     * Pin or unpin item [id]. Extracted to ClipboardRepositoryPin.kt (CopyPaste-ra15.4).
     * Pinned items survive the retention prune pass and have no sensitive TTL.
     */
    fun setPinned(id: String, pinned: Boolean) = setPinnedImpl(id, pinned)

    /**
     * Reorder pinned items. Extracted to ClipboardRepositoryPin.kt (CopyPaste-ra15.4).
     * [ids] must contain exactly the currently-pinned item IDs in the desired new order.
     */
    fun reorderPinned(ids: List<String>) = reorderPinnedImpl(ids)

    /**
     * Apply authoritative pin state from an inbound sync row. Extracted to
     * ClipboardRepositoryPin.kt (CopyPaste-ra15.4). Does NOT mint a new local mutation.
     * @param id       The stable item_id.
     * @param pinned   Authoritative pin state from the remote row.
     * @param pinOrder Authoritative pin_order from the remote row (null = no ordering).
     */
    fun applyAuthoritativePinState(id: String, pinned: Boolean, pinOrder: Double?) =
        applyAuthoritativePinStateImpl(id, pinned, pinOrder)

    /**
     * Re-stamp [id] as the most-recently-used item (copy-back). Extracted to
     * ClipboardRepositoryPin.kt (CopyPaste-ra15.4).
     * Returns the new lamport timestamp, or -1L when the item was not found, pinned, or deleted.
     */
    fun bumpToTop(id: String): Long = bumpToTopImpl(id)

    // ── Write / LWW path. Full documentation in ClipboardRepositoryWrite.kt (CopyPaste-vp63.33). ─

    suspend fun storeItem(
        plaintext: String,
        key: ByteArray,
        sourceId: String? = null,
        overrideId: String? = null,
        contentType: String = "text/plain",
        lamportTs: Long = 0L,
        wallTimeMs: Long = System.currentTimeMillis(),
        originDeviceId: String = "",
        sourceApp: String? = null,
    ): String = storeItemImpl(
        plaintext, key, sourceId, overrideId, contentType, lamportTs, wallTimeMs, originDeviceId, sourceApp,
    )

    suspend fun storeItemWithLww(
        plaintext: String,
        key: ByteArray,
        itemId: String,
        incomingLamportTs: Long,
        wallTimeMs: Long = System.currentTimeMillis(),
        originDeviceId: String = "",
    ): Boolean = storeItemWithLwwImpl(plaintext, key, itemId, incomingLamportTs, wallTimeMs, originDeviceId)

    fun lastStoredId(): String? = lastStoredIdImpl()

    fun storedLamportTsForItemId(itemId: String): Long? = storedLamportTsForItemIdImpl(itemId)

    // ── Internal helpers ──────────────────────────────────────────────────────

    /**
     * CopyPaste-iovc: public entry-point so Settings can retroactively apply the
     * history cap immediately after the user changes [Settings.maxHistoryItems]
     * and taps Save — without waiting for the next clipboard capture to call the
     * private [pruneToLimitsImpl] path.
     */
    fun applyHistoryCap() {
        pruneToLimitsImpl()
    }

    /**
     * CopyPaste-bdac.88: Compute how many items reducing the "Maximum stored
     * items" cap to [newMax] would PERMANENTLY tombstone — WITHOUT mutating the
     * store. Used by Settings to populate the confirmation dialog before the
     * destructive [applyHistoryCap] runs.
     *
     * Counts LIVE (non-tombstone) items only; PINNED items are never evicted, so
     * the result is the number of live UNPINNED items that would be removed to
     * bring the live count down to [newMax]. Returns 0 when [newMax] is >= the
     * current live item count — a non-destructive change that needs no
     * confirmation (matching the macOS display-filter, which deletes nothing).
     *
     * Delegates to the shared [planCountCapEvictions] planner so the dialog count
     * and the actual [pruneToLimitsImpl] count-cap pass can never disagree.
     */
    fun countPrunableByMaxItems(newMax: Int): Int =
        synchronized(idsWriteLock) {
            val pinnedSet = storedPinnedIds()
            val liveIds = storedIds().filter { id ->
                val raw = prefs.getString("item_$id", null) ?: return@filter false
                !ClipboardBlobCodec.isDeletedBlob(raw)
            }
            planCountCapEvictions(liveIds, pinnedSet, newMax).size
        }

    /**
     * General retention TTL in seconds. Read from the same "copypaste" prefs file
     * Settings owns (key `general_ttl_secs`) so a future settings UI can drive it;
     * defaults to [DEFAULT_GENERAL_TTL_SECS] (30 days) to mirror the macOS
     * `sync_ttl_secs` retention floor. `0` disables the general age pass.
     */
    internal fun generalTtlSecs(): Long =
        appContext.getSharedPreferences(SETTINGS_PREFS_NAME, Context.MODE_PRIVATE)
            .getLong(KEY_GENERAL_TTL_SECS, DEFAULT_GENERAL_TTL_SECS)
            .coerceAtLeast(0L)

    companion object {

        /**
         * Package names of apps whose clipboard content must always be treated as
         * sensitive (isSensitive=true), regardless of the content-classifier verdict.
         * Defined in [ClipboardBlobCodec.KNOWN_SENSITIVE_PACKAGES]; re-exported here
         * for call-site compatibility (CopyPaste-44rq.48 / PRIV-7).
         */
        val KNOWN_SENSITIVE_PACKAGES: Set<String>
            get() = ClipboardBlobCodec.KNOWN_SENSITIVE_PACKAGES

        /**
         * Compute the next Lamport timestamp — delegates to the package-level
         * [com.copypaste.android.nextLamportTs] extracted in ClipboardRepositoryPlan.kt
         * (CopyPaste-ra15.4). Kept here so callers using [ClipboardRepository.nextLamportTs]
         * are unaffected.
         *
         * `max(prev + 1, now_ms)` — monotonic and wall-clock time-ordered.
         */
        fun nextLamportTs(prevLamport: Long, nowMs: Long): Long =
            com.copypaste.android.nextLamportTs(prevLamport, nowMs)

        /**
         * CopyPaste-bdac.88 / crh3.39 / crh3.108 — PURE count-cap planner.
         *
         * Delegates to the package-level [com.copypaste.android.planCountCapEvictions]
         * extracted in ClipboardRepositoryPlan.kt (CopyPaste-ra15.4). Kept here so
         * callers using [ClipboardRepository.planCountCapEvictions] are unaffected.
         *
         * See ClipboardRepositoryPlan.kt for full documentation.
         */
        internal fun planCountCapEvictions(
            liveIds: List<String>,
            pinned: Set<String>,
            maxItems: Int,
        ): List<String> = com.copypaste.android.planCountCapEvictions(liveIds, pinned, maxItems)

        /**
         * Sync size ceiling in bytes (8 MiB). Delegates to [ClipboardBlobCodec.SYNC_MAX_BLOB_BYTES]
         * -- single source of truth, do not scatter the literal.
         */
        const val SYNC_MAX_BLOB_BYTES: Long = 8L * 1024 * 1024

        /** SharedPreferences file name -- single source of truth, not scattered as string literals. */
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

        /**
         * Default page size for [getItems] pagination.
         * First page = pinned + 50 most-recent unpinned; each subsequent page appends
         * 50 more unpinned rows as the user scrolls near the end of the list.
         */
        const val PAGE_SIZE = 50

        fun normalizeContentTypeForSync(stored: String): String =
            if (stored == "text" || stored.startsWith("text/")) "text" else stored

        const val KEY_ITEM_IDS = "item_ids"
        const val KEY_SYNCED_SOURCE_IDS = "synced_source_ids"
        const val KEY_PINNED_IDS = "pinned_ids"

        const val MAX_SEEN_SOURCE_IDS = 1_000

        // -- Dedup state -- delegates to ClipboardDedupState ------------------
        //
        // Process-wide dedup state extracted to [ClipboardDedupState] (CopyPaste-g06m.20).
        // These forwarding members preserve the public API for callers that use
        // ClipboardRepository.expectClip / shouldSkipExpectedClip / etc.

        /** @see ClipboardDedupState.expectClip */
        fun expectClip(content: String) = ClipboardDedupState.expectClip(content)

        /** @see ClipboardDedupState.shouldSkipExpectedClip */
        fun shouldSkipExpectedClip(content: String): Boolean =
            ClipboardDedupState.shouldSkipExpectedClip(content)

        /** @see ClipboardDedupState.expectImageUri */
        fun expectImageUri(uri: android.net.Uri) = ClipboardDedupState.expectImageUri(uri)

        /** @see ClipboardDedupState.shouldSkipExpectedImageUri */
        fun shouldSkipExpectedImageUri(uri: android.net.Uri): Boolean =
            ClipboardDedupState.shouldSkipExpectedImageUri(uri)

        /**
         * Zero the cross-listener dedup window. Call after [clearAll] so a re-copy
         * of the same text immediately after a clear is stored as a fresh row rather
         * than silently skipped as a recent duplicate.
         * @see ClipboardDedupState.resetDedupState
         */
        fun resetDedupState() = ClipboardDedupState.resetDedupState()

        /** @see ClipboardDedupState.isNewSourceId */
        fun isNewSourceId(sourceId: String, seen: Set<String>): Boolean =
            ClipboardDedupState.isNewSourceId(sourceId, seen)

        // -- Parse cache -- delegates to ClipboardItemCache -------------------

        const val PREVIEW_MAX_CHARS = 140
        const val UNABLE_TO_PREVIEW = "(unable to preview)"

        /**
         * Evict a single id from the parse cache.
         * @see ClipboardItemCache.evictParseCache
         */
        fun evictParseCache(id: String) = ClipboardItemCache.evictParseCache(id)

        /** Evict ALL entries -- call on clearAll / clearUnpinned.
         * @see ClipboardItemCache.evictAllParseCache
         */
        fun evictAllParseCache() = ClipboardItemCache.evictAllParseCache()

        /** @see ClipboardBlobCodec.previewFromPlaintext */
        fun previewFromPlaintext(text: String): String =
            ClipboardBlobCodec.previewFromPlaintext(text)

        /** @see ClipboardBlobCodec.localAesDecrypt */
        fun localAesDecrypt(ciphertext: ByteArray, nonce: ByteArray, key: ByteArray): ByteArray =
            ClipboardBlobCodec.localAesDecrypt(ciphertext, nonce, key)

        /** @see ClipboardBlobCodec.localAesEncrypt */
        fun localAesEncrypt(plaintext: ByteArray, key: ByteArray): EncryptedBlob =
            ClipboardBlobCodec.localAesEncrypt(plaintext, key)
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
    // Extracted to ClipboardRepositorySync.kt (CopyPaste-ra15.4).
    suspend fun localItemsForSync(
        key: ByteArray,
    ): List<uniffi.copypaste_android.LocalItem> = localItemsForSyncImpl(key)

    /**
     * Apply an inbound soft-delete tombstone (from relay, P2P, or cloud) with LWW
     * semantics.
     *
     * Two cases:
     *  1. **Item known locally**: tombstone iff incoming [lamportTs] is STRICTLY
     *     greater than the stored row's lamport_ts (newer remote delete wins; a stale
     *     re-sync cannot resurrect a re-pinned item).
     *  2. **Item unknown locally (delete-before-create)**: insert a ghost tombstone
     *     so that a later arriving create for the same [itemId] loses the LWW
     *     comparison. Mirrors daemon relay.rs `insert_tombstone` ~lines 924-940
     *     (CopyPaste-bfiu). The ghost tombstone is invisible in the UI
     *     (isDeletedBlob → filtered by getItems).
     *
     * If the stored lamport_ts >= [lamportTs] (known-item case) → no-op (local
     * state is at least as new).
     *
     * Returns true when a tombstone was written (for caller stats).
     */
    // Extracted to ClipboardRepositorySync.kt (CopyPaste-ra15.4).
    suspend fun applyInboundTombstoneWithLww(
        itemId: String,
        lamportTs: Long,
    ): Boolean = applyInboundTombstoneWithLwwImpl(itemId, lamportTs)

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

    // ── CopyPaste-8jx8: Export / Import clipboard history ────────────────────
    //
    // Export: produce a JSON file with text items' decrypted snippets and metadata.
    //   - Only TEXT content_type items are exported (binary image/file payloads are
    //     omitted — too large and not portable across devices/encryption keys).
    //   - Sensitive items are skipped to avoid leaking secrets into unencrypted files.
    //   - Pinned state is preserved so the user can round-trip their pinned clips.
    //   - Full plaintext is loaded (not just the snippet) so imports preserve content.
    //
    // Import: read the export JSON and insert each item that does not yet exist locally
    //   (deduplication is by item ID). Items are re-encrypted with the current device key.
    //
    // Format: JSON object { "version": 1, "exported_at": epochMs, "items": [ ... ] }
    //   Each item: { "id", "content_type", "snippet", "full_text", "wall_time_ms", "pinned" }
    //
    // Security:
    //   - The export JSON is PLAINTEXT. The caller (SettingsActivity) must use the
    //     storage-access-framework (SAF / ACTION_CREATE_DOCUMENT) so the user picks
    //     the destination; never auto-write to external storage without SAF.
    //   - Import uses the same storeItem() path so the new items are immediately
    //     encrypted with the device's current key.

    /**
     * Export TEXT clipboard items as a JSON string.
     *
     * Returns the JSON [String] on success. Image and file items are omitted.
     * Sensitive items (flagged by [ClipboardItem.isSensitive]) are omitted unless
     * [includeSensitive] is true — matching the macOS "Include sensitive items"
     * export toggle (CopyPaste-crh3.40). The default is false (safe default: secrets
     * stay out of plaintext export files unless the user explicitly opts in).
     *
     * [encryptionKey] is needed to decrypt stored ciphertext for the full-text field.
     * The returned JSON is plaintext — the caller must write it to a user-chosen
     * location via the Storage Access Framework (ACTION_CREATE_DOCUMENT).
     */
    // Extracted to ClipboardRepositorySync.kt (CopyPaste-ra15.4).
    suspend fun exportHistory(
        encryptionKey: ByteArray,
        includeSensitive: Boolean = false,
    ): String = exportHistoryImpl(encryptionKey, includeSensitive)

    // Extracted to ClipboardRepositorySync.kt (CopyPaste-ra15.4).
    //
    // CopyPaste-myh8.9: [settings] threaded in so file-type items whose declared
    // byte size exceeds [Settings.maxFileSizeBytes] are skipped rather than
    // imported. Today's exportHistoryImpl only ever emits content_type="text"
    // items (binary items are explicitly excluded from export), so this gate is
    // forward-compatible dead code against the current export format — it guards
    // any FUTURE non-text item a differently-produced import JSON might carry.
    suspend fun importHistory(json: String, encryptionKey: ByteArray, settings: Settings): Int =
        importHistoryImpl(json, encryptionKey, settings)
}
