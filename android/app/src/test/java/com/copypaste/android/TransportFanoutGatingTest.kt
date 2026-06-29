package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * CopyPaste-26zi: per-transport additive fan-out gating.
 *
 * The old segmented Relay|Supabase control implied EXCLUSIVITY, but the runtime
 * fans out to BOTH transports additively when configured. The fix replaces the
 * exclusive selector with INDEPENDENT enable toggles; a transport sends iff it is
 * BOTH enabled (user toggle) AND configured.
 *
 * [transportFanoutSet] is the single source of truth used by both the runtime
 * fan-out (ClipboardService.notifySyncManager) and these tests, so a disabled
 * transport is provably excluded from the send set.
 */
class TransportFanoutGatingTest {

    @Test
    fun `both enabled and configured fans out to both transports`() {
        val set = transportFanoutSet(
            relayEnabled = true, relayConfigured = true,
            supabaseEnabled = true, supabaseConfigured = true,
        )
        assertEquals(setOf(SyncTransport.RELAY, SyncTransport.SUPABASE), set)
    }

    @Test
    fun `disabling relay excludes relay even when configured`() {
        val set = transportFanoutSet(
            relayEnabled = false, relayConfigured = true,
            supabaseEnabled = true, supabaseConfigured = true,
        )
        assertFalse("relay disabled must not be in fan-out", SyncTransport.RELAY in set)
        assertTrue("supabase still active", SyncTransport.SUPABASE in set)
    }

    @Test
    fun `disabling supabase excludes supabase even when configured`() {
        val set = transportFanoutSet(
            relayEnabled = true, relayConfigured = true,
            supabaseEnabled = false, supabaseConfigured = true,
        )
        assertTrue("relay still active", SyncTransport.RELAY in set)
        assertFalse("supabase disabled must not be in fan-out", SyncTransport.SUPABASE in set)
    }

    @Test
    fun `enabled but unconfigured transport is excluded`() {
        val set = transportFanoutSet(
            relayEnabled = true, relayConfigured = false,
            supabaseEnabled = true, supabaseConfigured = false,
        )
        assertTrue("enabled-but-unconfigured yields empty set", set.isEmpty())
    }

    @Test
    fun `both disabled yields empty set regardless of configuration`() {
        val set = transportFanoutSet(
            relayEnabled = false, relayConfigured = true,
            supabaseEnabled = false, supabaseConfigured = true,
        )
        assertTrue(set.isEmpty())
    }

    /**
     * Additive model: enabling both is the supported, correct state — the two are
     * independent, NOT mutually exclusive. This is the core 26zi behavioural claim.
     */
    @Test
    fun `transports are additive not mutually exclusive`() {
        val set = transportFanoutSet(
            relayEnabled = true, relayConfigured = true,
            supabaseEnabled = true, supabaseConfigured = true,
        )
        assertEquals("both transports may be active simultaneously", 2, set.size)
    }
}
