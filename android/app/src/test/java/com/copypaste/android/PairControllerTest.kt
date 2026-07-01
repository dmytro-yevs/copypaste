package com.copypaste.android

import android.graphics.Bitmap
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import uniffi.copypaste_android.BootstrapResult
import uniffi.copypaste_android.LocalItem
import uniffi.copypaste_android.P2pSyncResult
import uniffi.copypaste_android.ScannedPairing
import uniffi.copypaste_android.SyncProvisioning

/**
 * CopyPaste-vp63.38 — characterization tests for [PairController]'s pure
 * scan-transition reducer (`PairController.scanTransition`), the one piece of
 * the pairing state machine that has neither an Android `Context` dependency
 * (Settings/DeviceKeyStore/ClipboardRepository require one and cannot be
 * constructed under plain JUnit — see class doc on [FakePairingApi]) nor a
 * Bitmap/native dependency, so it can run as a plain JVM unit test with a
 * fake [PairingApi].
 *
 * SECURITY: every fixture below uses an OBVIOUSLY FAKE fingerprint/PAKE
 * password — never a real pairing secret, per CopyPaste-vp63.38's security
 * note (the pairing QR/payload encodes real session-key material in prod).
 */
class PairControllerTest {

    private val fakePeer = ScannedPairing(
        fingerprint = "FAKE-TEST-FINGERPRINT-0000",
        deviceId = "fake-test-device-id",
        deviceName = "Fake Test Device",
        addrHint = "203.0.113.5:9999",
        pakePassword = "FAKE-TEST-PAKE-PASSWORD-NOT-REAL",
    )

    @Test
    fun `scanTransition returns Scanned with the parsed peer and formatted info on success`() {
        val api = FakePairingApi(parseResult = { fakePeer })

        val result = PairController.scanTransition(api, "CPPAIR2.fake-payload")

        val scanned = result as ScanTransition.Scanned
        assertEquals(fakePeer, scanned.scannedPeer)
        assertEquals("Fake Test Device (FAKE-TEST-FINGERPRINT-0000)", scanned.scannedInfo)
        assertEquals("CPPAIR2.fake-payload", scanned.pendingProvisioningRaw)
    }

    @Test
    fun `scanTransition returns Failed with a sanitized message when parsePairing throws`() {
        val api = FakePairingApi(
            parseResult = { throw IllegalStateException("boom: raw FFI detail nobody should see") },
        )

        val result = PairController.scanTransition(api, "not-a-real-payload")

        val failed = result as ScanTransition.Failed
        // CopyPaste-jwga: the raw exception text ("boom: raw FFI detail...") must
        // never leak into the user-facing message.
        assertTrue(!failed.errorMessage.contains("boom"))
        assertEquals("Pairing failed. Please try again.", failed.errorMessage)
    }

    @Test
    fun `scanTransition maps a network-flavoured failure to the network fallback message`() {
        val api = FakePairingApi(
            parseResult = { throw IllegalStateException("Connection refused") },
        )

        val result = PairController.scanTransition(api, "some-payload")

        val failed = result as ScanTransition.Failed
        assertEquals(
            "Pairing failed. Check that both devices are on the same network and try again.",
            failed.errorMessage,
        )
    }

    @Test
    fun `formatScannedInfo falls back to the literal device when the name is blank`() {
        assertEquals("device (abc123)", formatScannedInfo("", "abc123"))
    }
}

/**
 * Minimal fake [PairingApi]: every method not exercised by a given test throws,
 * so a test that unexpectedly hits an un-stubbed path fails loudly instead of
 * silently returning a bogus value. Only [parsePairing] is configurable here —
 * the rest of the seam (native bootstrap/sync/Bitmap calls) is exercised by
 * [PairController.scanTransition] callers elsewhere, none of which are pure
 * enough to unit test without Robolectric (they depend on Settings/
 * DeviceKeyStore/ClipboardRepository, all of which require a real Android
 * Context to construct).
 */
private class FakePairingApi(
    private val parseResult: () -> ScannedPairing,
) : PairingApi {
    override fun startPairing(deviceId: String, deviceName: String): PairingQrResult =
        throw UnsupportedOperationException("not stubbed for this test")

    override fun parsePairing(payload: String): ScannedPairing = parseResult()

    override fun encodeQrBitmap(qr: String, sizePx: Int): Bitmap =
        throw UnsupportedOperationException("not stubbed for this test")

    override fun bootstrapPairInitiator(
        addrHint: String,
        certDer: List<UByte>,
        keyDer: List<UByte>,
        pakePassword: String,
        syncAddr: String,
        localProvisioning: SyncProvisioning?,
        deviceName: String?,
        deviceModel: String?,
        osVersion: String?,
        appVersion: String?,
        localIp: String?,
        publicIp: String?,
    ): BootstrapResult = throw UnsupportedOperationException("not stubbed for this test")

    override fun syncWithPeer(
        peerAddr: String,
        peerFingerprint: String,
        sessionKey: ByteArray,
        certDer: List<UByte>,
        keyDer: List<UByte>,
        localItems: List<LocalItem>,
        revokedFingerprints: List<String>,
        deviceId: String,
    ): P2pSyncResult = throw UnsupportedOperationException("not stubbed for this test")

    override fun listRevokedFingerprints(dbPath: String, key: ByteArray): List<String> =
        throw UnsupportedOperationException("not stubbed for this test")
}
