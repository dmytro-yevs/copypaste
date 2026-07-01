package com.copypaste.android

import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * CopyPaste-vp63.36: characterization tests for [PeerRosterStore]'s pure
 * roster JSON parse/serialize logic, upsert/remove/updateLastSync mutation
 * semantics, the legacy single-peer migration, and the legacy scalar shims —
 * via [FakeSharedPreferences] + the injected [FakeKekCipher]/[FakeBase64Codec]
 * seams (session-key wrap/unwrap is KEK-bound but exercised end-to-end here
 * since the fake is fully reversible).
 */
class PeerRosterStoreTest {

    private var removedFingerprints = mutableListOf<String>()

    private fun store(prefs: FakeSharedPreferences = FakeSharedPreferences()): PeerRosterStore {
        removedFingerprints = mutableListOf()
        val secrets = KeystoreSecretStore(prefs, FakeKekCipher(), FakeBase64Codec)
        return PeerRosterStore(
            prefs,
            secrets,
            onPeerRemoved = { fp -> removedFingerprints += fp },
            b64 = FakeBase64Codec,
        )
    }

    private fun peer(fingerprint: String, name: String = "peer-$fingerprint") = PairedPeer(
        fingerprint = fingerprint,
        syncAddr = "10.0.0.1:9999",
        name = name,
    )

    @Test
    fun `parse then serialize round-trips a roster with metadata`() {
        val s = store()
        val original = listOf(
            PairedPeer(
                fingerprint = "fp1",
                syncAddr = "10.0.0.5:1234",
                name = "Alice's Pixel",
                sessionKeyWrappedB64 = "wrapped-b64",
                sessionKeyIvB64 = "iv-b64",
                lastSyncMs = 42L,
                pairedAtMs = 7L,
                peerModel = "Pixel 8",
                peerOs = "Android 15",
                peerAppVersion = "1.2.3",
                peerLocalIp = "10.0.0.5",
                peerPublicIp = "203.0.113.9",
                peerDeviceId = "device-uuid-1",
            ),
        )
        val json = s.serializePairedPeers(original)
        val parsed = s.parsePairedPeers(json)
        assertEquals(original, parsed)
    }

    @Test
    fun `parsePairedPeers drops entries with a blank fingerprint`() {
        val s = store()
        val json = """[{"fingerprint":"","syncAddr":"x"},{"fingerprint":"fp2","syncAddr":"y"}]"""
        val parsed = s.parsePairedPeers(json)
        assertEquals(listOf("fp2"), parsed.map { it.fingerprint })
    }

    @Test
    fun `parsePairedPeers on malformed JSON yields empty roster, not a crash`() {
        val s = store()
        assertEquals(emptyList<PairedPeer>(), s.parsePairedPeers("not json at all"))
    }

    @Test
    fun `upsertPeer appends a new peer without discarding the first`() {
        val s = store()
        s.upsertPeer(peer("fp1"))
        s.upsertPeer(peer("fp2"))
        assertEquals(listOf("fp1", "fp2"), s.pairedPeers.map { it.fingerprint })
    }

    @Test
    fun `upsertPeer replaces an existing entry in place, preserving order`() {
        val s = store()
        s.upsertPeer(peer("fp1", "Old Name"))
        s.upsertPeer(peer("fp2"))
        s.upsertPeer(peer("fp1", "New Name"))
        assertEquals(listOf("fp1", "fp2"), s.pairedPeers.map { it.fingerprint })
        assertEquals("New Name", s.pairedPeers.first { it.fingerprint == "fp1" }.name)
    }

    @Test
    fun `removePeer drops the matching entry and invokes the high-water callback`() {
        val s = store()
        s.upsertPeer(peer("fp1"))
        s.upsertPeer(peer("fp2"))
        s.removePeer("fp1")
        assertEquals(listOf("fp2"), s.pairedPeers.map { it.fingerprint })
        assertEquals(listOf("fp1"), removedFingerprints)
    }

    @Test
    fun `removePeer is a no-op for an unknown fingerprint`() {
        val s = store()
        s.upsertPeer(peer("fp1"))
        s.removePeer("does-not-exist")
        assertEquals(listOf("fp1"), s.pairedPeers.map { it.fingerprint })
        assertTrue(removedFingerprints.isEmpty())
    }

    @Test
    fun `updatePeerLastSync only changes lastSyncMs, preserving other fields`() {
        val s = store()
        s.upsertPeer(peer("fp1"))
        s.updatePeerLastSync("fp1", 999L)
        val updated = s.pairedPeers.first()
        assertEquals(999L, updated.lastSyncMs)
        assertEquals("10.0.0.1:9999", updated.syncAddr)
    }

    @Test
    fun `updatePeerLastSync is a no-op for an unknown fingerprint`() {
        val s = store()
        s.upsertPeer(peer("fp1"))
        s.updatePeerLastSync("unknown", 999L)
        assertEquals(0L, s.pairedPeers.first().lastSyncMs)
    }

    @Test
    fun `sessionKeyFor round-trips a wrapped session key via wrapSessionKey`() {
        val s = store()
        val raw = ByteArray(32) { it.toByte() }
        val (wrappedB64, ivB64) = s.wrapSessionKey(raw)
        s.upsertPeer(
            peer("fp1").copy(sessionKeyWrappedB64 = wrappedB64, sessionKeyIvB64 = ivB64)
        )
        assertArrayEquals(raw, s.sessionKeyFor("fp1"))
    }

    @Test
    fun `sessionKeyFor returns empty array for an unknown peer`() {
        assertArrayEquals(ByteArray(0), store().sessionKeyFor("unknown"))
    }

    @Test
    fun `migrateLegacyPairedPeer synthesizes a roster entry from legacy scalar fields`() {
        val prefs = FakeSharedPreferences()
        prefs.edit()
            .putString("paired_peer_fingerprint", "legacy-fp")
            .putString("paired_peer_sync_addr", "192.168.1.5:4321")
            .apply()
        val s = store(prefs)

        val roster = s.pairedPeers
        assertEquals(1, roster.size)
        assertEquals("legacy-fp", roster[0].fingerprint)
        assertEquals("192.168.1.5:4321", roster[0].syncAddr)
        // Migration must be idempotent: subsequent reads see the persisted roster,
        // not a re-synthesized duplicate.
        assertEquals(1, s.pairedPeers.size)
    }

    @Test
    fun `migrateLegacyPairedPeer is a no-op when no legacy fingerprint exists`() {
        assertTrue(store().pairedPeers.isEmpty())
    }

    // ── Legacy single-peer shims ─────────────────────────────────────────────

    @Test
    fun `pairedPeerFingerprint shim inserts a new peer when none exists`() {
        val s = store()
        s.pairedPeerFingerprint = "fp1"
        assertEquals("fp1", s.pairedPeerFingerprint)
        assertEquals(1, s.pairedPeers.size)
    }

    @Test
    fun `pairedPeerFingerprint shim blank clears the first peer`() {
        val s = store()
        s.upsertPeer(peer("fp1"))
        s.pairedPeerFingerprint = ""
        assertTrue(s.pairedPeers.isEmpty())
        assertEquals(listOf("fp1"), removedFingerprints)
    }

    @Test
    fun `pairedPeerFingerprint shim rename preserves addr and session key`() {
        val s = store()
        val (wrappedB64, ivB64) = s.wrapSessionKey(ByteArray(32) { 7 })
        s.upsertPeer(peer("fp1").copy(sessionKeyWrappedB64 = wrappedB64, sessionKeyIvB64 = ivB64))

        s.pairedPeerFingerprint = "fp1-renamed"

        assertEquals("fp1-renamed", s.pairedPeerFingerprint)
        assertEquals("10.0.0.1:9999", s.pairedPeerSyncAddr)
        assertArrayEquals(ByteArray(32) { 7 }, s.pairedPeerSessionKey)
    }

    @Test
    fun `pairedPeerSyncAddr shim updates the first peer's syncAddr`() {
        val s = store()
        s.upsertPeer(peer("fp1"))
        s.pairedPeerSyncAddr = "1.2.3.4:5"
        assertEquals("1.2.3.4:5", s.pairedPeers.first().syncAddr)
    }

    @Test
    fun `pairedPeerSessionKey shim round-trips through the first peer`() {
        val s = store()
        s.upsertPeer(peer("fp1"))
        val raw = ByteArray(32) { 9 }
        s.pairedPeerSessionKey = raw
        assertArrayEquals(raw, s.pairedPeerSessionKey)
    }

    @Test
    fun `pairedPeerSessionKey shim empty array clears the wrapped fields`() {
        val s = store()
        s.upsertPeer(peer("fp1"))
        s.pairedPeerSessionKey = ByteArray(32) { 9 }
        s.pairedPeerSessionKey = ByteArray(0)
        assertArrayEquals(ByteArray(0), s.pairedPeerSessionKey)
        assertEquals("", s.pairedPeers.first().sessionKeyWrappedB64)
    }
}
