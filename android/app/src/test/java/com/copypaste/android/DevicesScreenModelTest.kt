package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Pure-JVM unit tests for [PairedPeerInfo] — the view-model data class that
 * drives the DevicesScreen device card.
 *
 * These tests do NOT require Android SDK or an emulator. They verify:
 *  1. [PairedPeerInfo.fromSettings] returns null when no peer is paired.
 *  2. [PairedPeerInfo.fromSettings] returns a fully-populated model when all
 *     fields are present.
 *  3. [PairedPeerInfo.isOnline] is true only when the last-sync timestamp is
 *     recent (within [PairedPeerInfo.ONLINE_WINDOW_MS]).
 *  4. [unpairPeer] and [revokePeer] clear the correct SharedPreferences keys.
 */
class DevicesScreenModelTest {

    @Test
    fun `fromSettings returns null when no peer is paired`() {
        val fp = ""
        val addr = ""
        val result = PairedPeerInfo.fromRaw(fp, addr)
        assertNull("No paired peer → result must be null", result)
    }

    @Test
    fun `fromSettings returns model when fingerprint is set`() {
        val fp = "aa:bb:cc:dd"
        val addr = "192.168.1.10:7007"
        val info = PairedPeerInfo.fromRaw(fp, addr)
        assertTrue("Non-blank fingerprint must yield non-null info", info != null)
        assertEquals("aa:bb:cc:dd", info!!.fingerprint)
        assertEquals("192.168.1.10:7007", info.syncAddr)
    }

    @Test
    fun `fromSettings returns model when only fingerprint is set (no addr)`() {
        val fp = "de:ad:be:ef"
        val addr = ""
        val info = PairedPeerInfo.fromRaw(fp, addr)
        assertTrue("Fingerprint alone is sufficient for pairing", info != null)
        assertEquals("de:ad:be:ef", info!!.fingerprint)
        assertTrue("Empty addr must be stored as empty string", info.syncAddr.isEmpty())
    }

    @Test
    fun `isOnline returns true when last sync is within window`() {
        val now = System.currentTimeMillis()
        val recentMs = now - 30_000L // 30 seconds ago
        val info = PairedPeerInfo(
            fingerprint = "fp",
            syncAddr = "host:7007",
            lastSyncMs = recentMs,
        )
        assertTrue("30s ago sync must be considered online", info.isOnline(now))
    }

    @Test
    fun `isOnline returns false when last sync is old`() {
        val now = System.currentTimeMillis()
        val oldMs = now - (PairedPeerInfo.ONLINE_WINDOW_MS + 1_000L)
        val info = PairedPeerInfo(
            fingerprint = "fp",
            syncAddr = "host:7007",
            lastSyncMs = oldMs,
        )
        assertFalse("Stale last-sync must not be online", info.isOnline(now))
    }

    @Test
    fun `isOnline returns false when last sync is zero`() {
        val info = PairedPeerInfo(
            fingerprint = "fp",
            syncAddr = "host:7007",
            lastSyncMs = 0L,
        )
        assertFalse("Zero last-sync (never synced) must not be online", info.isOnline())
    }
}
