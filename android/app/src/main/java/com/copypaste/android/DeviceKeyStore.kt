package com.copypaste.android

import android.content.Context
import android.content.SharedPreferences
import android.util.Base64
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import uniffi.copypaste_android.DeviceCert
import uniffi.copypaste_android.generateDeviceCert

/**
 * Persists this device's P2P identity certificate.
 *
 * On first use [getOrCreate] calls the Rust FFI [generateDeviceCert] (a fresh
 * self-signed X25519 leaf cert + private key) and stores the four fields in a
 * dedicated SharedPreferences file. The DER blobs are base64-encoded; the
 * device_id and fingerprint are stored verbatim.
 *
 * The same cert/key pair must be reused across every pairing and sync so the
 * peer can pin our fingerprint — regenerating would invalidate prior pairings.
 *
 * Generation is lazy (first pairing) and happens on [Dispatchers.IO] since the
 * Rust call does CPU-bound key generation off the main thread.
 */
class DeviceKeyStore(context: Context) {

    private val prefs: SharedPreferences =
        context.getSharedPreferences(PREFS_NAME, Context.MODE_PRIVATE)

    /**
     * Return the persisted device cert, generating and storing it on first use.
     * Must be called off the main thread (already hops to [Dispatchers.IO]).
     */
    suspend fun getOrCreate(): DeviceCert = withContext(Dispatchers.IO) {
        load() ?: run {
            val cert = generateDeviceCert()
            persist(cert)
            cert
        }
    }

    /** Return the persisted cert, or null if pairing has never run. */
    fun peek(): DeviceCert? = load()

    private fun load(): DeviceCert? {
        val deviceId = prefs.getString(KEY_DEVICE_ID, null) ?: return null
        val fingerprint = prefs.getString(KEY_FINGERPRINT, null) ?: return null
        val certB64 = prefs.getString(KEY_CERT_DER, null) ?: return null
        val keyB64 = prefs.getString(KEY_KEY_DER, null) ?: return null
        return DeviceCert(
            deviceId = deviceId,
            fingerprint = fingerprint,
            certDer = decodeUBytes(certB64),
            keyDer = decodeUBytes(keyB64),
        )
    }

    private fun persist(cert: DeviceCert) {
        prefs.edit()
            .putString(KEY_DEVICE_ID, cert.deviceId)
            .putString(KEY_FINGERPRINT, cert.fingerprint)
            .putString(KEY_CERT_DER, encodeUBytes(cert.certDer))
            .putString(KEY_KEY_DER, encodeUBytes(cert.keyDer))
            .apply()
    }

    companion object {
        private const val PREFS_NAME = "copypaste_device_cert"
        private const val KEY_DEVICE_ID = "device_id"
        private const val KEY_FINGERPRINT = "fingerprint"
        private const val KEY_CERT_DER = "cert_der_b64"
        private const val KEY_KEY_DER = "key_der_b64"

        private fun encodeUBytes(bytes: List<UByte>): String {
            val raw = ByteArray(bytes.size) { bytes[it].toByte() }
            return Base64.encodeToString(raw, Base64.NO_WRAP)
        }

        private fun decodeUBytes(b64: String): List<UByte> {
            val raw = Base64.decode(b64, Base64.NO_WRAP)
            return raw.map { it.toUByte() }
        }
    }
}
