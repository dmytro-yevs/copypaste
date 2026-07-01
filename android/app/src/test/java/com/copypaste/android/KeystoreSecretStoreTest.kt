package com.copypaste.android

import org.junit.After
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * CopyPaste-vp63.36: characterization tests for [KeystoreSecretStore]'s pure
 * wrap/unwrap-consuming logic (readWrappedSecret/writeWrappedSecret migration,
 * cloudSyncKeyDirect, encryptionKey generation/caching) via the injected
 * [FakeKekCipher] + [FakeBase64Codec] seams — AndroidKeyStore and
 * `android.util.Base64` are both out of scope for plain JUnit4 (no
 * Robolectric) tests in this module.
 */
class KeystoreSecretStoreTest {

    private fun store(
        prefs: FakeSharedPreferences = FakeSharedPreferences(),
        kek: KekCipher = FakeKekCipher(),
    ) = KeystoreSecretStore(prefs, kek, FakeBase64Codec)

    @After
    fun tearDown() {
        // The unwrapped encryption key is cached in a companion (process-wide,
        // i.e. JVM-static) field — reset it after every test so state does not
        // leak between test methods sharing the same test JVM.
        store().clearCachedKey()
    }

    @Test
    fun `relayToken round-trips through KEK wrap`() {
        val s = store()
        s.relayToken = "abc123deadbeef"
        assertEquals("abc123deadbeef", s.relayToken)
    }

    @Test
    fun `relayToken empty write clears wrapped and legacy keys`() {
        val prefs = FakeSharedPreferences()
        val s = store(prefs)
        s.relayToken = "secret-token"
        assertTrue(prefs.contains("relay_token_wrapped_b64"))

        s.relayToken = ""
        assertEquals("", s.relayToken)
        assertTrue(!prefs.contains("relay_token_wrapped_b64"))
        assertTrue(!prefs.contains("relay_token_iv_b64"))
    }

    @Test
    fun `legacy plaintext relay token migrates to wrapped form on first read`() {
        val prefs = FakeSharedPreferences()
        // Simulate a pre-upgrade install: plaintext value under the legacy key,
        // no wrapped blob yet.
        prefs.edit().putString("relay_token", "legacy-plain-token").apply()
        val s = store(prefs)

        val read = s.relayToken
        assertEquals("legacy-plain-token", read)
        // Migration must have scrubbed the legacy plaintext and written the
        // wrapped form.
        assertTrue(!prefs.contains("relay_token"))
        assertTrue(prefs.contains("relay_token_wrapped_b64"))
        assertEquals("legacy-plain-token", s.relayToken)
    }

    @Test
    fun `unwrappable secret returns empty string rather than throwing`() {
        val kek = FakeKekCipher()
        val s = store(kek = kek)
        s.cloudSyncPassphrase = "correct horse battery staple"

        kek.failNextUnwrap = true
        assertEquals("", s.cloudSyncPassphrase)
    }

    @Test
    fun `cloudSyncKeyDirect round-trips raw bytes`() {
        val s = store()
        val key = ByteArray(32) { it.toByte() }
        s.cloudSyncKeyDirect = key
        assertArrayEquals(key, s.cloudSyncKeyDirect)
    }

    @Test
    fun `cloudSyncKeyDirect is null when unset`() {
        assertNull(store().cloudSyncKeyDirect)
    }

    @Test
    fun `cloudSyncKeyDirect set to empty array clears it`() {
        val s = store()
        s.cloudSyncKeyDirect = ByteArray(32) { 1 }
        s.cloudSyncKeyDirect = ByteArray(0)
        assertNull(s.cloudSyncKeyDirect)
    }

    @Test
    fun `encryptionKey is 32 bytes and stable across reads`() {
        val s = store()
        val first = s.encryptionKey
        val second = s.encryptionKey
        assertEquals(32, first.size)
        assertArrayEquals(first, second)
    }

    @Test
    fun `encryptionKey handed out as a defensive copy`() {
        val s = store()
        val handed = s.encryptionKey
        handed[0] = (handed[0] + 1).toByte()
        // Mutating the returned array must not corrupt the cached master key.
        assertArrayEquals(s.encryptionKey, s.encryptionKey)
    }

    @Test
    fun `legacy plaintext encryption key migrates into wrapped form`() {
        val prefs = FakeSharedPreferences()
        val legacyKey = ByteArray(32) { it.toByte() }
        prefs.edit()
            .putString("encryption_key_b64", FakeBase64Codec.encode(legacyKey, 0))
            .apply()
        val s = store(prefs)

        assertArrayEquals(legacyKey, s.encryptionKey)
        assertTrue(!prefs.contains("encryption_key_b64"))
        assertTrue(prefs.contains("encryption_key_wrapped_b64"))
    }

    @Test(expected = EncryptionKeyLostException::class)
    fun `unwrappable wrapped encryption key throws instead of regenerating`() {
        val prefs = FakeSharedPreferences()
        val kek = FakeKekCipher()
        val s = store(prefs, kek)
        // Force a wrapped key to exist.
        s.encryptionKey
        s.clearCachedKey()

        kek.failNextUnwrap = true
        s.encryptionKey
    }
}
