package com.copypaste.android

import android.graphics.Bitmap
import uniffi.copypaste_android.BootstrapResult
import uniffi.copypaste_android.LocalItem
import uniffi.copypaste_android.P2pSyncResult
import uniffi.copypaste_android.ScannedPairing
import uniffi.copypaste_android.SyncProvisioning

// CopyPaste-vp63.38: extracted from PairController.kt so PairController stays
// under the file-size target (the interface + its verbose Real delegation
// account for ~100 lines on their own).

/**
 * Seam over every native/UniFFI call (and the Bitmap-producing QR encode) that
 * [PairController] drives, so the pairing state machine can be exercised in a
 * plain JUnit test with a fake implementation — no `.so`, no ZXing, no Bitmap
 * factory required. [Real] delegates to the existing package-level wrappers
 * (CopypasteBindings.kt / QrUtils.kt) — behaviour is unchanged.
 *
 * SECURITY: never log the `pakePassword` / session key material that flows
 * through these calls; fakes used in tests MUST use obviously-fake fixtures,
 * never a real pairing secret.
 */
internal interface PairingApi {
    fun startPairing(deviceId: String, deviceName: String): PairingQrResult
    fun parsePairing(payload: String): ScannedPairing
    fun encodeQrBitmap(qr: String, sizePx: Int): Bitmap
    fun bootstrapPairInitiator(
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
    ): BootstrapResult
    fun syncWithPeer(
        peerAddr: String,
        peerFingerprint: String,
        sessionKey: ByteArray,
        certDer: List<UByte>,
        keyDer: List<UByte>,
        localItems: List<LocalItem>,
        revokedFingerprints: List<String>,
        deviceId: String,
    ): P2pSyncResult
    fun listRevokedFingerprints(dbPath: String, key: ByteArray): List<String>

    /** Delegates to the real package-level wrappers — unchanged FFI behaviour. */
    object Real : PairingApi {
        override fun startPairing(deviceId: String, deviceName: String) =
            com.copypaste.android.startPairing(deviceId, deviceName)

        override fun parsePairing(payload: String) =
            com.copypaste.android.parsePairing(payload)

        override fun encodeQrBitmap(qr: String, sizePx: Int) =
            com.copypaste.android.encodeQrBitmap(qr, sizePx)

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
        ) = uniffi.copypaste_android.bootstrapPairInitiator(
            addrHint = addrHint,
            certDer = certDer,
            keyDer = keyDer,
            pakePassword = pakePassword,
            syncAddr = syncAddr,
            localProvisioning = localProvisioning,
            deviceName = deviceName,
            deviceModel = deviceModel,
            osVersion = osVersion,
            appVersion = appVersion,
            localIp = localIp,
            publicIp = publicIp,
        )

        override fun syncWithPeer(
            peerAddr: String,
            peerFingerprint: String,
            sessionKey: ByteArray,
            certDer: List<UByte>,
            keyDer: List<UByte>,
            localItems: List<LocalItem>,
            revokedFingerprints: List<String>,
            deviceId: String,
        ) = com.copypaste.android.syncWithPeer(
            peerAddr = peerAddr,
            peerFingerprint = peerFingerprint,
            sessionKey = sessionKey,
            certDer = certDer,
            keyDer = keyDer,
            localItems = localItems,
            revokedFingerprints = revokedFingerprints,
            deviceId = deviceId,
        )

        override fun listRevokedFingerprints(dbPath: String, key: ByteArray) =
            com.copypaste.android.listRevokedFingerprints(dbPath, key)
    }
}

/**
 * Pure outcome of [PairController.Companion.scanTransition] — parsing a
 * scanned/deep-linked pairing payload. No Context/Settings dependency: fully
 * exercisable in a JUnit test by injecting a fake [PairingApi].
 */
internal sealed class ScanTransition {
    /** Payload parsed successfully — the peer-review UI should now be shown. */
    data class Scanned(
        val scannedPeer: ScannedPairing,
        val scannedInfo: String,
        val pendingProvisioningRaw: String,
    ) : ScanTransition()

    /** Payload failed to parse — a sanitized, user-facing error message. */
    data class Failed(val errorMessage: String) : ScanTransition()
}
