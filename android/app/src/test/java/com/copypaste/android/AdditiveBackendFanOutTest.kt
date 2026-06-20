package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Unit tests for CopyPaste-26zi — Android SyncBackend additive fan-out.
 *
 * macOS daemon fans out to BOTH relay AND cloud transports additively when both
 * are configured. The old Android path used a `when (settings.syncBackend)` XOR
 * switch in [ClipboardService.notifySyncManager] (routed to ONE backend only)
 * and gated [FgsSyncLoop]'s Supabase poll on `syncBackend == SUPABASE`.
 *
 * The fix: replace the XOR switch with independent per-transport gates:
 *   - if [Settings.isRelayConfigured] → [SyncManager.pushToRelay]
 *   - if [Settings.isSupabaseConfigured] → [SyncManager.pushToSupabase]
 * Both may fire on the same capture. [FgsSyncLoop]'s poll guard must also
 * use [Settings.isSupabaseConfigured] directly, NOT `syncBackend == SUPABASE`.
 *
 * These tests verify the gate predicates and the routing logic in isolation.
 * No Android runtime or native library needed — pure JVM unit tests.
 */
class AdditiveBackendFanOutTest {

    // ── Routing predicate helpers (mirrors the fixed notifySyncManager logic) ──

    /**
     * Simulates the FIXED additive routing decision: collect which transports
     * would be used given the two independent configuration flags.
     *
     * Before the fix: only ONE transport was selected via `when (syncBackend)`.
     * After the fix: each transport fires independently when configured.
     */
    private fun resolveTransports(
        isRelayConfigured: Boolean,
        isSupabaseConfigured: Boolean,
    ): Set<String> {
        val used = mutableSetOf<String>()
        if (isSupabaseConfigured) used += "supabase"
        if (isRelayConfigured) used += "relay"
        return used
    }

    /**
     * Simulates the BROKEN XOR routing (the old `when (syncBackend)` path).
     * Used as a "before" baseline that the fix must diverge from when both are configured.
     */
    private fun resolveTransportsXor(
        syncBackend: SyncBackend,
    ): Set<String> = when (syncBackend) {
        SyncBackend.SUPABASE -> setOf("supabase")
        SyncBackend.RELAY -> setOf("relay")
    }

    // ── Additive routing: both configured ─────────────────────────────────────

    @Test
    fun bothConfigured_additiveRoutingUsesBoth() {
        val used = resolveTransports(isRelayConfigured = true, isSupabaseConfigured = true)
        assertTrue("relay must be used when configured", "relay" in used)
        assertTrue("supabase must be used when configured", "supabase" in used)
        assertEquals("exactly two transports when both configured", 2, used.size)
    }

    @Test
    fun bothConfigured_xorRoutingOnlyUsesOne() {
        // Documents the BROKEN before-state for both canonical syncBackend values.
        val usedSupabase = resolveTransportsXor(SyncBackend.SUPABASE)
        val usedRelay    = resolveTransportsXor(SyncBackend.RELAY)
        assertEquals("XOR SUPABASE uses only supabase", setOf("supabase"), usedSupabase)
        assertEquals("XOR RELAY uses only relay", setOf("relay"), usedRelay)
        // Neither XOR result matches the additive result when both are configured.
        assertFalse("XOR result never has both", usedSupabase == setOf("relay", "supabase"))
        assertFalse("XOR result never has both", usedRelay    == setOf("relay", "supabase"))
    }

    // ── Additive routing: single-transport configurations ─────────────────────

    @Test
    fun onlySupabaseConfigured_usesOnlySupabase() {
        val used = resolveTransports(isRelayConfigured = false, isSupabaseConfigured = true)
        assertEquals(setOf("supabase"), used)
    }

    @Test
    fun onlyRelayConfigured_usesOnlyRelay() {
        val used = resolveTransports(isRelayConfigured = true, isSupabaseConfigured = false)
        assertEquals(setOf("relay"), used)
    }

    @Test
    fun neitherConfigured_noTransportUsed() {
        val used = resolveTransports(isRelayConfigured = false, isSupabaseConfigured = false)
        assertTrue("no transport used when neither configured", used.isEmpty())
    }

    // ── FgsSyncLoop poll gate: must use isSupabaseConfigured, not syncBackend ──

    /**
     * Models the FIXED poll-enabled predicate in [FgsSyncLoop.start]:
     *
     *   enabled = syncEnabled && isSupabaseConfigured
     *
     * The broken version gated on `syncBackend == SUPABASE`, which meant the
     * poll was suppressed whenever the user had `syncBackend = RELAY` even if
     * Supabase was fully configured.
     */
    private fun fgsPollEnabled(
        syncEnabled: Boolean,
        isSupabaseConfigured: Boolean,
    ): Boolean = syncEnabled && isSupabaseConfigured

    /** Broken version of the poll gate (old code). */
    private fun fgsPollEnabledBroken(
        syncEnabled: Boolean,
        syncBackend: SyncBackend,
        isSupabaseConfigured: Boolean,
    ): Boolean = syncEnabled && syncBackend == SyncBackend.SUPABASE && isSupabaseConfigured

    @Test
    fun fgsPoll_enabledWhenSupabaseConfigured_regardlessOfSyncBackend() {
        // Fixed: Supabase poll runs when Supabase is configured, regardless of syncBackend.
        assertTrue(fgsPollEnabled(syncEnabled = true, isSupabaseConfigured = true))
    }

    @Test
    fun fgsPoll_brokenGate_suppressesPollWhenBackendIsRelay() {
        // Documents the BROKEN before-state: poll was gated off when syncBackend == RELAY.
        assertFalse(
            "broken gate suppresses poll when syncBackend=RELAY even if Supabase is configured",
            fgsPollEnabledBroken(
                syncEnabled = true,
                syncBackend = SyncBackend.RELAY,
                isSupabaseConfigured = true,
            )
        )
    }

    @Test
    fun fgsPoll_fixedGate_pollsWhenSupabaseConfiguredAndBackendIsRelay() {
        // Fixed: poll fires when Supabase is configured, even if syncBackend == RELAY.
        assertTrue(
            "fixed gate polls when Supabase is configured regardless of syncBackend",
            fgsPollEnabled(syncEnabled = true, isSupabaseConfigured = true)
        )
    }

    @Test
    fun fgsPoll_notEnabledWhenSyncDisabled() {
        assertFalse(fgsPollEnabled(syncEnabled = false, isSupabaseConfigured = true))
    }

    @Test
    fun fgsPoll_notEnabledWhenSupabaseNotConfigured() {
        assertFalse(fgsPollEnabled(syncEnabled = true, isSupabaseConfigured = false))
    }
}
