package com.copypaste.android

import android.util.Log
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import java.util.UUID

/**
 * Write / Last-Writer-Wins path for [ClipboardRepository]: capture insert
 * ([storeItemImpl]), sync-in replace ([storeItemWithLwwImpl]), and the shared
 * fail-closed encrypt gate ([encryptOrFailClosed]).
 *
 * Extracted from [ClipboardRepository] (CopyPaste-vp63.33). These extension functions
 * access [ClipboardRepository]'s internal fields via the extension receiver and call
 * [ClipboardBlobCodec] / [ClipboardItemCache] / [ClipboardDedupState] directly
 * (bypassing the removed private-wrapper aliases), mirroring the existing extraction
 * pattern from [ClipboardRepositoryPin] / [ClipboardRepositoryPrune] /
 * [ClipboardRepositorySync] (CopyPaste-ra15.4).
 */

private const val TAG = "ClipboardRepository"

/**
 * FAIL-CLOSED encrypt gate (ADR-001 / Android fail-closed rule) shared by
 * [storeItemImpl] and [storeItemWithLwwImpl].
 *
 * On [IllegalStateException] or [UnsatisfiedLinkError] (native library absent),
 * logs via `"$callerTag: native encryption unavailable (...) — $actionDescription"`,
 * posts the one-shot [NotificationHelper.notifyNativeUnavailable] sentinel (via
 * [notifyNativeUnavailableOnce]), and ALWAYS throws [IllegalStateException] — never
 * falls back to [ClipboardBlobCodec.localAesEncrypt] (AES-256-GCM), which would
 * produce items peers cannot decrypt.
 *
 * Callers that must SKIP (not propagate) the failure — [storeItemWithLwwImpl]'s two
 * call sites — wrap the call in `try { } catch (e: IllegalStateException) { ... }`.
 * [storeItemImpl] does not catch, so the exception propagates exactly as before.
 */
internal fun ClipboardRepository.encryptOrFailClosed(
    id: String,
    bytes: ByteArray,
    key: ByteArray,
    keyVersion: UByte,
    callerTag: String,
    actionDescription: String,
): EncryptedBlob {
    try {
        return encryptText(id, bytes, key, keyVersion)
    } catch (e: IllegalStateException) {
        Log.e(TAG, "$callerTag: native encryption unavailable (${e.message}) — $actionDescription")
        notifyNativeUnavailableOnce()
        throw e
    } catch (e: UnsatisfiedLinkError) {
        Log.e(TAG, "$callerTag: native encryption unavailable (UnsatisfiedLinkError) — $actionDescription")
        notifyNativeUnavailableOnce()
        throw IllegalStateException("UnsatisfiedLinkError: ${e.message}", e)
    }
}

/**
 * Post the native-unavailable sentinel notification exactly once per process
 * (mirrors the original inline `if (!nativeUnavailableNotified) { ... }` guard
 * duplicated at every call site before this extraction).
 */
private fun ClipboardRepository.notifyNativeUnavailableOnce() {
    if (!nativeUnavailableNotified) {
        nativeUnavailableNotified = true
        NotificationHelper.notifyNativeUnavailable(appContext)
    }
}

/**
 * Implementation of [ClipboardRepository.storeItem].
 *
 * Encrypt [plaintext] with [key] and persist, returning the STABLE row id of
 * the stored item — or an empty string when nothing was stored (blank text,
 * oversized text, sensitive content, a recent local duplicate, or — for synced
 * items — already stored under the same [sourceId]).
 *
 * After inserting, calls [pruneToLimitsImpl] to enforce the storage-quota cap
 * (SIZE only — no count cap).
 *
 * [sourceApp] is the package name of the app that set the clipboard (e.g.
 * "com.agilebits.onepassword"). When non-null and present in
 * [ClipboardBlobCodec.KNOWN_SENSITIVE_PACKAGES], the item is stored with isSensitive forced to
 * true at read time (parseItem), regardless of the content classifier verdict.
 * Conservative: only ever overrides sensitivity to TRUE, never false.
 */
internal suspend fun ClipboardRepository.storeItemImpl(
    plaintext: String,
    key: ByteArray,
    sourceId: String? = null,
    overrideId: String? = null,
    contentType: String = "text/plain",
    lamportTs: Long = 0L,
    wallTimeMs: Long = System.currentTimeMillis(),
    originDeviceId: String = "",
    sourceApp: String? = null,
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
            if (!ClipboardDedupState.isNewSourceId(dedupSourceId, seen)) {
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
    synchronized(ClipboardDedupState.dedupLock) {
        val now = System.currentTimeMillis()
        if (dedupKey == ClipboardDedupState.lastStoredKey && now - ClipboardDedupState.lastStoredAtMs < ClipboardDedupState.DEDUP_WINDOW_MS) {
            Log.d(TAG, "Duplicate clip within ${ClipboardDedupState.DEDUP_WINDOW_MS}ms — skipping")
            return@withContext ""
        }
        ClipboardDedupState.lastStoredKey = dedupKey
        ClipboardDedupState.lastStoredAtMs = now
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
    // key_version=2 matches the daemon's ITEM_KEY_VERSION_CURRENT (AAD "{id}|4|2").
    // This makes Android-stored items decryptable on the daemon side and vice versa.
    val keyVersion: UByte = 2u
    // SECURITY: do NOT fall back to localAesEncrypt (AES-256-GCM) on FFI failure.
    // AES-GCM items use a different key derivation/AAD format that peers (daemon,
    // other Android devices) cannot decrypt — storing them produces items that silently
    // fail sync with no user-visible error.  Instead, propagate the failure so the
    // caller skips this store, and post a one-shot sentinel notification so the user
    // knows something is wrong.
    val blob = encryptOrFailClosed(
        id, textBytes, key, keyVersion,
        callerTag = "storeItem",
        actionDescription = "item NOT stored to avoid producing AES-GCM items that peers cannot decrypt",
    )

    val encoded = ClipboardBlobCodec.encodeItem(blob, textBytes.size, contentType = contentType, lamportTs = lamportTs, wallTimeMs = wallTimeMs, originDeviceId = originDeviceId, keyVersion = keyVersion, sourceApp = sourceApp)
    synchronized(idsWriteLock) {
        // Append the id, removing any prior occurrence first so the index
        // stays canonical (no duplicate ids). A synced item re-stored under
        // the same overrideId — e.g. after clearUnpinned wiped the
        // synced-source-id seen-set while a pinned id stayed in the index —
        // would otherwise append a second copy of the same id, which then
        // crashes the history LazyColumn ("Key … was already used").
        val ids = appendUniqueId(storedIds(), id)
        ClipboardItemCache.cachedIds = ids
        prefs.edit()
            .putString("item_$id", encoded)
            .putString(ClipboardRepository.KEY_ITEM_IDS, ids.joinToString(","))
            // Reverse-lookup: item_id → storage_id for LWW cloud sync.
            // For locally-captured items the storage id IS the item_id.
            .putString("item_id_ref_$id", id)
            .apply()
    }

    Log.d(TAG, "Stored item $id (${textBytes.size} bytes, contentType=$contentType)")

    // Prune to size-only quota after insert.
    pruneToLimitsImpl()
    id
}

/**
 * Implementation of [ClipboardRepository.storeItemWithLww].
 *
 * Store a cloud-synced item with Last-Writer-Wins semantics (Task 5).
 *
 * [itemId] is the stable UUID from the `item_id` column (same across devices).
 * [incomingLamportTs] is the lamport_ts from the cloud row (Unix-ms on both
 * sides, so the compare is valid cross-platform).
 *
 * Behaviour:
 * - If [itemId] is not yet stored locally → store as a new item (same as
 *   [storeItemImpl]).
 * - If [itemId] already exists locally AND [incomingLamportTs] is strictly
 *   greater than the stored lamport_ts → replace the stored row in-place
 *   (re-encrypt with [key], keep the same storage id in the index).
 * - Otherwise (equal or older lamport_ts) → skip as a dup.
 *
 * Returns true when a new row was inserted or an existing row was replaced.
 */
internal suspend fun ClipboardRepository.storeItemWithLwwImpl(
    plaintext: String,
    key: ByteArray,
    itemId: String,
    incomingLamportTs: Long,
    wallTimeMs: Long = System.currentTimeMillis(),
    originDeviceId: String = "",
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

        // LWW: apply the SAME total order as remote_wins() in
        // copypaste-sync/src/merge.rs ~lines 97-112:
        //   1. lamport_ts — larger wins.
        //   2. wall_time  — larger wins (tie-break on equal lamport).
        //   3. origin_device_id — lexicographically larger wins (deterministic).
        // CopyPaste-up1c: previously only lamport_ts was compared; the wall_time
        // + origin_device_id tie-break was missing, causing non-deterministic
        // conflict resolution on simultaneous edits.
        // Read the full raw blob once so we can extract both lamport_ts (field 5),
        // wall_time (field 0), and origin_device_id (field 7) for the 3-key LWW
        // without double-reading prefs.
        val storedRaw = prefs.getString("item_$existingStorageId", null)
        val storedParts = storedRaw?.split("|")
        val storedTs = storedParts?.getOrNull(5)?.toLongOrNull() ?: 0L
        val remoteWins = when {
            incomingLamportTs > storedTs -> true
            incomingLamportTs < storedTs -> false
            else -> {
                // Equal lamport — compare wall_time then origin_device_id.
                // Mirrors remote_wins() in copypaste-sync/src/merge.rs ~lines 106-109.
                val storedWall = storedParts?.getOrNull(0)?.toLongOrNull() ?: 0L
                val storedOrigin = storedParts?.getOrNull(7) ?: ""
                when {
                    wallTimeMs > storedWall -> true
                    wallTimeMs < storedWall -> false
                    else -> originDeviceId > storedOrigin
                }
            }
        }
        if (!remoteWins) {
            Log.d(TAG, "LWW: skipping dup item_id=$itemId (stored=$storedTs, incoming=$incomingLamportTs)")
            return@synchronized null  // null = "skip, do not store as new item either"
        }

        // Replace in-place: re-encrypt and overwrite the stored blob.
        // key_version=2 matches the daemon's ITEM_KEY_VERSION_CURRENT.
        val lwwKeyVersion: UByte = 2u
        // SECURITY: same fail-closed rule as storeItem — do NOT fall back to
        // AES-GCM on FFI failure. Propagate so the LWW replace is skipped.
        val blob = try {
            encryptOrFailClosed(
                existingStorageId, plaintextBytes, key, lwwKeyVersion,
                callerTag = "LWW replace",
                actionDescription = "skipping replace to avoid producing AES-GCM items that peers cannot decrypt",
            )
        } catch (e: IllegalStateException) {
            return@synchronized null  // null → skip, do not attempt new-item insert
        }
        val encoded = ClipboardBlobCodec.encodeItem(blob, plaintextBytes.size, lamportTs = incomingLamportTs, wallTimeMs = wallTimeMs, originDeviceId = originDeviceId, keyVersion = lwwKeyVersion)
        prefs.edit().putString("item_$existingStorageId", encoded).apply()
        ClipboardItemCache.evictParseCache(existingStorageId) // A: blob changed — evict stale decrypt entry
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
            // above, so pruneToLimitsImpl() (which takes idsWriteLock) cannot deadlock.
            pruneToLimitsImpl()
            return@withContext true
        }
        else -> { /* false: fall through to new-item insert below */ }
    }

    // New item: generate a fresh storage id and store normally.
    // key_version=2 matches the daemon's ITEM_KEY_VERSION_CURRENT.
    val newKeyVersion: UByte = 2u
    val storageId = itemId // Use the stable item_id as the storage key for easy lookup.
    // SECURITY: same fail-closed rule — do NOT fall back to AES-GCM on FFI failure.
    val blob = try {
        encryptOrFailClosed(
            storageId, plaintextBytes, key, newKeyVersion,
            callerTag = "storeItemWithLww",
            actionDescription = "skipping new-item insert to avoid producing AES-GCM items that peers cannot decrypt",
        )
    } catch (e: IllegalStateException) {
        return@withContext false
    }
    val encoded = ClipboardBlobCodec.encodeItem(blob, plaintextBytes.size, lamportTs = incomingLamportTs, wallTimeMs = wallTimeMs, originDeviceId = originDeviceId, keyVersion = newKeyVersion)

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
        ClipboardItemCache.cachedIds = ids
        prefs.edit()
            .putString("item_$storageId", encoded)
            .putString(ClipboardRepository.KEY_ITEM_IDS, ids.joinToString(","))
            .putString("item_id_ref_$storageId", storageId)
            .apply()
    }
    Log.d(TAG, "storeItemWithLww: stored new item_id=$itemId as storageId=$storageId")
    pruneToLimitsImpl()
    true
}

/**
 * Implementation of [ClipboardRepository.lastStoredId].
 *
 * Return the id of the most recently stored item, or null when the index is
 * empty. Used by image capture callers that need the id that [storeItemImpl] just
 * wrote so they can call [ClipboardRepository.storeImageBytes] under the same key.
 *
 * Safe to call immediately after [ClipboardRepository.storeItem] returns true because storeItem
 * appends the new id at the END of the comma-joined index before returning.
 * The caller runs on [Dispatchers.IO] and storeItem holds [ClipboardRepository.idsWriteLock] for
 * the entire append, so by the time storeItem returns the id is visible here.
 */
internal fun ClipboardRepository.lastStoredIdImpl(): String? = storedIds().lastOrNull()

/**
 * Implementation of [ClipboardRepository.storedLamportTsForItemId].
 *
 * CopyPaste-vg4r: return the stored lamport_ts for a stable [itemId] (the relay/cloud
 * item_id, not a local storage id), or null when the item does not exist locally.
 *
 * Used by binary ingest paths (image, file) in [SyncManager.ingestRelaySseItem] to
 * apply LWW ordering without going through [ClipboardRepository.storeItemWithLww] (which is text-only).
 * The caller compares the incoming lamport_ts against the stored one before
 * deciding whether to overwrite:
 *   - incoming > stored → overwrite (new version wins)
 *   - incoming <= stored → skip (local version is current or newer)
 *
 * Thread-safe: reads are protected by the [ClipboardRepository.idsWriteLock] monitor (same lock that
 * [storeItemImpl] / [storeItemWithLwwImpl] hold during their read-decide-write sequences).
 */
internal fun ClipboardRepository.storedLamportTsForItemIdImpl(itemId: String): Long? {
    val storageId = synchronized(idsWriteLock) {
        prefs.getString("item_id_ref_$itemId", null)
    } ?: return null
    val raw = prefs.getString("item_$storageId", null) ?: return null
    // Blob format: <wallTimeMs>|<contentType>|<payloadBytes>|<nonceB64>|<ciphertextB64>|<lamportTs>|…
    // lamportTs is field index 5.
    return raw.split("|").getOrNull(5)?.toLongOrNull()
}
