package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Pure-JVM tests for the per-record, per-transport acknowledgement model
 * (CopyPaste-yaip) and the periodic relay/cloud drain decision (CopyPaste-1t38).
 *
 * ## What this covers
 *
 * The crux of yaip is that a queued mutation must NOT be dropped when only ONE
 * of several enabled transports succeeds — it must stay pending for the transports
 * that have not yet acknowledged it, so a device reachable only via the failed
 * transport still converges on the next drain. The crux of 1t38 is that after an
 * offline→online transition the periodic loop must re-attempt the drain within
 * one loop interval (governed by a backoff window), without a UI action.
 *
 * These tests drive the pure logic only — no Android runtime, no SharedPreferences,
 * no FFI. Run with:
 *   ./gradlew :app:testDebugUnitTest --tests "*.OutboundMutationQueueAckTest"
 */
class OutboundMutationQueueAckTest {

    private fun rec(
        itemId: String,
        op: String = OutboundMutationQueue.OP_PIN,
        lamportTs: Long = 1L,
        acked: Set<String> = emptySet(),
    ) = OutboundMutationQueue.MutationRecord(
        itemId = itemId,
        op = op,
        lamportTs = lamportTs,
        wallTimeMs = 100L,
        pinned = true,
        pinOrder = 1.0,
        ackedTransports = acked,
    )

    // ── enabledTransports ─────────────────────────────────────────────────────

    @Test
    fun `enabledTransports includes only the configured targets`() {
        assertEquals(
            setOf(OutboundMutationQueue.TRANSPORT_RELAY, OutboundMutationQueue.TRANSPORT_SUPABASE),
            OutboundMutationQueue.enabledTransports(relay = true, supabase = true, p2p = false),
        )
        assertEquals(
            setOf(OutboundMutationQueue.TRANSPORT_RELAY),
            OutboundMutationQueue.enabledTransports(relay = true, supabase = false, p2p = false),
        )
        assertEquals(
            emptySet<String>(),
            OutboundMutationQueue.enabledTransports(relay = false, supabase = false, p2p = false),
        )
        assertEquals(
            setOf(
                OutboundMutationQueue.TRANSPORT_RELAY,
                OutboundMutationQueue.TRANSPORT_SUPABASE,
                OutboundMutationQueue.TRANSPORT_P2P,
            ),
            OutboundMutationQueue.enabledTransports(relay = true, supabase = true, p2p = true),
        )
    }

    // ── isFullyAcked ──────────────────────────────────────────────────────────

    @Test
    fun `isFullyAcked is true only when every enabled transport acked`() {
        val enabled = setOf(
            OutboundMutationQueue.TRANSPORT_RELAY,
            OutboundMutationQueue.TRANSPORT_SUPABASE,
        )
        assertFalse(
            OutboundMutationQueue.isFullyAcked(
                rec("a", acked = setOf(OutboundMutationQueue.TRANSPORT_RELAY)),
                enabled,
            ),
        )
        assertTrue(
            OutboundMutationQueue.isFullyAcked(
                rec(
                    "a",
                    acked = setOf(
                        OutboundMutationQueue.TRANSPORT_RELAY,
                        OutboundMutationQueue.TRANSPORT_SUPABASE,
                    ),
                ),
                enabled,
            ),
        )
    }

    @Test
    fun `isFullyAcked is false when no transport is enabled so the record is never silently dropped`() {
        // Defensive: with zero enabled targets there is nowhere to deliver. Rather
        // than vacuously dropping the mutation we KEEP it (isFullyAcked == false).
        assertFalse(
            OutboundMutationQueue.isFullyAcked(
                rec("a", acked = emptySet()),
                emptySet(),
            ),
        )
    }

    // ── reconcile: the per-transport ack matrix ───────────────────────────────

    @Test
    fun `relay ok cloud fail keeps the record pending for cloud`() {
        val enabled = setOf(
            OutboundMutationQueue.TRANSPORT_RELAY,
            OutboundMutationQueue.TRANSPORT_SUPABASE,
        )
        val records = listOf(rec("item-1", lamportTs = 7L))
        val newAcks = mapOf(
            ("item-1" to 7L) to setOf(OutboundMutationQueue.TRANSPORT_RELAY),
        )
        val remaining = OutboundMutationQueue.reconcile(records, newAcks, enabled)
        assertEquals("record must stay pending for cloud", 1, remaining.size)
        assertEquals(
            setOf(OutboundMutationQueue.TRANSPORT_RELAY),
            remaining[0].ackedTransports,
        )
    }

    @Test
    fun `cloud ok relay fail keeps the record pending for relay`() {
        val enabled = setOf(
            OutboundMutationQueue.TRANSPORT_RELAY,
            OutboundMutationQueue.TRANSPORT_SUPABASE,
        )
        val records = listOf(rec("item-2", lamportTs = 9L))
        val newAcks = mapOf(
            ("item-2" to 9L) to setOf(OutboundMutationQueue.TRANSPORT_SUPABASE),
        )
        val remaining = OutboundMutationQueue.reconcile(records, newAcks, enabled)
        assertEquals(1, remaining.size)
        assertEquals(
            setOf(OutboundMutationQueue.TRANSPORT_SUPABASE),
            remaining[0].ackedTransports,
        )
    }

    @Test
    fun `both transports ok removes the record`() {
        val enabled = setOf(
            OutboundMutationQueue.TRANSPORT_RELAY,
            OutboundMutationQueue.TRANSPORT_SUPABASE,
        )
        val records = listOf(rec("item-3", lamportTs = 11L))
        val newAcks = mapOf(
            ("item-3" to 11L) to setOf(
                OutboundMutationQueue.TRANSPORT_RELAY,
                OutboundMutationQueue.TRANSPORT_SUPABASE,
            ),
        )
        val remaining = OutboundMutationQueue.reconcile(records, newAcks, enabled)
        assertTrue("fully acked record must be removed", remaining.isEmpty())
    }

    @Test
    fun `record stays pending for p2p even after relay and cloud both ack`() {
        val enabled = setOf(
            OutboundMutationQueue.TRANSPORT_RELAY,
            OutboundMutationQueue.TRANSPORT_SUPABASE,
            OutboundMutationQueue.TRANSPORT_P2P,
        )
        val records = listOf(rec("item-4", lamportTs = 13L))
        // SyncManager drain acks relay + cloud, but cannot ack p2p.
        val afterCloudRelay = OutboundMutationQueue.reconcile(
            records,
            mapOf(
                ("item-4" to 13L) to setOf(
                    OutboundMutationQueue.TRANSPORT_RELAY,
                    OutboundMutationQueue.TRANSPORT_SUPABASE,
                ),
            ),
            enabled,
        )
        assertEquals("must remain pending for p2p", 1, afterCloudRelay.size)
        assertEquals(
            setOf(
                OutboundMutationQueue.TRANSPORT_RELAY,
                OutboundMutationQueue.TRANSPORT_SUPABASE,
            ),
            afterCloudRelay[0].ackedTransports,
        )
        // FgsSyncLoop p2p dial later acks p2p → now fully acked → removed.
        val afterP2p = OutboundMutationQueue.reconcile(
            afterCloudRelay,
            mapOf(("item-4" to 13L) to setOf(OutboundMutationQueue.TRANSPORT_P2P)),
            enabled,
        )
        assertTrue(afterP2p.isEmpty())
    }

    @Test
    fun `reconcile keeps records when no transport is enabled`() {
        val records = listOf(rec("item-5"))
        val remaining = OutboundMutationQueue.reconcile(records, emptyMap(), emptySet())
        assertEquals(1, remaining.size)
    }

    @Test
    fun `reconcile prunes acks for transports that are no longer enabled`() {
        // Relay was previously acked, but relay is now disabled and only supabase
        // is enabled. The record must be retained pending supabase, and the stale
        // relay ack pruned so it cannot mask a future relay re-enable.
        val enabled = setOf(OutboundMutationQueue.TRANSPORT_SUPABASE)
        val records = listOf(rec("item-6", acked = setOf(OutboundMutationQueue.TRANSPORT_RELAY)))
        val remaining = OutboundMutationQueue.reconcile(records, emptyMap(), enabled)
        assertEquals(1, remaining.size)
        assertEquals(emptySet<String>(), remaining[0].ackedTransports)
    }

    // ── Restart recovery: partial acks survive encode/decode ──────────────────

    @Test
    fun `partial acks survive encode-decode across a simulated restart`() {
        val records = listOf(
            rec("item-7", op = OutboundMutationQueue.OP_DELETE, lamportTs = 21L,
                acked = setOf(OutboundMutationQueue.TRANSPORT_RELAY)),
            rec("item-8", op = OutboundMutationQueue.OP_PIN, lamportTs = 22L,
                acked = setOf(
                    OutboundMutationQueue.TRANSPORT_RELAY,
                    OutboundMutationQueue.TRANSPORT_SUPABASE,
                )),
        )
        val persisted = OutboundMutationQueue.encodeQueue(records)
        val recovered = OutboundMutationQueue.decodeQueue(persisted)
        assertEquals(2, recovered.size)
        assertEquals(
            setOf(OutboundMutationQueue.TRANSPORT_RELAY),
            recovered[0].ackedTransports,
        )
        assertEquals(
            setOf(
                OutboundMutationQueue.TRANSPORT_RELAY,
                OutboundMutationQueue.TRANSPORT_SUPABASE,
            ),
            recovered[1].ackedTransports,
        )
    }

    @Test
    fun `legacy 6-field record decodes with empty ack set`() {
        // Records persisted before this change have no 7th field. They must decode
        // with an empty ack set (not crash, not be dropped).
        val legacy = "legacy-id|pin|5|6|true|1.0"
        val decoded = OutboundMutationQueue.MutationRecord.decode(legacy)
        assertEquals("legacy-id", decoded?.itemId)
        assertEquals(emptySet<String>(), decoded?.ackedTransports)
    }

    // ── 1t38: periodic drain after an offline→online transition ───────────────

    @Test
    fun `shouldAttemptDrain respects queue emptiness and the backoff window`() {
        // Empty queue → never attempt.
        assertFalse(FgsSyncLoop.shouldAttemptDrain(queueSize = 0, nowMs = 1000L, backoffUntilMs = 0L))
        // Pending but inside the backoff window → skip.
        assertFalse(FgsSyncLoop.shouldAttemptDrain(queueSize = 3, nowMs = 500L, backoffUntilMs = 1000L))
        // Pending and the backoff window has elapsed → attempt.
        assertTrue(FgsSyncLoop.shouldAttemptDrain(queueSize = 3, nowMs = 1000L, backoffUntilMs = 1000L))
    }

    @Test
    fun `periodic drain delivers a queued mutation after a simulated offline to online transition`() {
        // Fake clock + fake transports. While "offline" both transports throw, so
        // the drain produces no acks and the record stays pending while the backoff
        // window grows. After "online" both transports succeed and the record is
        // removed — all WITHOUT a new UI action, proving 1t38's periodic drain.
        val enabled = setOf(
            OutboundMutationQueue.TRANSPORT_RELAY,
            OutboundMutationQueue.TRANSPORT_SUPABASE,
        )
        var online = false
        // Fake transport: returns the ack set it can deliver for the given record.
        fun fakeDrainPass(records: List<OutboundMutationQueue.MutationRecord>):
            Map<Pair<String, Long>, Set<String>> {
            if (!online) return emptyMap() // offline: nothing delivered
            return records.associate { r ->
                (r.itemId to r.lamportTs) to enabled
            }
        }

        var queue = listOf(rec("offline-item", lamportTs = 42L))
        var now = 0L
        var backoffUntil = 0L
        var failures = 0

        // ── Tick 1 (offline): drain attempted, nothing delivered, backoff set ──
        assertTrue(FgsSyncLoop.shouldAttemptDrain(queue.size, now, backoffUntil))
        run {
            val acks = fakeDrainPass(queue)
            queue = OutboundMutationQueue.reconcile(queue, acks, enabled)
        }
        assertEquals("record stays pending while offline", 1, queue.size)
        failures += 1
        backoffUntil = now + FgsSyncLoop.backoffMs(failures)

        // ── Tick 2 (still inside backoff): skipped ──
        now += 1_000L
        assertFalse(
            "must not hammer the network inside the backoff window",
            FgsSyncLoop.shouldAttemptDrain(queue.size, now, backoffUntil),
        )

        // ── Connectivity returns; clock advances past the backoff window ──
        online = true
        now = backoffUntil + 1L

        // ── Tick 3 (online): drain attempted and the record is delivered ──
        assertTrue(FgsSyncLoop.shouldAttemptDrain(queue.size, now, backoffUntil))
        run {
            val acks = fakeDrainPass(queue)
            queue = OutboundMutationQueue.reconcile(queue, acks, enabled)
        }
        assertTrue("queue drains after connectivity returns, no UI action", queue.isEmpty())
    }
}
