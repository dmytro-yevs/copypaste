package com.copypaste.android

import android.content.Context
import android.content.SharedPreferences
import android.util.Log

/**
 * Durable outbound mutation queue for UI-level clipboard mutations (CopyPaste-0qpn).
 *
 * ## Problem
 *
 * setPinned / reorderPinned / deleteItem / bulkDelete / clearAll only write to the
 * local SharedPreferences store. No sync producer fires for these operations, so
 * pin/unpin/reorder and soft-delete tombstones never propagate to other devices
 * (macOS, other Androids) via relay, Supabase, or P2P.
 *
 * ## Solution
 *
 * Before each local write, the caller enqueues a [MutationRecord] into this durable
 * queue (persisted under "copypaste_outbound_mutations" SharedPreferences). A
 * producer in [SyncManager] / [ClipboardViewModel] drains the queue and pushes each
 * record over every configured transport (relay, Supabase, P2P supplement).
 *
 * ## Queue format
 *
 * The queue is stored as a newline-delimited list of pipe-encoded [MutationRecord]
 * strings under the key [KEY_QUEUE].  Each record encodes as:
 *   `<itemId>|<op>|<lamportTs>|<wallTimeMs>|<pinned>|<pinOrder>`
 * where <pinOrder> is the double value or the sentinel "null".
 *
 * ## P2P high-water fix
 *
 * [FgsSyncLoop.filterByOutboundHighWater] uses `wallTimeMs > highWater` which
 * silently drops pin-bumped items (wall_time unchanged, only lamport_ts bumped).
 * The producer reads the queue and sends those mutations directly, bypassing the
 * wall-time filter — the queue entry carries the bumped lamport_ts so LWW works.
 *
 * ## Thread safety
 *
 * All mutations to the queue go through [enqueueMutation] / [drainQueue] under
 * [queueLock]. SharedPreferences writes use `apply()` (async) for enqueueing and
 * `commit()` (synchronous) when draining so a process kill between enqueue and drain
 * does not lose entries.
 *
 * ## Bulk-delete / clear ordering guarantee
 *
 * For [OP_BULK_DELETE] and [OP_CLEAR] the caller must enqueue per-item [OP_DELETE]
 * records BEFORE physically removing the rows, so the queue carries each itemId
 * even after the prefs keys are gone.
 */
object OutboundMutationQueue {

    private const val TAG = "OutboundMutationQueue"

    // ── Prefs key ─────────────────────────────────────────────────────────────

    internal const val PREFS_NAME = "copypaste_outbound_mutations"
    internal const val KEY_QUEUE = "queue"

    // ── Operation constants ───────────────────────────────────────────────────

    /** Item was pinned (setPinned(id, true)). */
    const val OP_PIN = "pin"

    /** Item was unpinned (setPinned(id, false)). */
    const val OP_UNPIN = "unpin"

    /** Pinned items were reordered (reorderPinned). One record per item. */
    const val OP_REORDER = "reorder"

    /** A single item was soft-deleted (deleteItem). */
    const val OP_DELETE = "delete"

    /** Multiple items were bulk-deleted (deleteItems). One record per item. */
    const val OP_BULK_DELETE = "bulk_delete"

    /** All unpinned items were cleared (clearAll / clearUnpinned). One record per
     *  deleted item (enqueued before physical removal). */
    const val OP_CLEAR = "clear"

    // ── Encode / decode (pure — no Android runtime) ───────────────────────────

    /**
     * One entry in the outbound mutation queue.
     *
     * @param itemId     Stable cross-device item UUID.
     * @param op         One of the [OP_*] constants.
     * @param lamportTs  Bumped lamport timestamp (from [ClipboardRepository.nextLamportTs]).
     * @param wallTimeMs Wall-clock ms at mutation time.
     * @param pinned     Pin state after the mutation (true for [OP_PIN]/[OP_REORDER],
     *                   false for [OP_UNPIN]/[OP_DELETE]/[OP_BULK_DELETE]/[OP_CLEAR]).
     * @param pinOrder   Fractional position in the pinned list (null for non-pin ops).
     */
    data class MutationRecord(
        val itemId: String,
        val op: String,
        val lamportTs: Long,
        val wallTimeMs: Long,
        val pinned: Boolean,
        val pinOrder: Double?,
    ) {
        /**
         * Encode as a single pipe-delimited line.
         * Format: `<itemId>|<op>|<lamportTs>|<wallTimeMs>|<pinned>|<pinOrder>`
         * where <pinOrder> is the double value or "null".
         *
         * SECURITY: itemId must never contain '|' (it is a UUID). All other fields
         * are fixed-type primitives. The format is safe for newline-delimited storage.
         */
        fun encode(): String {
            val pinOrderStr = if (pinOrder != null) pinOrder.toString() else "null"
            return "$itemId|$op|$lamportTs|$wallTimeMs|$pinned|$pinOrderStr"
        }

        companion object {
            /**
             * Decode a line encoded by [encode]. Returns null when the line is
             * malformed, so [decodeQueue] can skip corrupt entries without crashing.
             */
            fun decode(line: String): MutationRecord? {
                if (line.isBlank()) return null
                val parts = line.split("|")
                if (parts.size < 6) return null
                return try {
                    val itemId = parts[0].takeIf { it.isNotBlank() } ?: return null
                    val op = parts[1].takeIf { it.isNotBlank() } ?: return null
                    val lamportTs = parts[2].toLong()
                    val wallTimeMs = parts[3].toLong()
                    val pinned = parts[4].toBooleanStrict()
                    val pinOrder = if (parts[5] == "null") null else parts[5].toDouble()
                    MutationRecord(itemId, op, lamportTs, wallTimeMs, pinned, pinOrder)
                } catch (_: Exception) {
                    null
                }
            }
        }
    }

    /**
     * Serialize a list of [MutationRecord]s to a newline-delimited string for prefs
     * storage. An empty list serializes to an empty string.
     *
     * Pure function — no Android runtime — intentionally accessible from JVM tests.
     */
    fun encodeQueue(records: List<MutationRecord>): String =
        records.joinToString("\n") { it.encode() }

    /**
     * Deserialize the newline-delimited queue string. Malformed lines are silently
     * skipped (heal corrupt state rather than crashing).
     *
     * Pure function — no Android runtime — intentionally accessible from JVM tests.
     */
    fun decodeQueue(raw: String): List<MutationRecord> {
        if (raw.isBlank()) return emptyList()
        return raw.split("\n").mapNotNull { MutationRecord.decode(it) }
    }

    // ── Android persistence ────────────────────────────────────────────────────

    /**
     * Guards reads + writes of the queue in SharedPreferences so concurrent
     * callers (UI thread enqueue + IO-dispatcher drain) do not corrupt the list.
     */
    private val queueLock = Any()

    /**
     * Append [record] to the durable outbound queue. Uses `apply()` (async write)
     * because the local mutation (SharedPreferences.commit()) has already persisted
     * the actual clipboard data; losing this queue entry on a SIGKILL means only
     * ONE mutation is not propagated (the next mutation or process restart will
     * catch up when the queue is re-drained). `commit()` on every enqueue would
     * stall the caller's IO thread.
     */
    fun enqueueMutation(context: Context, record: MutationRecord) {
        synchronized(queueLock) {
            val prefs = prefs(context)
            val existing = prefs.getString(KEY_QUEUE, "") ?: ""
            val records = decodeQueue(existing).toMutableList()
            records.add(record)
            prefs.edit().putString(KEY_QUEUE, encodeQueue(records)).apply()
        }
        Log.d(TAG, "enqueue: ${record.op} itemId=${record.itemId.take(8)}… lamport=${record.lamportTs}")
    }

    /**
     * Read the current queue contents WITHOUT removing them. Used by the producer
     * to get a snapshot to push; call [removeRecords] after successful delivery.
     *
     * Returns an empty list when the queue is empty or the prefs are unavailable.
     */
    fun peekQueue(context: Context): List<MutationRecord> =
        synchronized(queueLock) {
            val raw = prefs(context).getString(KEY_QUEUE, "") ?: ""
            decodeQueue(raw)
        }

    /**
     * Remove records whose [MutationRecord.itemId] + [MutationRecord.lamportTs]
     * pair matches an entry in [delivered]. Uses commit() (synchronous) so the
     * producer's "delivered" acknowledgement is durable: if the process is killed
     * between a successful push and this removal, the record will be re-pushed on
     * the next startup (idempotent because the receiver uses LWW dedup).
     */
    fun removeRecords(context: Context, delivered: Set<Pair<String, Long>>) {
        if (delivered.isEmpty()) return
        synchronized(queueLock) {
            val prefs = prefs(context)
            val raw = prefs.getString(KEY_QUEUE, "") ?: ""
            val remaining = decodeQueue(raw).filter { rec ->
                (rec.itemId to rec.lamportTs) !in delivered
            }
            prefs.edit().putString(KEY_QUEUE, encodeQueue(remaining)).commit()
        }
        Log.d(TAG, "removeRecords: removed ${delivered.size} delivered records")
    }

    /**
     * Replace the entire queue with [records]. Intended for compaction: after
     * a successful full drain the producer passes the failed records back to persist
     * them for retry. Uses commit() (synchronous) for durability.
     */
    fun replaceQueue(context: Context, records: List<MutationRecord>) {
        synchronized(queueLock) {
            prefs(context).edit().putString(KEY_QUEUE, encodeQueue(records)).commit()
        }
    }

    /**
     * Return the total number of pending records. Useful for logging/metrics.
     */
    fun queueSize(context: Context): Int =
        synchronized(queueLock) {
            val raw = prefs(context).getString(KEY_QUEUE, "") ?: ""
            decodeQueue(raw).size
        }

    private fun prefs(context: Context): SharedPreferences =
        context.applicationContext.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)
}
