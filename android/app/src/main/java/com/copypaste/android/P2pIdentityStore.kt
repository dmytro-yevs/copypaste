package com.copypaste.android

import android.content.Context
import android.content.SharedPreferences
import android.util.Base64
import android.util.Log

/**
 * Collaborator extracted from the [Settings] god-file (CopyPaste-vp63.36):
 * owns this device's persistent P2P mTLS identity (self-signed cert + private
 * key + fingerprint) plus the one-time migration from the legacy plaintext
 * `copypaste_device_cert` prefs file. [Settings] delegates [p2pIdentity]
 * verbatim (facade, zero call-site churn).
 *
 * STABILITY CONTRACT: the identity MUST be generated exactly once and reused
 * across every app launch/pairing/sync session — see original doc on
 * [Settings.p2pIdentity]. The private key is KEK-wrapped via [secrets]; it is
 * NEVER persisted in cleartext.
 *
 * @param b64 injected so tests can supply a real (JVM-side) base64 codec —
 *   see [Base64Codec] doc for why `android.util.Base64` itself is not usable
 *   in this project's plain-JUnit unit tests.
 */
class P2pIdentityStore(
    private val prefs: SharedPreferences,
    private val appContext: Context,
    private val secrets: KeystoreSecretStore,
    private val b64: Base64Codec = AndroidBase64Codec,
) {
    var p2pIdentity: P2pIdentity?
        get() {
            migrateLegacyP2pIdentity()
            val deviceId = prefs.getString(KEY_P2P_DEVICE_ID, null) ?: return null
            val fingerprint = prefs.getString(KEY_P2P_FINGERPRINT, null) ?: return null
            val certB64 = prefs.getString(KEY_P2P_CERT_DER_B64, null) ?: return null
            val wrappedB64 = prefs.getString(KEY_P2P_KEY_WRAPPED_B64, null) ?: return null
            val ivB64 = prefs.getString(KEY_P2P_KEY_IV_B64, null) ?: return null
            val keyDer = runCatching {
                secrets.unwrap(
                    wrapped = b64.decode(wrappedB64, Base64.DEFAULT),
                    iv = b64.decode(ivB64, Base64.DEFAULT),
                )
            }.getOrElse { e ->
                Log.w(TAG, "Failed to unwrap P2P device key (${e.javaClass.simpleName}); identity reset", e)
                return null
            }
            return P2pIdentity(
                deviceId = deviceId,
                fingerprint = fingerprint,
                certDer = b64.decode(certB64, Base64.DEFAULT),
                keyDer = keyDer,
            )
        }
        set(v) {
            if (v == null) {
                prefs.edit()
                    .remove(KEY_P2P_DEVICE_ID)
                    .remove(KEY_P2P_FINGERPRINT)
                    .remove(KEY_P2P_CERT_DER_B64)
                    .remove(KEY_P2P_KEY_WRAPPED_B64)
                    .remove(KEY_P2P_KEY_IV_B64)
                    .apply()
                return
            }
            val (wrapped, iv) = secrets.wrap(v.keyDer)
            prefs.edit()
                .putString(KEY_P2P_DEVICE_ID, v.deviceId)
                .putString(KEY_P2P_FINGERPRINT, v.fingerprint)
                .putString(KEY_P2P_CERT_DER_B64, b64.encode(v.certDer, Base64.NO_WRAP))
                .putString(KEY_P2P_KEY_WRAPPED_B64, b64.encode(wrapped, Base64.DEFAULT))
                .putString(KEY_P2P_KEY_IV_B64, b64.encode(iv, Base64.DEFAULT))
                .commit() // synchronous: an identity lost to a force-stop breaks pairing
        }

    /**
     * Migrate a P2P identity persisted by an earlier build in the dedicated
     * `copypaste_device_cert` SharedPreferences file (where the private key was
     * stored as plaintext base64) into the KEK-wrapped form above, then scrub the
     * legacy file. No-op when nothing legacy exists or migration already ran.
     */
    private fun migrateLegacyP2pIdentity() {
        if (prefs.contains(KEY_P2P_KEY_WRAPPED_B64)) return
        val legacy = appContext.getSharedPreferences(LEGACY_CERT_PREFS, Context.MODE_PRIVATE)
        val deviceId = legacy.getString(LEGACY_CERT_DEVICE_ID, null) ?: return
        val fingerprint = legacy.getString(LEGACY_CERT_FINGERPRINT, null) ?: return
        val certB64 = legacy.getString(LEGACY_CERT_CERT_DER, null) ?: return
        val keyB64 = legacy.getString(LEGACY_CERT_KEY_DER, null) ?: return
        Log.i(TAG, "Migrating legacy plaintext P2P identity into AndroidKeyStore wrap")
        p2pIdentity = P2pIdentity(
            deviceId = deviceId,
            fingerprint = fingerprint,
            certDer = b64.decode(certB64, Base64.NO_WRAP),
            keyDer = b64.decode(keyB64, Base64.NO_WRAP),
        )
        legacy.edit().clear().apply()
    }

    companion object {
        private const val TAG = "P2pIdentityStore"

        // ── P2P device identity (mTLS): cert/id/fingerprint plain, key KEK-wrapped ──
        private const val KEY_P2P_DEVICE_ID = "p2p_identity_device_id"
        private const val KEY_P2P_FINGERPRINT = "p2p_identity_fingerprint"
        private const val KEY_P2P_CERT_DER_B64 = "p2p_identity_cert_der_b64"
        private const val KEY_P2P_KEY_WRAPPED_B64 = "p2p_identity_key_wrapped_b64"
        private const val KEY_P2P_KEY_IV_B64 = "p2p_identity_key_iv_b64"

        // Legacy plaintext identity prefs file (pre-KEK-wrap builds). Read-only,
        // migrated and cleared by [migrateLegacyP2pIdentity].
        private const val LEGACY_CERT_PREFS = "copypaste_device_cert"
        private const val LEGACY_CERT_DEVICE_ID = "device_id"
        private const val LEGACY_CERT_FINGERPRINT = "fingerprint"
        private const val LEGACY_CERT_CERT_DER = "cert_der_b64"
        private const val LEGACY_CERT_KEY_DER = "key_der_b64"
    }
}
