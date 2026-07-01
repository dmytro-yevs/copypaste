package com.copypaste.android

/**
 * Reversible, pure-JVM fake for [KekCipher] (CopyPaste-vp63.36 injected-KEK
 * seam). The real [AndroidKeystoreKekCipher] requires a device/emulator
 * AndroidKeyStore; this fake XORs with a fixed per-instance key so
 * wrap(unwrap(x)) round-trips deterministically in JUnit, letting
 * [KeystoreSecretStore] / [PeerRosterStore] / [P2pIdentityStore] pure
 * serialization + migration logic be characterized without touching
 * AndroidKeyStore.
 *
 * NOT secure — test-only. The "iv" byte is stored as a 1-byte tag (0x00 for a
 * normal wrap) so [failNextUnwrap] can simulate a lost/incompatible KEK by
 * flipping it, exercising the "wrapped blob exists but cannot be decrypted"
 * error paths (EncryptionKeyLostException, empty-string/null fallbacks).
 */
class FakeKekCipher(private val keyByte: Byte = 0x5A) : KekCipher {
    /** When true, the next [unwrap] call throws instead of decoding. */
    var failNextUnwrap: Boolean = false

    override fun wrap(raw: ByteArray): Pair<ByteArray, ByteArray> {
        val ct = ByteArray(raw.size) { i -> (raw[i].toInt() xor keyByte.toInt()).toByte() }
        return ct to byteArrayOf(0x00)
    }

    override fun unwrap(wrapped: ByteArray, iv: ByteArray): ByteArray {
        if (failNextUnwrap) {
            failNextUnwrap = false
            throw IllegalStateException("FakeKekCipher: simulated KEK unwrap failure")
        }
        return ByteArray(wrapped.size) { i -> (wrapped[i].toInt() xor keyByte.toInt()).toByte() }
    }
}
