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
     *
     * This is the glue that lets the UI ViewModel re-load the history the moment
     * a clip is captured in the BACKGROUND (the primary capture path on Android
     * 10+ via [ClipboardAccessibilityService]) — previously the list only
     * refreshed on first composition or a manual Refresh tap, so background
     * captures were stored but never appeared until the user forced a reload.
     *
     * The caller MUST retain the listener (SharedPreferences holds only a weak
     * reference) and unsubscribe via [stopObserving].
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
     * Each stored blob is DECRYPTED with [key] (same key used at store time) so
     * the row shows a real, truncated single-line preview of the clip — not the
     * old "(N chars)" placeholder (bug Ac). The decryption happens here on
     * [Dispatchers.IO]; sensitivity is re-evaluated against the decrypted text so
     * the UI can mask correctly.
     *
     * If a blob cannot be decrypted (e.g. it was written by the local AES-GCM
     * fallback while the .so was absent and we cannot read it back, or the key
     * rotated) the row falls back to a neutral "(unable to preview)" label — we
     * never surface ciphertext and never crash.
     */
    suspend fun getItems(key: ByteArray, limit: Int = 50): List<ClipboardItem> =
        withContext(Dispatchers.IO) {
            val ids = storedIds().takeLast(limit)
            ids.mapNotNull { id ->
                val raw = prefs.getString("item_$id", null) ?: return@mapNotNull null
                parseItem(id, raw, key)
            }.reversed()
        }

    suspend fun deleteItem(id: String): Boolean = withContext(Dispatchers.IO) {
        synchronized(idsWriteLock) {
            val ids = storedIds().toMutableList()
            if (!ids.remove(id)) return@synchronized false
            prefs.edit()
                .remove("item_$id")
                .putString(KEY_ITEM_IDS, ids.joinToString(","))
                .apply()
            true
        }
    }

    /**
     * Encrypt [plaintext] with [key] and persist, returning the STABLE row id of
     * the stored item — or an empty string when nothing was stored (blank text,
     * sensitive content, a recent local duplicate, or — for synced items —
     * already stored under the same [sourceId]).
     *
     * The id is the stable cross-device `item_id` for this clip: it is minted
     * ONCE here (or carried in via [overrideId] for an incoming synced item),
     * persisted as the SharedPreferences storage key, bound into the local AEAD
     * AAD (see [encryptText]), and reused verbatim on every later Supabase push
     * and P2P sync. It must NEVER be re-minted per push — re-minting made the
     * desktop see each logical clip as a brand-new item (duplicates + broken
     * LWW). See [localItemsForSync], which sets `LocalItem.item_id = id`.
     *
     * [overrideId] is the stable cross-device `item_id` of an INCOMING synced
     * item (Supabase `item_id` / P2P `SyncedItem.item_id`). Persisting the row
     * under that id means a later re-sync of the same clip reuses it instead of
     * minting a fresh local UUID — so the item is not resurfaced as a duplicate
     * on the originating device. For locally captured clips it is null and a
     * fresh UUID is minted. When supplied it is also the de-facto source id.
     *
     * [sourceId] is the STABLE remote identifier of an incoming synced item used
     * for cross-poll dedup (defaults to [overrideId] when that is set). See
     * LOW-2: [FgsSyncLoop.poll] and [SupabasePollWorker.doWork] share the
     * `lastSupabasePollWallTime` cursor, so the same remote row can be fetched by
     * both within the same wall-time bucket. The 2 s content [DEDUP_WINDOW_MS]
     * only catches near-simultaneous stores; rows fetched > 2 s apart slipped
     * through and produced a duplicate. Recording the source id closes that gap
     * regardless of timing.
     */
    suspend fun storeItem(
        plaintext: String,
        key: ByteArray,
        sourceId: String? = null,
        overrideId: String? = null,
    ): String = withContext(Dispatchers.IO) {
        if (plaintext.isBlank()) return@withContext ""

        // The id that dedup keys on: an explicit [sourceId] wins; otherwise the
        // incoming [overrideId] (which IS the stable remote id) is the source id.
        val dedupSourceId = sourceId ?: overrideId

        // ── LOW-2: source-id dedup for incoming synced items. Both poll callers
        // can fetch the same remote row (shared wall-time cursor), so content/
        // time dedup alone misses copies fetched > DEDUP_WINDOW_MS apart. Skip
        // when this remote source id was already stored; record it atomically.
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

        // ── HIGH-3: cross-listener dedup. The same physical copy fires the
        // clip-changed listener in every owner (FGS, a11y service, activity).
        // Skip if identical content was stored within the recent window so a
        // single copy yields a single row, while a later re-copy still stores.
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
                .putString(KEY_ITEM_IDS, ids.joinToString(","))
                // Reverse-lookup: item_id → storage_id for LWW cloud sync.
                // For locally-captured items the storage id IS the item_id.
                .putString("item_id_ref_$id", id)
                .apply()
        }

        Log.d(TAG, "Stored item $id (${plaintext.length} chars)")
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

        // Pre-compute a candidate blob for the REPLACE case outside the lock.
        // We may not use it (if the item doesn't exist or the lamport check
        // fails), but we need it ready before we enter the lock.
        val plaintextBytes = plaintext.toByteArray(Charsets.UTF_8)

        // We cannot know the storageId until we're inside the lock, so we
        // encrypt with a sentinel and re-encrypt with the real id if needed.
        // Instead, defer the encrypt until we hold the lock (the encrypt call
        // is fast relative to the lock hold time and does not call back into
        // any locked method).
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
            val encoded = encodeItem(blob, plaintext.length, incomingLamportTs)
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
                .putString(KEY_ITEM_IDS, ids.joinToString(","))
                .putString("item_id_ref_$storageId", storageId)
                .apply()
        }
        Log.d(TAG, "storeItemWithLww: stored new item_id=$itemId as storageId=$storageId")
        true
    }

    // ── Internal helpers ──────────────────────────────────────────────────────

    private fun storedIds(): List<String> =
        prefs.getString(KEY_ITEM_IDS, "")
            ?.split(",")
            ?.filter { it.isNotBlank() }
            ?: emptyList()

    /**
     * The set of remote source ids already stored locally (LOW-2). Persisted as
     * a comma-joined string under [KEY_SYNCED_SOURCE_IDS] so dedup survives
     * process death — the WorkManager worker runs in a fresh process and must
     * still see ids stored by an earlier FGS-loop poll.
     */
    private fun storedSourceIds(): LinkedHashSet<String> =
        LinkedHashSet(
            prefs.getString(KEY_SYNCED_SOURCE_IDS, "")
                ?.split(",")
                ?.filter { it.isNotBlank() }
                ?: emptyList()
        )

    /**
     * Append [sourceId] to the persisted seen-set [seen] (already known to lack
     * it), trimming oldest-first to [MAX_SEEN_SOURCE_IDS] so the prefs string
     * cannot grow unbounded. Insertion order is preserved by [LinkedHashSet].
     */
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

    private fun parseItem(id: String, raw: String, key: ByteArray): ClipboardItem? {
        val parts = try {
            raw.split("|")
        } catch (e: Exception) {
            Log.w(TAG, "Failed to parse item $id: ${e.message}")
            return null
        }
        // Required structural fields. A malformed row (missing pieces) is dropped
        // rather than rendered, so the index never shows a half-decoded entry.
        val wallTimeMs = parts.getOrNull(0)?.toLongOrNull() ?: return null
        val contentType = parts.getOrNull(1) ?: return null
        val nonceB64 = parts.getOrNull(3)
        val ctB64 = parts.getOrNull(4)

        // Decrypt for a real preview. Try UniFFI first (the normal store path),
        // then the local AES-GCM fallback (used when the .so was absent at store
        // time). Either failure yields a neutral, non-crashing label.
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

        val snippet = if (plaintext == null) {
            UNABLE_TO_PREVIEW
        } else {
            previewFromPlaintext(plaintext)
        }

        return ClipboardItem(
            id = id,
            contentType = contentType,
            isSensitive = sensitive,
            wallTimeMs = wallTimeMs,
            snippet = snippet,
        )
    }

    /**
     * Decrypt a stored blob into UTF-8 plaintext for preview rendering.
     *
     * The blob may have been produced by either the UniFFI XChaCha20 path
     * ([encryptText]) or the Kotlin AES-256-GCM fallback ([localAesEncrypt]).
     * The two are not interchangeable, so try UniFFI first and on any failure
     * fall back to local AES-GCM. Throws if neither can read the blob.
     */
    private fun decryptForPreview(
        id: String,
        ciphertext: ByteArray,
        nonce: ByteArray,
        key: ByteArray,
    ): String {
        val bytes = try {
            decryptText(id, ciphertext, nonce, key)
        } catch (_: Exception) {
            // Native unavailable or wrong AEAD scheme for this blob — the item
            // may have been stored via the local AES-GCM fallback.
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
         * Map a stored MIME-style content type to the canonical token the Rust
         * FFI send path (`sync_with_peer`) accepts when re-keying items for
         * peers. Stored text items use "text/plain"; the FFI only re-keys items
         * whose content type is exactly "text", so any "text/<any>" value must be
         * collapsed to "text" at the sync boundary or the item is silently
         * dropped (items_sent = 0). Non-text types pass through unchanged.
         */
        fun normalizeContentTypeForSync(stored: String): String =
            if (stored == "text" || stored.startsWith("text/")) "text" else stored

        /**
         * SharedPreferences key holding the comma-joined ordered index of item
         * ids. Public so observers (e.g. [ClipboardViewModel]) can filter the
         * OnSharedPreferenceChangeListener callback to just the index mutations
         * that signal an add/delete — every store/delete rewrites this key.
         */
        const val KEY_ITEM_IDS = "item_ids"

        /**
         * SharedPreferences key holding the comma-joined set of remote source
         * ids (Supabase `item_id` / P2P `SyncedItem.id`) already stored locally,
         * used for LOW-2 cross-poll dedup of incoming synced items.
         */
        const val KEY_SYNCED_SOURCE_IDS = "synced_source_ids"

        /**
         * Upper bound on the persisted source-id seen-set. Oldest ids are
         * dropped first once exceeded. Comfortably larger than any realistic
         * sync backlog, so a re-fetched row is still recognised, while the prefs
         * string stays bounded.
         */
        const val MAX_SEEN_SOURCE_IDS = 1_000

        /** Window in which an identical-content store is treated as a duplicate. */
        private const val DEDUP_WINDOW_MS = 2_000L

        /**
         * Pure LOW-2 dedup predicate: an incoming synced item is new (should be
         * stored) iff its remote [sourceId] is not already in [seen]. Extracted
         * with no Android deps so it is unit-testable on the host JVM.
         */
        fun isNewSourceId(sourceId: String, seen: Set<String>): Boolean =
            sourceId !in seen

        /** Max characters shown in a history-row preview before ellipsizing. */
        const val PREVIEW_MAX_CHARS = 140

        /** Neutral label shown when a stored blob cannot be decrypted. */
        const val UNABLE_TO_PREVIEW = "(unable to preview)"

        private const val AES_TRANSFORMATION = "AES/GCM/NoPadding"
        private const val GCM_TAG_BITS = 128
        private const val GCM_NONCE_BYTES = 12

        /**
         * Build a safe, human-readable history preview from decrypted [text].
         *
         * Pure (no Android / native deps) so it is unit-testable on the host JVM.
         * Collapses all runs of whitespace (incl. newlines/tabs) to single spaces,
         * trims, and caps the result at [PREVIEW_MAX_CHARS], appending an ellipsis
         * when truncated. Blank/whitespace-only input yields an empty string, which
         * the UI renders as its "empty" placeholder.
         */
        fun previewFromPlaintext(text: String): String {
            val collapsed = text.replace(Regex("\\s+"), " ").trim()
            if (collapsed.isEmpty()) return ""
            return if (collapsed.length > PREVIEW_MAX_CHARS) {
                collapsed.take(PREVIEW_MAX_CHARS).trimEnd() + "…"
            } else {
                collapsed
            }
        }

        /**
         * Counterpart to [localAesEncrypt]: AES-256-GCM decryption using only
         * javax.crypto. Reads back blobs produced by the local fallback when the
         * UniFFI .so was unavailable at store time. Throws on auth-tag mismatch
         * (wrong key) — the caller treats that as "unable to preview".
         */
        fun localAesDecrypt(ciphertext: ByteArray, nonce: ByteArray, key: ByteArray): ByteArray {
            val cipher = Cipher.getInstance(AES_TRANSFORMATION)
            cipher.init(
                Cipher.DECRYPT_MODE,
                SecretKeySpec(key.copyOf(32), "AES"),
                GCMParameterSpec(GCM_TAG_BITS, nonce)
            )
            return cipher.doFinal(ciphertext)
        }

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
                // Normalize the stored MIME-style content type ("text/plain",
                // "text/<any>") to the canonical "text" token the FFI send path
                // (`sync_with_peer`) re-keys and offers to peers. Without this
                // mapping every Android item is filtered out and ZERO items are
                // sent. We only normalize at the sync boundary; the on-disk
                // value is left untouched.
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
     *
     * This function was the incoming side of the relay cloud backend. It called
     * [SyncManager.syncIncoming], which fetched items encrypted with the sender's
     * local per-device AES key — a key no other device holds, making every fetched
     * payload undecryptable. Additionally, nothing in any active code path ever
     * called this function; the relay backend was write-only from day one.
     *
     * Decision: cloud sync = Supabase only. This function is intentionally never
     * called. Use [FgsSyncLoop.poll] (via [SyncManager.pollFromSupabase]) for
     * incoming cloud sync.
     *
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
