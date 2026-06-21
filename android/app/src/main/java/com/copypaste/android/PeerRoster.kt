package com.copypaste.android

/**
 * One paired peer in the multi-peer roster ([Settings.pairedPeers]).
 *
 * The PAKE session key is stored KEK-wrapped: [sessionKeyWrappedB64] is the
 * base64 AES-GCM ciphertext and [sessionKeyIvB64] its IV. Raw key bytes are
 * NEVER held in this type or written to the roster JSON — use
 * [Settings.sessionKeyFor] (or [Settings.wrapSessionKey] to build the wrapped
 * fields). [fingerprint] is the roster key (peers are upserted/removed by it).
 */
data class PairedPeer(
    val fingerprint: String,
    val syncAddr: String,
    val name: String,
    val sessionKeyWrappedB64: String,
    val sessionKeyIvB64: String,
    val lastSyncMs: Long = 0L,
    /**
     * Unix epoch ms when this device was paired (stamped at pairing time).
     * Parity with macOS PairedDevice.added_at (stored as epoch seconds there;
     * we store ms here and convert to seconds for display).
     * Defaults to 0 (unknown) for peers persisted before this field was added.
     */
    val pairedAtMs: Long = 0L,
    // ABI 14 (HB-1b): the peer's device metadata, learned in-band during pairing
    // (BootstrapResult.peer*/PairStatus.peer*). Persisted here so Wave 3 can render
    // a device card at parity with macOS. All null for a legacy peer / pre-ABI-14
    // roster entry. peerPublicIp is informational only — never a trust input.
    val peerModel: String? = null,
    val peerOs: String? = null,
    val peerAppVersion: String? = null,
    val peerLocalIp: String? = null,
    val peerPublicIp: String? = null,
    /**
     * CopyPaste-27m7: the peer's stable device UUID (from Hello.device_id in the sync
     * protocol), distinct from [fingerprint] (the TLS certificate hash).
     *
     * [ClipboardItem.originDeviceId] holds this UUID, NOT the TLS fingerprint, so
     * [OriginDeviceFilter.deviceDisplayName] must match on this field to resolve peer
     * names. Null for legacy roster entries written before this field was added.
     *
     * Populated in [PairActivity.runPairAndSync] and [SasPairingDialog.persistConfirmed]
     * when the FFI surface exposes the peer's device_id (BootstrapResult/PairStatus).
     * At the time of writing (Wave-3), neither BootstrapResult nor PairStatus carries
     * peer_device_id in the UDL — that FFI gap is tracked separately. The field is
     * persisted now so it will be populated automatically once the FFI is extended.
     */
    val peerDeviceId: String? = null,
    // Runtime-only: round-trip time in ms measured by FgsSyncLoop over the mTLS P2P
    // connection. Not persisted to the roster JSON — populated in-memory during an
    // active sync session. Wired to the UI via DevicesViewModel; actual FgsSyncLoop
    // instrumentation deferred to CopyPaste-8dd (Gradle build cycle).
    val latencyMs: Int? = null,
    /**
     * CopyPaste-1jms.4: true when this peer was admitted through the SAS
     * (Short Authentication String) confirmation flow (QR scan + visual verify).
     * Defaults to [true] for backward-compatibility with existing persisted
     * entries (all historical entries were created via the SAS flow).
     *
     * Set to [false] for peers admitted by any other mechanism (e.g. future
     * cloud-import or admin provisioning) so [trustLabel] can surface the
     * distinction. Only [PairActivity.runPairAndSync] and
     * [SasPairingDialog.persistConfirmed] should set this to [true].
     */
    val sasVerified: Boolean = true,
) {
    /** Convenience overload for callers that have no wrapped key yet (e.g. the
     *  legacy-fingerprint shim). Defaults the wrapped fields to empty. */
    constructor(fingerprint: String, syncAddr: String, name: String) :
        this(fingerprint, syncAddr, name, "", "", 0L, 0L)
}

/**
 * This device's persistent P2P mTLS identity. The raw DER blobs cross the UniFFI
 * boundary as `List<UByte>` (see [uniffi.copypaste_android.DeviceCert]); this
 * type holds them as `ByteArray` for storage and is converted at the FFI seam.
 *
 * [keyDer] is secret private-key material — never log it or persist it in
 * cleartext. [Settings.p2pIdentity] wraps it with the AndroidKeyStore KEK.
 */
data class P2pIdentity(
    val deviceId: String,
    val fingerprint: String,
    val certDer: ByteArray,
    val keyDer: ByteArray,
) {
    // Content equality on the DER blobs (the default data-class equals/hashCode
    // compare ByteArray by reference, which is never useful here).
    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is P2pIdentity) return false
        return deviceId == other.deviceId &&
            fingerprint == other.fingerprint &&
            certDer.contentEquals(other.certDer) &&
            keyDer.contentEquals(other.keyDer)
    }

    override fun hashCode(): Int {
        var result = deviceId.hashCode()
        result = 31 * result + fingerprint.hashCode()
        result = 31 * result + certDer.contentHashCode()
        result = 31 * result + keyDer.contentHashCode()
        return result
    }

    /**
     * CopyPaste-ah3i: zero the private key material in-place.
     *
     * Call this immediately after the keyDer has been wrapped by the
     * AndroidKeyStore KEK (i.e. after [Settings.p2pIdentity] setter returns)
     * so that the plaintext private key DER bytes do not linger in heap memory
     * longer than necessary. Mirrors the UDL secret ByteArray zeroing contract.
     *
     * Safe to call multiple times (idempotent: fill with 0 twice is still 0).
     */
    fun zeroKeyMaterial() {
        keyDer.fill(0)
    }
}
