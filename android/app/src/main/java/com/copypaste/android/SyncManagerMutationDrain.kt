package com.copypaste.android

import android.util.Log
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext

/**
 * Outbound mutation queue drain for [SyncManager] (CopyPaste-0qpn), extracted
 * CopyPaste-vp63.34. Extension functions on [SyncManager]; public signature
 * ([SyncManager.drainOutboundMutationQueue]) is unchanged so existing callers
 * ([ClipboardService], [FgsSyncLoop]) are unaffected.
 */
private const val TAG = "SyncManager"

/**
 * Drain the [OutboundMutationQueue] and push each pending mutation over every
 * configured transport (relay, Supabase).
 *
 * ## What this fixes
 *
 * UI mutations (pin/unpin/reorder/delete/bulk-delete/clear) previously only
 * wrote local SharedPreferences. No sync producer fired for them, so peers
 * never received the changes. This producer pushes each queued mutation as
 * a tombstone (OP_DELETE/OP_BULK_DELETE/OP_CLEAR) or a pin-state envelope
 * (OP_PIN/OP_UNPIN/OP_REORDER) to every active transport.
 *
 * ## Tombstones
 *
 * Delete operations push a tombstone envelope: `deleted=true`, `ct_b64=""`,
 * with the bumped `lamport_ts`. Receivers apply it via their existing
 * `applyInboundTombstoneWithLww` path (relay SSE + Supabase poll + P2P).
 *
 * ## Pin mutations
 *
 * Pin/reorder push a live item envelope whose `pinned` and `pin_order` fields
 * carry the authoritative state. We cannot re-encrypt the payload here (no
 * decryption key in the SyncManager), so we read the existing cloud-encrypted
 * form by re-using [SyncManager.pushToRelay] / [SyncManager.pushToSupabase] with a sentinel that
 * pushes a zero-byte plaintext when `pinned=true` but signals only metadata.
 *
 * Design choice: for pin-only mutations we push a relay/Supabase tombstone
 * with `deleted=false`, `pinned=<state>`, `pin_order=<order>`, and an
 * EMPTY ct_b64. Receivers check `deleted` first; a non-deleted envelope with
 * empty ct_b64 is treated as a pin-metadata-only update — the receiver's
 * `applyAuthoritativePinState` handles this without overwriting the item body.
 *
 * Both transports are fully supported. [SupabaseClient.pushMutationRow] PATCHes
 * the existing row (filtered by item_id) to set `deleted`/`pinned`/`pin_order`
 * and bumps `lamport_ts` — mirroring the daemon's `cloud.rs` `mark_deleted` /
 * `update_pin_state` paths. A successful push on either transport marks the
 * record delivered; the queue entry is removed only after at least one transport
 * confirms success.
 *
 * ## Per-transport behaviour
 *
 * | Op         | Relay | Supabase |
 * |------------|-------|----------|
 * | DELETE     | yes   | yes      |
 * | BULK_DELETE| yes   | yes      |
 * | CLEAR      | yes   | yes      |
 * | PIN        | yes   | yes      |
 * | UNPIN      | yes   | yes      |
 * | REORDER    | yes   | yes      |
 *
 * ## Idempotency
 *
 * Records are removed from the queue only after a successful push. A failed
 * push leaves the record in the queue for retry on the next drain call.
 * Receivers dedup via LWW on item_id + lamport_ts.
 *
 * @param context        Android context for [OutboundMutationQueue] access.
 * @param repository     Used only to resolve the current pin state for validation.
 * @return               Number of records successfully delivered.
 */
@Suppress("UNUSED_PARAMETER") // repository reserved for future use
suspend fun SyncManager.drainOutboundMutationQueue(
    context: android.content.Context,
    repository: ClipboardRepository,
): Int = withContext(Dispatchers.IO) {
    val s = settings ?: run {
        Log.w(TAG, "drainOutboundMutationQueue: no Settings instance provided")
        return@withContext 0
    }

    // CopyPaste-1t38: single-flight. If a drain is already running (periodic
    // tick + UI hook racing), skip — the in-flight drain covers the same queue.
    if (!draining.compareAndSet(false, true)) {
        Log.d(TAG, "drainOutboundMutationQueue: drain already in flight — skipping")
        return@withContext 0
    }
    try {
        drainOutboundMutationQueueInner(context, s)
    } finally {
        draining.set(false)
    }
}

private suspend fun SyncManager.drainOutboundMutationQueueInner(
    context: android.content.Context,
    s: Settings,
): Int {
    val pending = OutboundMutationQueue.peekQueue(context)
    if (pending.isEmpty()) return 0

    // CopyPaste-yaip: the set of transports a record must reach before it can
    // be dropped. p2p is "enabled" only when P2P sync is on AND at least one
    // peer is paired; SyncManager cannot ack p2p itself — a record needing p2p
    // stays pending until FgsSyncLoop's dial acks it. relay/Supabase "enabled"
    // means CONFIGURED: a transient resolve/auth failure leaves the record
    // pending for that transport (retried next drain), it is never dropped.
    val enabled = OutboundMutationQueue.enabledTransports(
        relay = s.isRelayConfigured,
        supabase = s.isSupabaseConfigured,
        p2p = s.p2pSyncEnabled && s.pairedPeers.isNotEmpty(),
    )
    if (enabled.isEmpty()) {
        Log.d(TAG, "drainOutboundMutationQueue: no transport enabled — leaving ${pending.size} record(s) queued")
        return 0
    }

    Log.d(TAG, "drainOutboundMutationQueue: draining ${pending.size} pending mutation(s); enabled=$enabled")

    // CopyPaste-yaip: resolve Supabase context once outside the per-record loop.
    // resolveSyncContext is ~0 ms on the happy path (cached JWT + cached sync key).
    // Null when Supabase is unconfigured; Supabase pushes are skipped in that case.
    val supaCtx = if (s.isSupabaseConfigured) {
        try {
            resolveSyncContext()
        } catch (e: Exception) {
            Log.w(TAG, "drainOutboundMutationQueue: Supabase context unavailable: ${e.message}")
            null
        }
    } else {
        null
    }

    // CopyPaste-yaip: per-record, per-transport acknowledgements. A record is
    // NOT dropped just because ONE transport succeeded — applyAcks removes it
    // only once every enabled transport has acked. We attempt a transport only
    // when it is enabled AND not already acked for this record (cheaper retries).
    val newAcks = mutableMapOf<Pair<String, Long>, MutableSet<String>>()
    fun ack(rec: OutboundMutationQueue.MutationRecord, transport: String) {
        newAcks.getOrPut(rec.itemId to rec.lamportTs) { mutableSetOf() }.add(transport)
    }

    val relayEnabled = OutboundMutationQueue.TRANSPORT_RELAY in enabled
    val supabaseEnabled = OutboundMutationQueue.TRANSPORT_SUPABASE in enabled

    for (rec in pending) {
        val isDelete = rec.op == OutboundMutationQueue.OP_DELETE ||
            rec.op == OutboundMutationQueue.OP_BULK_DELETE ||
            rec.op == OutboundMutationQueue.OP_CLEAR
        // CopyPaste-yaip: un-suppress isPinOp — pin mutations now push to Supabase.
        val isPinOp = rec.op == OutboundMutationQueue.OP_PIN ||
            rec.op == OutboundMutationQueue.OP_UNPIN ||
            rec.op == OutboundMutationQueue.OP_REORDER

        // ── Relay transport ──────────────────────────────────────────────
        if (relayEnabled && OutboundMutationQueue.TRANSPORT_RELAY !in rec.ackedTransports) {
            try {
                val relayOk = pushToRelay(
                    itemId = rec.itemId,
                    // Tombstones and pin-only ops carry empty plaintext.
                    plaintext = ByteArray(0),
                    contentType = "text",
                    lamportTs = rec.lamportTs,
                    deleted = isDelete,
                    pinned = rec.pinned,
                    pinOrder = rec.pinOrder,
                )
                if (relayOk) {
                    ack(rec, OutboundMutationQueue.TRANSPORT_RELAY)
                    Log.d(
                        TAG,
                        "drainOutboundMutationQueue: relay ok ${rec.op} " +
                            "itemId=${rec.itemId.take(8)}… lamport=${rec.lamportTs}",
                    )
                } else {
                    Log.w(
                        TAG,
                        "drainOutboundMutationQueue: relay push failed for ${rec.op} " +
                            "itemId=${rec.itemId.take(8)}… — staying pending for relay",
                    )
                }
            } catch (e: Exception) {
                Log.w(TAG, "drainOutboundMutationQueue: relay exception for ${rec.op}: ${e.message}")
            }
        }

        // ── Supabase transport (CopyPaste-yaip) ───────────────────────────
        // Tombstones and pin mutations both use SupabaseClient.pushMutationRow,
        // which PATCHes the existing row (filtered by item_id) to set
        // deleted/pinned/pin_order + bumped lamport_ts — mirrors the daemon's
        // cloud.rs `mark_deleted` / `update_pin_state` paths.
        //
        // The Supabase ack is recorded INDEPENDENTLY of the relay ack: a relay
        // failure no longer drops the record (and vice versa). supaCtx may be
        // null on a transient resolve/auth failure even though Supabase is
        // enabled — in that case we record no ack and the record stays pending
        // for Supabase, to be retried on the next drain.
        if (supabaseEnabled &&
            OutboundMutationQueue.TRANSPORT_SUPABASE !in rec.ackedTransports &&
            supaCtx != null &&
            (isDelete || isPinOp)
        ) {
            try {
                val supaOk = supaCtx.client.pushMutationRow(
                    bearerToken = supaCtx.bearer,
                    itemId = rec.itemId,
                    lamportTs = rec.lamportTs,
                    isDelete = isDelete,
                    pinned = rec.pinned,
                    pinOrder = rec.pinOrder,
                )
                if (supaOk) {
                    ack(rec, OutboundMutationQueue.TRANSPORT_SUPABASE)
                    Log.d(
                        TAG,
                        "drainOutboundMutationQueue: supabase ok ${rec.op} " +
                            "itemId=${rec.itemId.take(8)}… lamport=${rec.lamportTs}",
                    )
                } else {
                    Log.w(
                        TAG,
                        "drainOutboundMutationQueue: supabase push failed for ${rec.op} " +
                            "itemId=${rec.itemId.take(8)}… — staying pending for supabase",
                    )
                }
            } catch (e: Exception) {
                Log.w(TAG, "drainOutboundMutationQueue: supabase exception for ${rec.op}: ${e.message}")
            }
        }
    }

    // CopyPaste-yaip: durably merge this pass's acks. A record is removed ONLY
    // when every enabled transport (relay, Supabase, and p2p when applicable)
    // has acknowledged it; partial success persists the remaining transports as
    // still-pending. p2p is acked separately by FgsSyncLoop's dial path.
    val removed = OutboundMutationQueue.applyAcks(context, newAcks, enabled)

    Log.d(
        TAG,
        "drainOutboundMutationQueue: removed $removed/${pending.size} fully-acked record(s)",
    )
    return removed
}
