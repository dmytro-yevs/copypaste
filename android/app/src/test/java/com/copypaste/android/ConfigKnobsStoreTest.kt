package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * CopyPaste-vp63.36: characterization tests for [ConfigKnobsStore] — defaults
 * seeded from [defaultConfig] (JVM-fallback path, since no .so is loaded under
 * plain JUnit4) and the round-trip/clamp behavior of the size/quota/ttl knobs.
 */
class ConfigKnobsStoreTest {

    private fun store() = ConfigKnobsStore(FakeSharedPreferences())

    @Test
    fun `maxTextSizeBytes defaults to the native default when unset`() {
        val s = store()
        assertEquals(defaultConfig().maxTextSizeBytes.toLong(), s.maxTextSizeBytes)
    }

    @Test
    fun `maxTextSizeBytes round-trips a valid value`() {
        val s = store()
        s.maxTextSizeBytes = 123_456L
        assertEquals(123_456L, s.maxTextSizeBytes)
    }

    @Test
    fun `maxTextSizeBytes clamps a negative value to zero floor`() {
        val s = store()
        s.maxTextSizeBytes = -5L
        assertTrue(s.maxTextSizeBytes >= 0L)
    }

    @Test
    fun `sensitiveTtlSecs 0 auto-wipe-disabled sentinel survives the clamp`() {
        val s = store()
        s.sensitiveTtlSecs = 0L
        assertEquals(0L, s.sensitiveTtlSecs)
    }

    @Test
    fun `excludedAppBundleIds trims blanks and de-dups on write`() {
        val s = store()
        s.excludedAppBundleIds = listOf(" com.example.app ", "com.example.app", "", "  ")
        assertEquals(listOf("com.example.app"), s.excludedAppBundleIds)
    }

    @Test
    fun `excludedAppBundleIds defaults to the native default (empty) when unset`() {
        assertEquals(defaultConfig().excludedAppBundleIds, store().excludedAppBundleIds)
    }

    @Test
    fun `collectPublicIp and pasteAsPlainText round-trip independently`() {
        val s = store()
        s.collectPublicIp = false
        s.pasteAsPlainText = true
        assertEquals(false, s.collectPublicIp)
        assertEquals(true, s.pasteAsPlainText)
    }

    @Test
    fun `clampConfigForSave clamps only the three save-screen knobs, leaving others stored`() {
        val s = store()
        s.sensitiveTtlSecs = 77L
        val clamped = s.clampConfigForSave(
            maxTextSizeBytes = 1_000L,
            maxImageSizeBytes = 2_000L,
            storageQuotaBytes = 3_000L,
        )
        assertEquals(1_000L, clamped.maxTextSizeBytes.toLong())
        assertEquals(2_000L, clamped.maxImageSizeBytes.toLong())
        assertEquals(3_000L, clamped.storageQuotaBytes.toLong())
        // sensitiveTtlSecs is not one of the three save-screen knobs — clampConfigForSave
        // must read it from the currently stored value (77), not silently reset it.
        assertEquals(77L, clamped.sensitiveTtlSecs.toLong())
    }
}
