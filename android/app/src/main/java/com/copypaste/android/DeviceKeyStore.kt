package com.copypaste.android

import android.content.Context
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uniffi.copypaste_android.DeviceCert
import uniffi.copypaste_android.generateDeviceCert

/**
 * Persists this device's P2P identity certificate.
 *
 * On first use [getOrCreate] calls the Rust FFI [generateDeviceCert] (a fresh
 * self-signed ECDSA P-256 leaf cert + private key) ONCE and persists it via
 * [Settings.p2pIdentity], which wraps the private key with the AndroidKeyStore
 * KEK (the same mechanism used for the master encryption key and the paired-peer
 * session key). Every subsequent launch reuses the stored identity.
 *
 * STABILITY CONTRACT: the same cert/key pair MUST be reused across every pairing
 * and sync so the peer can pin our fingerprint. Regenerating would mint a new
 * fingerprint, which the peer's mTLS allowlist rejects — silently breaking P2P
 * sync after an app restart. This is the Android-side mirror of the daemon's
 * `load_or_create` cert persistence.
 *
 * Generation is lazy (first pairing) and happens on [Dispatchers.IO] since the
 * Rust call does CPU-bound key generation off the main thread.
 */
class DeviceKeyStore(context: Context) {

    private val settings = Settings(context)

    /**
     * Return the persisted device cert, generating and storing it on first use.
     * Must be called off the main thread (already hops to [Dispatchers.IO]).
     */
    suspend fun getOrCreate(): DeviceCert = withContext(Dispatchers.IO) {
        settings.p2pIdentity?.toDeviceCert() ?: run {
            val cert = generateDeviceCert()
            // CopyPaste-ah3i / CopyPaste-1jms.11: convert to P2pIdentity, persist
            // (the setter wraps keyDer with the AndroidKeyStore KEK), then zero the
            // plaintext private-key ByteArray so it does not linger on the heap.
            //
            // CopyPaste-1jms.11: pass a COPY of keyDer to Settings so that
            // zeroKeyMaterial() below cannot corrupt any ByteArray reference that
            // Settings (or wrapKey) might still hold. The original identity.keyDer is
            // zeroed; the cert returned to the caller retains its own keyDer (List<UByte>
            // from FFI) which is a separate allocation and is unaffected.
            val identity = cert.toP2pIdentity()
            settings.p2pIdentity = identity.copy(keyDer = identity.keyDer.copyOf())
            // Zero the original keyDer now that the wrapped copy is stored.
            identity.zeroKeyMaterial()
            // CopyPaste-44rq.55: also zero cert.keyDer (the List<UByte> from the
            // UniFFI FFI allocation). Since DeviceCert.keyDer is `var` we can
            // replace it with a zeroed list before returning so the private key
            // does not linger in the original FFI-allocated List object on the heap.
            // Callers that need keyDer after first-use MUST call peek() to re-fetch
            // the KEK-unwrapped key from AndroidKeyStore — settings.p2pIdentity
            // was persisted just above and is immediately available via peek().
            cert.keyDer = List(cert.keyDer.size) { 0.toUByte() }
            cert
        }
    }

    /** Return the persisted cert, or null if pairing has never run. */
    fun peek(): DeviceCert? = settings.p2pIdentity?.toDeviceCert()

    private companion object {
        private fun P2pIdentity.toDeviceCert(): DeviceCert = DeviceCert(
            deviceId = deviceId,
            fingerprint = fingerprint,
            certDer = certDer.map { it.toUByte() },
            keyDer = keyDer.map { it.toUByte() },
        )

        private fun DeviceCert.toP2pIdentity(): P2pIdentity = P2pIdentity(
            deviceId = deviceId,
            fingerprint = fingerprint,
            certDer = ByteArray(certDer.size) { certDer[it].toByte() },
            keyDer = ByteArray(keyDer.size) { keyDer[it].toByte() },
        )
    }
}
