package com.copypaste.android

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
import org.junit.BeforeClass
import org.junit.Test
import org.junit.runner.RunWith
import uniffi.copypaste_android.LocalItem
import uniffi.copypaste_android.bootstrapPairInitiator
import uniffi.copypaste_android.generateDeviceCert
import uniffi.copypaste_android.parsePairingQr
import uniffi.copypaste_android.syncWithPeer

/**
 * LIVE macOS↔Android P2P sync test (runs on the emulator against a REAL remote
 * macOS daemon over the emulator→host network). TEST-ONLY: this exercises the
 * already-built P2P FFI (`generateDeviceCert` / `bootstrapPairInitiator` /
 * `syncWithPeer`) end-to-end, with NO production-code changes.
 *
 * It mirrors the exact initiator flow `PairActivity.runPairAndSync` uses, but is
 * driven by instrumentation arguments supplied by the host orchestrator:
 *
 *   -e cppair_payload "<CPPAIR1...>"   the pairing QR string from the macOS
 *                                      daemon (`copypaste pair-qr --raw`), with
 *                                      its addr_hint host already rewritten to
 *                                      10.0.2.2 (the emulator alias for the
 *                                      host loopback) by the orchestrator.
 *   -e cppair_marker  "<plaintext>"    the exact plaintext copied on macOS that
 *                                      we expect to receive over the sync.
 *
 * Emulator→host networking note: the macOS bootstrap + sync listeners bind
 * 0.0.0.0, so they are reachable from the emulator via the special host-loopback
 * alias 10.0.2.2. The QR's addr_hint and the daemon's own_sync_addr advertise a
 * host-side address (LAN IPv4 / 127.0.0.1 respectively) that the emulator cannot
 * route to, so BOTH are rewritten to 10.0.2.2 here (peer_sync_addr below). This
 * is the documented gap from PairActivity's L4 note — handled entirely test-side.
 *
 * If the args are absent (e.g. the suite is run without the orchestrator), the
 * test is skipped via JUnit Assume rather than failing.
 */
@RunWith(AndroidJUnit4::class)
class LiveP2pSyncTest {

    companion object {
        @BeforeClass
        @JvmStatic
        fun setUpClass() {
            // cargo-ndk emits libcopypaste_android.so; point JNA at it before the
            // first UniFFI call (same override the conformance test uses).
            System.setProperty(
                "uniffi.component.copypaste_android.libraryOverride",
                "copypaste_android",
            )
        }

        private fun List<UByte>.toByteArray(): ByteArray =
            ByteArray(size) { this[it].toByte() }

        /** Rewrite the host of a `host:port` to the emulator→host alias 10.0.2.2. */
        private fun rewriteHostToEmulatorAlias(addr: String): String {
            val port = addr.substringAfterLast(':', "")
            require(port.isNotEmpty()) { "address '$addr' has no :port" }
            return "10.0.2.2:$port"
        }
    }

    @Test
    fun liveSyncFromMacOsDaemon() {
        val args = InstrumentationRegistry.getArguments()
        val payload = args.getString("cppair_payload")
        val expectedMarker = args.getString("cppair_marker")
        assumeTrue(
            "LiveP2pSyncTest requires -e cppair_payload and -e cppair_marker " +
                "(supplied by the host orchestrator); skipping.",
            !payload.isNullOrBlank() && !expectedMarker.isNullOrBlank(),
        )

        // 1. Parse the (host-rewritten) QR payload natively — recovers the peer
        //    fingerprint, addr_hint (already 10.0.2.2:<bootstrap_port>) and the
        //    PAKE password derived from the single-use token.
        val scanned = parsePairingQr(payload!!)

        // 2. This device's mTLS identity for the bootstrap + sync channels.
        val cert = generateDeviceCert()

        // 3. PAKE bootstrap over the real network: dial the macOS bootstrap
        //    listener (10.0.2.2:<bootstrap_port>) and run the initiator side.
        //    On success the shared session key + the peer's sync-listener address
        //    come back in-band.
        val bootstrap = bootstrapPairInitiator(
            addrHint = scanned.addrHint,
            certDer = cert.certDer,
            keyDer = cert.keyDer,
            pakePassword = scanned.pakePassword,
            syncAddr = "",
            // ABI-14 device metadata (mirrors PairActivity.runPairAndSync). A scanning
            // device carries no provisioning of its own — it receives the peer's.
            localProvisioning = null,
            deviceName = android.os.Build.MODEL ?: "AndroidTest",
            deviceModel = android.os.Build.MODEL ?: "AndroidTest",
            osVersion = "Android " + android.os.Build.VERSION.RELEASE,
            appVersion = "androidTest",
            localIp = "",
        )

        // The daemon advertises its own_sync_addr as 127.0.0.1:<port> (host
        // loopback) — unroutable from the emulator. Rewrite to 10.0.2.2:<port>.
        val peerSyncAddr = rewriteHostToEmulatorAlias(bootstrap.peerSyncAddr)

        // 4. One sync session against the macOS daemon. We send nothing
        //    (localItems empty) and expect to RECEIVE the macOS-copied item.
        val result = syncWithPeer(
            peerAddr = peerSyncAddr,
            peerFingerprint = bootstrap.peerFingerprint,
            sessionKey = bootstrap.sessionKey,
            certDer = cert.certDer,
            keyDer = cert.keyDer,
            localItems = emptyList<LocalItem>(),
            // ABI-14: this device's revocation set (none in the test) + stable id.
            revokedFingerprints = emptyList(),
            deviceId = "androidTest-device",
        )

        val plaintexts = result.items.map { String(it.plaintext.toByteArray(), Charsets.UTF_8) }

        // 5. Assert the macOS-copied marker arrived with correct plaintext.
        assertTrue(
            "expected received items to contain marker '$expectedMarker' " +
                "(itemsReceived=${result.itemsReceived}, items=$plaintexts)",
            plaintexts.any { it.contains(expectedMarker!!) },
        )
    }
}
