package com.copypaste.android

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.isActive
import kotlinx.coroutines.withContext

/**
 * Supabase catch-up poll drain (CopyPaste-vp63.35), extracted verbatim from
 * `FgsSyncLoop.poll()`.
 *
 * Complements the [SupabaseRealtimeClient] WebSocket push channel: clips arrive
 * primarily via WS in ~1 s, and this drain heals any rows missed while the WS was
 * down (Doze, OEM kills, network flap). See [FgsSyncLoop] for the full cadence
 * doc (WS-primary, poll-as-catch-up).
 *
 * ## Cursor strategy (Tasks 4/5/6)
 * Uses an ascending compound keyset cursor (wall_time, id) that mirrors the
 * macOS daemon's `build_poll_url`. For every row in the batch — including
 * self-echo (own deviceId) rows and blank rows — the cursor is advanced BEFORE
 * any `continue`. This prevents stalling on a batch of own-device rows.
 *
 * ## LWW replace (Task 5)
 * When an incoming row's item_id already exists locally, the incoming
 * lamport_ts is compared to the stored row's. If strictly newer, the local
 * row is replaced (last-writer-wins), mirroring the daemon's cloud.rs LWW.
 */
class SupabaseCatchUpPoller(
    private val settings: Settings,
    private val repository: ClipboardRepository,
    private val syncManager: SyncManager,
    /**
     * Called AT MOST ONCE after a full Supabase catch-up drain with the text of
     * the NEWEST (highest wall_time) text clip that was stored. See
     * [FgsSyncLoop]'s constructor doc for the full contract.
     */
    private val onSyncedTextClip: ((text: String) -> Unit)? = null,
) {
    /**
     * Perform one poll cycle using the compound keyset cursor.
     *
     * For every row in the batch (Tasks 4/5/6):
     *   1. Advance the (wall_time, id) cursor BEFORE any continue — so a batch
     *      of only own-device rows still moves the cursor forward.
     *   2. Skip self-echo rows (own deviceId).
     *   3. Decrypt; skip if decryption fails.
     *   4. Skip blank plaintext.
     *   5. LWW replace: if item_id exists locally with an older lamport_ts,
     *      replace it; otherwise skip as a dup.
     *
     * Returns the number of new/replaced items stored.
     */
    suspend fun poll(): Int = withContext(Dispatchers.IO) {
        // Drain loop: a full batch (size == POLL_LIMIT) almost certainly means
        // the server has more rows waiting. Re-poll IMMEDIATELY in that case
        // instead of returning and waiting the idle delay — otherwise a backlog
        // of N rows would drain at only POLL_LIMIT rows per poll interval
        // (~20/min). On a SHORT batch (< POLL_LIMIT) we have caught up, so we
        // break and let the caller apply the normal idle delay.
        //
        // Each iteration runs the original single-cycle logic unchanged (LWW,
        // compound (wall_time, id) cursor, self-echo skip). The cursor is
        // persisted after every cycle, so a re-poll continues from where the
        // previous cycle left off.
        var totalNewCount = 0
        // Accumulate (text, wallTime) for every text clip stored across ALL
        // batch cycles in this drain. After the full drain, we apply only the
        // NEWEST text clip once — not one per item (which would spam the system
        // clipboard and could re-trigger the capture loop).
        val storedTextClips = mutableListOf<Pair<String, Long>>()
        while (isActive) {
            val batch = syncManager.pollFromSupabase(
                sinceWallTime = settings.lastSupabasePollWallTime,
                sinceId = settings.lastSupabasePollId,
            ) ?: break

            var newCount = 0
            val startWallTime = settings.lastSupabasePollWallTime
            val startId = settings.lastSupabasePollId
            var cursorWallTime = startWallTime
            var cursorId = startId
            // CopyPaste-44rq.36: collect (storedId, imageBytes) pairs during the
            // batch loop; thumbnail generation is deferred to AFTER the loop so the
            // cursor is advanced for ALL items before the CPU-bound decode/compress
            // work starts. Thumbnails are then generated in parallel on Dispatchers.Default.
            val pendingThumbnails = mutableListOf<Pair<String, ByteArray>>()

            for (row in batch.rows) {
                // Task 6: advance cursor for EVERY row before any continue.
                if (row.wallTime > cursorWallTime ||
                    (row.wallTime == cursorWallTime && row.id > cursorId)) {
                    cursorWallTime = row.wallTime
                    cursorId = row.id
                }

                // Skip own-device rows (self-echo from our push).
                if (row.deviceId == settings.deviceId) continue

                // Decrypt; skip rows that fail (wrong key, tampered blob).
                val item = batch.client.decryptRow(row, batch.syncKey) ?: continue

                // CopyPaste-up1c: tombstone fast-path — mirrors daemon cloud.rs ~line 2659.
                // A deleted row carries deleted=true and empty plaintext; route to
                // applyInboundTombstoneWithLww (handles ghost tombstone for delete-before-create).
                if (item.deleted) {
                    val tombstoned = repository.applyInboundTombstoneWithLww(
                        itemId = item.itemId,
                        lamportTs = item.lamportTs,
                    )
                    if (tombstoned) newCount++
                    continue
                }

                val isImage = item.contentType == "image" ||
                    item.contentType.startsWith("image/")
                val isFile = item.contentType == "file"

                val stored = if (isImage) {
                    // Image row: store a placeholder entry then persist raw bytes.
                    // storeItem deduplicates via overrideId so re-polls are no-ops.
                    if (item.plaintext.isEmpty()) {
                        false
                    } else {
                        val storedId = repository.storeItem(
                            plaintext = "[image]",
                            key = settings.encryptionKey,
                            overrideId = item.itemId,
                            contentType = item.contentType,
                            lamportTs = item.lamportTs,
                            originDeviceId = item.deviceId,
                        )
                        if (storedId.isNotEmpty()) {
                            repository.storeImageBytes(storedId, item.plaintext)
                            // CopyPaste-44rq.36: defer thumbnail generation — queue the pair
                            // so all cursors advance before any CPU-bound decode/compress work.
                            pendingThumbnails.add(storedId to item.plaintext)
                            true
                        } else {
                            false
                        }
                    }
                } else if (isFile) {
                    // File row: store actual bytes so the user can save/copy them.
                    // CopyPaste-1jms.35: decryptRow decodes the in-band file-identity
                    // header, so DecryptedItem.fileName/fileMime ARE populated for
                    // cloud-polled files — pass them through (like the relay/P2P path)
                    // so the row shows "[file: report.pdf]" with its real MIME instead
                    // of "[file]" + null metadata.
                    if (item.plaintext.isEmpty()) {
                        false
                    } else {
                        val label = SyncFileHelper.buildFileLabel(item.fileName)
                        val storedId = repository.storeItem(
                            plaintext = label,
                            key = settings.encryptionKey,
                            overrideId = item.itemId,
                            contentType = item.contentType,
                            lamportTs = item.lamportTs,
                            originDeviceId = item.deviceId,
                        )
                        if (storedId.isNotEmpty()) {
                            repository.storeFileBytes(storedId, item.plaintext)
                            repository.storeFileMeta(storedId, item.fileName, item.fileMime)
                            true
                        } else {
                            false
                        }
                    }
                } else {
                    // Text row: LWW replace — replace only when incoming lamport_ts
                    // is strictly newer than the locally stored row for the same item_id.
                    val text = item.plaintext.toString(Charsets.UTF_8)
                    if (text.isBlank()) {
                        false
                    } else {
                        val didStore = repository.storeItemWithLww(
                            plaintext = text,
                            key = settings.encryptionKey,
                            itemId = item.itemId,
                            incomingLamportTs = item.lamportTs,
                            wallTimeMs = item.wallTime,
                            originDeviceId = item.deviceId,
                        )
                        // Track this text clip for the post-drain auto-apply
                        // selection; we only apply the newest one at the end.
                        if (didStore) storedTextClips.add(text to row.wallTime)
                        didStore
                    }
                }

                // lcmq: apply authoritative pin state (pin/unpin/reorder) from cloud row.
                // Uses applyAuthoritativePinState — not setPinned — so authoritative unpins
                // and pin_order convergence work without minting a new local mutation.
                if (stored) {
                    repository.applyAuthoritativePinState(item.itemId, item.pinned, item.pinOrder)
                }

                if (stored) newCount++
            }

            // Persist the advanced cursor after processing the full batch.
            // advanceSupabaseCursor is monotonic and holds supabaseCursorLock so
            // a concurrent SupabasePollWorker run cannot interleave and lose an advance.
            settings.advanceSupabaseCursor(cursorWallTime, cursorId)

            // CopyPaste-44rq.36: generate thumbnails for all images in this batch in
            // parallel AFTER the cursor is advanced. Cursor advancement is the critical
            // path; thumbnail generation (50–200 ms per image) is not.
            if (pendingThumbnails.isNotEmpty()) {
                coroutineScope {
                    pendingThumbnails.map { (storedId, imageBytes) ->
                        async(Dispatchers.Default) {
                            SyncThumbnailHelper.generateAndStore(imageBytes) { thumbBytes ->
                                repository.storeThumbnailBytes(storedId, thumbBytes)
                            }
                        }
                    }.awaitAll()
                }
                pendingThumbnails.clear()
            }

            totalNewCount += newCount

            // Short batch → caught up. Stop draining and return.
            if (batch.rows.size < SupabaseClient.POLL_LIMIT) break

            // Safety: if a full batch somehow failed to advance the cursor,
            // break rather than spin forever re-fetching the same window.
            if (cursorWallTime == startWallTime && cursorId == startId) break
        }

        // Auto-apply: after the full drain, apply only the NEWEST text clip once.
        // This prevents N clipboard overwrites for a batch of N items and avoids
        // re-triggering the capture loop for intermediate clips.
        SyncLoopPolicy.newestTextClip(storedTextClips)?.let { text ->
            onSyncedTextClip?.invoke(text)
        }

        totalNewCount
    }
}
