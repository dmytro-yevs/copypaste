package com.copypaste.android

import android.util.Log
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/**
 * Store one [uniffi.copypaste_android.SyncedItem] received over P2P, mapping it to
 * the right local storage path by content type. Shared by BOTH the Android→macOS
 * dialer ([P2pDialer.dialPairedPeer], via [FgsSyncLoop.storeSyncedItem]) and the
 * macOS→Android inbound listener poll ([ClipboardService] → `pollP2pListener`).
 *
 * Extracted from `FgsSyncLoop.storeSyncedItem` verbatim (CopyPaste-vp63.35) so the
 * mapping logic lives in exactly one place regardless of which caller received the
 * item.
 *
 * Persists under the peer's STABLE item_id ([uniffi.copypaste_android.SyncedItem.itemId])
 * as `overrideId`, so a re-dial or a re-receipt from the listener is deduped by
 * [ClipboardRepository] (LWW on item_id) — no extra cross-path dedup needed.
 *
 * Advances the local Lamport clock past every received item (mirrors the Supabase
 * path) so future local pushes order correctly under LWW.
 */
class SyncedItemStore(
    private val settings: Settings,
    private val repository: ClipboardRepository,
    /**
     * Supplies the scope used to launch fire-and-forget thumbnail generation
     * tasks so they are cancelled when the FGS is destroyed. Returns null before
     * [FgsSyncLoop.start] runs (unit tests, stub mode) — falls back to an ad-hoc
     * scope so callers that invoke [store] directly do not crash.
     */
    private val scopeProvider: () -> CoroutineScope?,
) {
    /**
     * Returns true when a new (or replaced) row was stored, false on a dedup /
     * empty / blank no-op.
     */
    suspend fun store(item: uniffi.copypaste_android.SyncedItem): Boolean =
        withContext(Dispatchers.IO) {
            // Advance the local Lamport clock to stay causally after every received
            // item — without this the local clock lags behind the peer's, making
            // future local pushes appear "older" and breaking LWW ordering.
            settings.lamportClock.observe(item.wallTimeMs)

            // ABI 15: tombstone frame — apply via LWW so a newer remote delete wins
            // and a stale re-sync cannot resurrect a live item.
            if (item.deleted) {
                val tombstoned = repository.applyInboundTombstoneWithLww(
                    itemId = item.itemId,
                    lamportTs = item.wallTimeMs,
                )
                if (tombstoned) {
                    Log.d(TAG, "P2P: applied inbound tombstone for itemId=${item.itemId.take(8)}…")
                }
                return@withContext tombstoned
            }

            val key = settings.encryptionKey

            // UniFFI maps `sequence<u8>` to List<UByte>; storeImageBytes and the
            // UTF-8 text decode below both want a ByteArray.
            val plaintextBytes = ByteArray(item.plaintext.size) { item.plaintext[it].toByte() }

            val stored = when {
                contentTypeIsImage(item.contentType) -> {
                    // Image frame: store a placeholder row under the peer's STABLE
                    // item_id, then persist the raw image bytes so HistoryActivity
                    // can render them. Re-dials dedup via overrideId.
                    if (plaintextBytes.isEmpty()) {
                        false
                    } else {
                        val storedId = repository.storeItem(
                            plaintext = "[image]",
                            key = key,
                            overrideId = item.itemId,
                            contentType = item.contentType,
                        )
                        if (storedId.isNotEmpty()) {
                            repository.storeImageBytes(storedId, plaintextBytes)
                            // CopyPaste-44rq.36: fire-and-forget thumbnail generation on
                            // Dispatchers.Default so the P2P sync result is returned
                            // immediately and the next item can be processed without
                            // waiting 50–200 ms for the decode/compress step.
                            val capturedId = storedId
                            val capturedBytes = plaintextBytes
                            // Use the FGS-bound scope so this task is cancelled when the
                            // service is destroyed; fall back to an ad-hoc scope only in
                            // unit tests where start() was never called.
                            (scopeProvider() ?: CoroutineScope(Dispatchers.Default)).launch(Dispatchers.Default) {
                                SyncThumbnailHelper.generateAndStore(capturedBytes) { thumbBytes ->
                                    repository.storeThumbnailBytes(capturedId, thumbBytes)
                                }
                            }
                            true
                        } else {
                            false
                        }
                    }
                }
                contentTypeIsFile(item.contentType) -> {
                    // File frame: store actual bytes so the user can save/copy them.
                    // file_name/mime are carried in-band so the label shows the real
                    // name ("[file: report.pdf]") instead of "[file]".
                    if (plaintextBytes.isEmpty()) {
                        false
                    } else {
                        val label = SyncFileHelper.buildFileLabel(item.fileName)
                        val storedId = repository.storeItem(
                            plaintext = label,
                            key = key,
                            overrideId = item.itemId,
                            contentType = item.contentType,
                        )
                        if (storedId.isNotEmpty()) {
                            repository.storeFileBytes(storedId, plaintextBytes)
                            repository.storeFileMeta(storedId, item.fileName, item.mime)
                            true
                        } else {
                            false
                        }
                    }
                }
                else -> {
                    // Text frame: LWW-replace under the peer's STABLE item_id so an
                    // EDITED clip replaces the prior local row instead of being
                    // deduped/dropped (AB-17 — parity with the cloud/relay paths,
                    // which already use storeItemWithLww). SyncedItem carries no
                    // lamport field over the frozen P2P ABI, so wall_time_ms is the
                    // causal basis — the same value already observed into the local
                    // Lamport clock above and the same basis the macOS daemon's P2P
                    // LWW uses.
                    val plaintext = String(plaintextBytes, Charsets.UTF_8)
                    val didStore = repository.storeItemWithLww(
                        plaintext = plaintext,
                        key = key,
                        itemId = item.itemId,
                        incomingLamportTs = item.wallTimeMs,
                    )
                    // Auto-apply is intentionally NOT done per-frame here. The BATCH
                    // callers (P2pDialer.dialPairedPeer / the Supabase drain) apply
                    // only the NEWEST stored text clip once via onSyncedTextClip —
                    // applying per-frame would spam the system clipboard and
                    // re-trigger the capture loop during a multi-item catch-up.
                    didStore
                }
            }

            // lcmq: apply authoritative pin state (pin/unpin/reorder) from the P2P item.
            // Uses applyAuthoritativePinState — not setPinned — so authoritative unpins and
            // pin_order convergence work without minting a new local mutation.
            if (stored) {
                repository.applyAuthoritativePinState(item.itemId, item.pinned, item.pinOrder)
            }

            stored
        }

    companion object {
        private const val TAG = "SyncedItemStore"
    }
}
