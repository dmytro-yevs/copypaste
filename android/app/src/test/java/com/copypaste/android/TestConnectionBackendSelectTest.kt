package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * CopyPaste-bdac.42: backend selection for the "Test connection" button.
 *
 * The probe must test the ENABLED + CONFIGURED backend(s) — NOT hardcoded relay.
 * Selection is pure (no I/O, no coroutines) so it is fully unit-testable here.
 *
 * Rules:
 *   - Relay is testable  iff relayEnabled   == true AND relayUrl  is not blank.
 *   - Supabase is testable iff supabaseEnabled == true AND supabaseUrl + anonKey are not blank.
 *   - Both, one, or neither may be selected independently (additive model, CopyPaste-26zi).
 */
class TestConnectionBackendSelectTest {

    // ── relay-only ────────────────────────────────────────────────────────────

    @Test
    fun `relay enabled and configured — relay is selected`() {
        val spec = selectTestBackends(
            relayEnabled = true,
            relayUrl = "http://relay.example.com",
            supabaseEnabled = false,
            supabaseUrl = "",
            supabaseAnonKey = "",
        )
        assertTrue("relay must be selected when enabled and URL is set", spec.relay)
        assertFalse("supabase must NOT be selected when disabled", spec.supabase)
    }

    @Test
    fun `relay enabled but blank URL — relay is NOT selected`() {
        val spec = selectTestBackends(
            relayEnabled = true,
            relayUrl = "",
            supabaseEnabled = false,
            supabaseUrl = "",
            supabaseAnonKey = "",
        )
        assertFalse("relay must NOT be selected when URL is blank", spec.relay)
        assertFalse("supabase must NOT be selected", spec.supabase)
    }

    @Test
    fun `relay disabled with URL — relay is NOT selected`() {
        val spec = selectTestBackends(
            relayEnabled = false,
            relayUrl = "http://relay.example.com",
            supabaseEnabled = false,
            supabaseUrl = "",
            supabaseAnonKey = "",
        )
        assertFalse("relay must NOT be selected when disabled (even if URL is set)", spec.relay)
    }

    // ── supabase-only ─────────────────────────────────────────────────────────

    @Test
    fun `supabase enabled and configured — supabase is selected`() {
        val spec = selectTestBackends(
            relayEnabled = false,
            relayUrl = "",
            supabaseEnabled = true,
            supabaseUrl = "https://xyz.supabase.co",
            supabaseAnonKey = "eyJhbGci",
        )
        assertFalse("relay must NOT be selected when disabled", spec.relay)
        assertTrue("supabase must be selected when enabled and URL+key are set", spec.supabase)
    }

    @Test
    fun `supabase enabled but blank URL — supabase is NOT selected`() {
        val spec = selectTestBackends(
            relayEnabled = false,
            relayUrl = "",
            supabaseEnabled = true,
            supabaseUrl = "",
            supabaseAnonKey = "eyJhbGci",
        )
        assertFalse("supabase must NOT be selected when URL is blank", spec.supabase)
    }

    @Test
    fun `supabase enabled with URL but blank anonKey — supabase is NOT selected`() {
        val spec = selectTestBackends(
            relayEnabled = false,
            relayUrl = "",
            supabaseEnabled = true,
            supabaseUrl = "https://xyz.supabase.co",
            supabaseAnonKey = "",
        )
        assertFalse("supabase must NOT be selected when anon key is blank", spec.supabase)
    }

    @Test
    fun `supabase disabled with URL and key — supabase is NOT selected`() {
        val spec = selectTestBackends(
            relayEnabled = false,
            relayUrl = "",
            supabaseEnabled = false,
            supabaseUrl = "https://xyz.supabase.co",
            supabaseAnonKey = "eyJhbGci",
        )
        assertFalse("supabase must NOT be selected when disabled (even if URL+key are set)", spec.supabase)
    }

    // ── both enabled ──────────────────────────────────────────────────────────

    @Test
    fun `both enabled and configured — both are selected`() {
        val spec = selectTestBackends(
            relayEnabled = true,
            relayUrl = "http://relay.example.com",
            supabaseEnabled = true,
            supabaseUrl = "https://xyz.supabase.co",
            supabaseAnonKey = "eyJhbGci",
        )
        assertTrue("relay must be selected when enabled + configured", spec.relay)
        assertTrue("supabase must be selected when enabled + configured", spec.supabase)
    }

    // ── neither ───────────────────────────────────────────────────────────────

    @Test
    fun `neither enabled — nothing is selected`() {
        val spec = selectTestBackends(
            relayEnabled = false,
            relayUrl = "http://relay.example.com",
            supabaseEnabled = false,
            supabaseUrl = "https://xyz.supabase.co",
            supabaseAnonKey = "eyJhbGci",
        )
        assertFalse("relay must NOT be selected when disabled", spec.relay)
        assertFalse("supabase must NOT be selected when disabled", spec.supabase)
    }

    // ── equality of selected-backend set ─────────────────────────────────────

    /**
     * The set of backends TESTED must equal the set of ENABLED+CONFIGURED backends.
     * This formalises the core requirement of CopyPaste-bdac.42.
     */
    @Test
    fun `selected set equals enabled-and-configured set — supabase only`() {
        val spec = selectTestBackends(
            relayEnabled = false,
            relayUrl = "",
            supabaseEnabled = true,
            supabaseUrl = "https://xyz.supabase.co",
            supabaseAnonKey = "eyJhbGci",
        )
        val selected = buildSet {
            if (spec.relay)    add("relay")
            if (spec.supabase) add("supabase")
        }
        assertEquals(
            "only supabase should be in the selected set",
            setOf("supabase"),
            selected,
        )
    }

    @Test
    fun `selected set equals enabled-and-configured set — relay only`() {
        val spec = selectTestBackends(
            relayEnabled = true,
            relayUrl = "http://relay.example.com",
            supabaseEnabled = false,
            supabaseUrl = "",
            supabaseAnonKey = "",
        )
        val selected = buildSet {
            if (spec.relay)    add("relay")
            if (spec.supabase) add("supabase")
        }
        assertEquals(
            "only relay should be in the selected set",
            setOf("relay"),
            selected,
        )
    }

    @Test
    fun `selected set equals enabled-and-configured set — both`() {
        val spec = selectTestBackends(
            relayEnabled = true,
            relayUrl = "http://relay.example.com",
            supabaseEnabled = true,
            supabaseUrl = "https://xyz.supabase.co",
            supabaseAnonKey = "eyJhbGci",
        )
        val selected = buildSet {
            if (spec.relay)    add("relay")
            if (spec.supabase) add("supabase")
        }
        assertEquals(
            "both relay and supabase should be in the selected set",
            setOf("relay", "supabase"),
            selected,
        )
    }
}
