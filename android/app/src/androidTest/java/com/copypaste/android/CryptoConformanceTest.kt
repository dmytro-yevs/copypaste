package com.copypaste.android

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.json.JSONObject
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.BeforeClass
import org.junit.Test
import org.junit.runner.RunWith
import uniffi.copypaste_android.EncryptedBlob
import uniffi.copypaste_android.cloudDecrypt
import uniffi.copypaste_android.cloudEncrypt
import uniffi.copypaste_android.decryptText
import uniffi.copypaste_android.deriveCloudSyncKey
import uniffi.copypaste_android.encryptText

/**
 * Cross-language crypto conformance — Kotlin side (runs on the emulator).
 *
 * This is the actual cross-language check. It calls the UniFFI-generated Kotlin
 * bindings (`uniffi.copypaste_android.*`), which marshal across the FFI boundary
 * into the REAL Rust core compiled into `libcopypaste_android.so`. The test:
 *
 *   1. Loads the golden-vector fixture produced by the Rust generator
 *      (`crates/copypaste-android/tests/conformance_vectors.rs`), shipped here as
 *      an androidTest asset (`assets/golden_vectors.json`).
 *   2. Asserts Kotlin `decryptText` recovers each Rust-produced ciphertext into
 *      the exact recorded plaintext (Rust → Kotlin direction).
 *   3. Asserts Kotlin `encryptText` output round-trips back through Kotlin
 *      `decryptText` (Kotlin → Kotlin via the same FFI core).
 *   4. Repeats both for the cloud path, and asserts Argon2id key derivation in
 *      Kotlin reproduces the Rust-recorded sync key bit-for-bit.
 *
 * A mismatch indicates FFI / marshalling / AAD drift between the Kotlin bindings
 * and the Rust core.
 *
 * Library name note: the UniFFI bindings load `libuniffi_copypaste_android.so`
 * by default, but cargo-ndk emits `libcopypaste_android.so`. We point JNA at the
 * real artifact via the documented `uniffi.component.<name>.libraryOverride`
 * system property in [setUpClass] BEFORE any binding call triggers lib loading.
 */
@RunWith(AndroidJUnit4::class)
class CryptoConformanceTest {

    companion object {
        @BeforeClass
        @JvmStatic
        fun setUpClass() {
            // Must run before the first UniFFI call so the JNA `Native.load`
            // resolves `libcopypaste_android.so` instead of the default
            // `libuniffi_copypaste_android.so`.
            System.setProperty(
                "uniffi.component.copypaste_android.libraryOverride",
                "copypaste_android",
            )
        }

        private fun hexToBytes(hex: String): ByteArray =
            ByteArray(hex.length / 2) { i ->
                hex.substring(i * 2, i * 2 + 2).toInt(16).toByte()
            }

        /** UniFFI sequence<u8> ⇄ Kotlin: bindings use List<UByte>. */
        private fun ByteArray.toUByteList(): List<UByte> = map { it.toUByte() }

        private fun List<UByte>.toByteArray(): ByteArray =
            ByteArray(size) { this[it].toByte() }

        private fun loadFixture(): JSONObject {
            val ctx = InstrumentationRegistry.getInstrumentation().context
            val json = ctx.assets.open("golden_vectors.json").bufferedReader().use { it.readText() }
            return JSONObject(json)
        }
    }

    /** Rust → Kotlin: Kotlin decrypts every Rust-produced per-item ciphertext. */
    @Test
    fun kotlinDecryptsRustItemVectors() {
        val vectors = loadFixture().getJSONObject("item_aead").getJSONArray("vectors")
        for (i in 0 until vectors.length()) {
            val v = vectors.getJSONObject(i)
            val itemId = v.getString("item_id")
            val key = hexToBytes(v.getString("key_hex")).toUByteList()
            val nonce = hexToBytes(v.getString("nonce_hex")).toUByteList()
            val ciphertext = hexToBytes(v.getString("ciphertext_hex")).toUByteList()
            val expected = v.getString("plaintext_utf8").toByteArray(Charsets.UTF_8)
            // Default to key_version=1 for pre-4i2 fixture vectors (no field).
            val keyVersion: UByte = (if (v.has("key_version_u8")) v.getInt("key_version_u8") else 1).toUByte()

            val recovered = decryptText(itemId, ciphertext, nonce, key, keyVersion).toByteArray()
            assertArrayEquals(
                "Kotlin failed to decrypt Rust item vector '${v.getString("label")}'",
                expected,
                recovered,
            )
        }
    }

    /** Kotlin → Kotlin (real Rust core): encrypt then decrypt round-trips. */
    @Test
    fun kotlinEncryptRoundTripsThroughRustCore() {
        val vectors = loadFixture().getJSONObject("item_aead").getJSONArray("vectors")
        for (i in 0 until vectors.length()) {
            val v = vectors.getJSONObject(i)
            val itemId = "kotlin-roundtrip-${v.getString("label")}"
            val key = hexToBytes(v.getString("key_hex")).toUByteList()
            val plaintext = v.getString("plaintext_utf8").toByteArray(Charsets.UTF_8)
            // Use key_version=2 for new items (ITEM_KEY_VERSION_CURRENT=2).
            val keyVersion: UByte = 2u

            val blob: EncryptedBlob = encryptText(itemId, plaintext.toUByteList(), key, keyVersion)
            val recovered = decryptText(itemId, blob.ciphertext, blob.nonce, key, keyVersion).toByteArray()
            assertArrayEquals(
                "Kotlin encrypt→decrypt round-trip failed for '${v.getString("label")}'",
                plaintext,
                recovered,
            )
        }
    }

    /**
     * Argon2id determinism across languages: Kotlin re-derives the sync key from
     * the same passphrase and must match the Rust-recorded key byte-for-byte.
     */
    @Test
    fun kotlinDerivesIdenticalSyncKey() {
        val cloud = loadFixture().getJSONObject("cloud_aead")
        val passphrase = cloud.getString("passphrase_utf8")
        val expectedKey = hexToBytes(cloud.getString("sync_key_hex"))

        val derived = deriveCloudSyncKey(passphrase).toByteArray()
        assertArrayEquals(
            "Kotlin Argon2id derivation diverged from Rust",
            expectedKey,
            derived,
        )
    }

    /** Rust → Kotlin: Kotlin decrypts every Rust-produced cloud blob. */
    @Test
    fun kotlinDecryptsRustCloudVectors() {
        val cloud = loadFixture().getJSONObject("cloud_aead")
        val syncKey = hexToBytes(cloud.getString("sync_key_hex")).toUByteList()
        val vectors = cloud.getJSONArray("vectors")
        for (i in 0 until vectors.length()) {
            val v = vectors.getJSONObject(i)
            val itemId = v.getString("item_id")
            val blob = hexToBytes(v.getString("blob_hex")).toUByteList()
            val expected = v.getString("plaintext_utf8").toByteArray(Charsets.UTF_8)

            val recovered = cloudDecrypt(itemId, blob, syncKey).toByteArray()
            assertArrayEquals(
                "Kotlin failed to decrypt Rust cloud vector '${v.getString("label")}'",
                expected,
                recovered,
            )
        }
    }

    /** Kotlin → Kotlin cloud path round-trip through the real Rust core. */
    @Test
    fun kotlinCloudEncryptRoundTrips() {
        val cloud = loadFixture().getJSONObject("cloud_aead")
        val syncKey = hexToBytes(cloud.getString("sync_key_hex")).toUByteList()
        val itemId = "kotlin-cloud-roundtrip"
        val plaintext = "round-trip ✓ payload".toByteArray(Charsets.UTF_8)

        val blob = cloudEncrypt(itemId, plaintext.toUByteList(), syncKey)
        val recovered = cloudDecrypt(itemId, blob, syncKey).toByteArray()
        assertArrayEquals(plaintext, recovered)

        // Sanity: total length contract is nonce(24)+plaintext+tag(16).
        assertEquals(24 + plaintext.size + 16, blob.size)
    }
}
