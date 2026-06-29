package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * CopyPaste-otb7: Sync Diagnostics must source backend (Relay/Supabase) connection
 * status from ACTUAL backend operation results (last success / last error per
 * backend) — NOT from paired-peer P2P presence (lastSync / onlineCount).
 *
 * Bugs being fixed:
 *  - Bad cloud creds previously showed "Connected" because a peer was online (P2P).
 *  - Healthy cloud with no peer previously showed "Idle" because onlineCount was 0.
 *  - A draft (unsaved/unauthenticated) email rendered as a signed-in "Account".
 *
 * [deriveBackendConnState] takes only backend op timestamps; peer presence is a
 * separate signal and intentionally NOT an input here.
 */
class SyncDiagnosticsBackendStatusTest {

    private val recentMs = 5 * 60 * 1_000L
    private val now = 10_000_000L

    @Test
    fun `unknown when no backend op has ever occurred`() {
        val state = deriveBackendConnState(
            lastSuccessMs = 0L, lastErrorMs = 0L, nowMs = now, recentMs = recentMs,
        )
        assertEquals(BackendConnState.Unknown, state)
    }

    @Test
    fun `connected when last backend success is recent`() {
        val state = deriveBackendConnState(
            lastSuccessMs = now - 30_000L, lastErrorMs = 0L, nowMs = now, recentMs = recentMs,
        )
        assertEquals(BackendConnState.Connected, state)
    }

    @Test
    fun `error when last backend error is newer than last success`() {
        // Bad cloud creds: backend op failed most recently. Must show Error even if
        // a P2P peer is online (peer presence is NOT an input to this function).
        val state = deriveBackendConnState(
            lastSuccessMs = now - 600_000L, lastErrorMs = now - 1_000L, nowMs = now, recentMs = recentMs,
        )
        assertEquals(BackendConnState.Error, state)
    }

    @Test
    fun `connected with no peers online still reports backend success`() {
        // Healthy cloud, zero P2P peers: backend success is recent → Connected,
        // NOT Idle. The old code returned Idle here because onlineCount was 0.
        val state = deriveBackendConnState(
            lastSuccessMs = now - 5_000L, lastErrorMs = 0L, nowMs = now, recentMs = recentMs,
        )
        assertEquals(BackendConnState.Connected, state)
    }

    @Test
    fun `idle when last success is stale and no newer error`() {
        val state = deriveBackendConnState(
            lastSuccessMs = now - (6 * 60 * 1_000L), lastErrorMs = 0L, nowMs = now, recentMs = recentMs,
        )
        assertEquals(BackendConnState.Idle, state)
    }

    @Test
    fun `recovered success newer than error reports connected`() {
        val state = deriveBackendConnState(
            lastSuccessMs = now - 1_000L, lastErrorMs = now - 60_000L, nowMs = now, recentMs = recentMs,
        )
        assertEquals(BackendConnState.Connected, state)
    }

    // ── signed-in inference (never from a draft email) ───────────────────────

    @Test
    fun `draft email alone never implies signed in`() {
        // A non-blank email the user is typing, with no active backend session,
        // must NOT render as signed-in.
        assertFalse(isSupabaseSignedIn(savedEmail = "user@example.com", hasActiveSession = false))
    }

    @Test
    fun `signed in requires both saved email and active session`() {
        assertTrue(isSupabaseSignedIn(savedEmail = "user@example.com", hasActiveSession = true))
    }

    @Test
    fun `blank saved email is never signed in even with a session`() {
        assertFalse(isSupabaseSignedIn(savedEmail = "", hasActiveSession = true))
    }
}
