package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Pure-JVM parity tests for CopyPaste-0w8z: the Kotlin [fallbackDefaultConfig]
 * literals in [CopypasteBindings.kt] MUST stay in sync with the Rust constants in
 * `crates/copypaste-core/src/config/defaults.rs`.
 *
 * In JVM unit-test context the native .so is not loaded, so [defaultConfig] always
 * delegates to [fallbackDefaultConfig].  This test verifies every numeric/boolean
 * field matches the canonical Rust constant so a future change to defaults.rs is
 * not silently missed on the Kotlin side.
 *
 * Rust constants (crates/copypaste-core/src/config/defaults.rs) vs Kotlin fallback:
 *   MAX_TEXT_SIZE_BYTES      = 10 * 1024 * 1024        (10 MiB)
 *   MAX_IMAGE_SIZE_BYTES     = 64 * 1024 * 1024        (64 MiB)
 *   MAX_FILE_SIZE_BYTES      = 100 * 1024 * 1024       (100 MiB)
 *   STORAGE_QUOTA_BYTES      = 10 * 1024 * 1024 * 1024 (10 GiB)
 *   SENSITIVE_TTL_SECS       = 30
 *   POLL_INTERVAL_MS         = 500
 *   IMAGE_QUALITY            = 100
 *
 * Booleans:
 *   soundOnCopy              = true   (notify-on-copy default)
 *   notifyOnCopy             = true
 *   maskSensitiveContent     = true
 *   syncOnWifiOnly           = false
 *   p2pEnabled               = false  (P2P defaults off on Android — not in defaults.rs,
 *                                      Android-specific safe default)
 *   collectPublicIp          = true
 *   pasteAsPlainText         = false
 *   excludedAppBundleIds     = emptyList()
 *
 * Run with: ./gradlew :app:testDebugUnitTest
 */
class FallbackDefaultConfigParityTest {

    // In JVM tests isNativeLibraryLoaded == false, so defaultConfig() == fallbackDefaultConfig().
    private val cfg by lazy { defaultConfig() }

    // ── Size caps (ULong → Long comparison via toLong()) ───────────────────────

    @Test
    fun `maxTextSizeBytes matches Rust MAX_TEXT_SIZE_BYTES 10 MiB`() {
        val expected = 10L * 1024 * 1024
        assertEquals(
            "maxTextSizeBytes must be 10 MiB (Rust MAX_TEXT_SIZE_BYTES = 10*1024*1024) — " +
                "update fallbackDefaultConfig() in CopypasteBindings.kt if defaults.rs changes",
            expected,
            cfg.maxTextSizeBytes.toLong(),
        )
    }

    @Test
    fun `maxImageSizeBytes matches Rust MAX_IMAGE_SIZE_BYTES 64 MiB`() {
        val expected = 64L * 1024 * 1024
        assertEquals(
            "maxImageSizeBytes must be 64 MiB (Rust MAX_IMAGE_SIZE_BYTES = 64*1024*1024) — " +
                "update fallbackDefaultConfig() in CopypasteBindings.kt if defaults.rs changes",
            expected,
            cfg.maxImageSizeBytes.toLong(),
        )
    }

    @Test
    fun `maxFileSizeBytes matches Rust MAX_FILE_SIZE_BYTES 100 MiB`() {
        val expected = 100L * 1024 * 1024
        assertEquals(
            "maxFileSizeBytes must be 100 MiB (Rust MAX_FILE_SIZE_BYTES = 100*1024*1024) — " +
                "update fallbackDefaultConfig() in CopypasteBindings.kt if defaults.rs changes",
            expected,
            cfg.maxFileSizeBytes.toLong(),
        )
    }

    @Test
    fun `storageQuotaBytes matches Rust STORAGE_QUOTA_BYTES 10 GiB`() {
        val expected = 10L * 1024 * 1024 * 1024
        assertEquals(
            "storageQuotaBytes must be 10 GiB (Rust STORAGE_QUOTA_BYTES = 10*1024*1024*1024) — " +
                "update fallbackDefaultConfig() in CopypasteBindings.kt if defaults.rs changes",
            expected,
            cfg.storageQuotaBytes.toLong(),
        )
    }

    // ── TTL / intervals ────────────────────────────────────────────────────────

    @Test
    fun `sensitiveTtlSecs matches Rust SENSITIVE_TTL_SECS 30`() {
        assertEquals(
            "sensitiveTtlSecs must be 30s (Rust SENSITIVE_TTL_SECS = 30) — " +
                "update fallbackDefaultConfig() in CopypasteBindings.kt if defaults.rs changes",
            30L,
            cfg.sensitiveTtlSecs.toLong(),
        )
    }

    @Test
    fun `pollIntervalMs matches Rust POLL_INTERVAL_MS 500`() {
        assertEquals(
            "pollIntervalMs must be 500ms (Rust POLL_INTERVAL_MS = 500) — " +
                "update fallbackDefaultConfig() in CopypasteBindings.kt if defaults.rs changes",
            500L,
            cfg.pollIntervalMs.toLong(),
        )
    }

    // ── Image quality ──────────────────────────────────────────────────────────

    @Test
    fun `imageQuality matches Rust IMAGE_QUALITY 100`() {
        assertEquals(
            "imageQuality must be 100 (Rust IMAGE_QUALITY = 100, lossless default) — " +
                "update fallbackDefaultConfig() in CopypasteBindings.kt if defaults.rs changes",
            100,
            cfg.imageQuality.toInt(),
        )
    }

    // ── Boolean defaults ────────────────────────────────────────────────────────

    @Test
    fun `soundOnCopy defaults to true`() {
        assertTrue("soundOnCopy must default to true", cfg.soundOnCopy)
    }

    @Test
    fun `notifyOnCopy defaults to true`() {
        assertTrue("notifyOnCopy must default to true", cfg.notifyOnCopy)
    }

    @Test
    fun `maskSensitiveContent defaults to true`() {
        assertTrue("maskSensitiveContent must default to true", cfg.maskSensitiveContent)
    }

    @Test
    fun `syncOnWifiOnly defaults to false`() {
        assertFalse("syncOnWifiOnly must default to false (sync on any network)", cfg.syncOnWifiOnly)
    }

    @Test
    fun `p2pEnabled defaults to false on Android`() {
        assertFalse(
            "p2pEnabled must default to false (Android safe default — P2P is opt-in)",
            cfg.p2pEnabled,
        )
    }

    @Test
    fun `collectPublicIp defaults to true`() {
        assertTrue("collectPublicIp must default to true", cfg.collectPublicIp)
    }

    @Test
    fun `pasteAsPlainText defaults to false`() {
        assertFalse("pasteAsPlainText must default to false (rich paste by default)", cfg.pasteAsPlainText)
    }

    @Test
    fun `excludedAppBundleIds defaults to empty list`() {
        assertTrue(
            "excludedAppBundleIds must default to empty (Rust DEFAULT_EXCLUDED_APP_BUNDLE_IDS = [])",
            cfg.excludedAppBundleIds.isEmpty(),
        )
    }

    // ── Guard: ensure we are using the Kotlin fallback (no .so in JVM tests) ──

    /**
     * Confirm the native library is NOT loaded in JVM unit tests so the above
     * assertions actually exercise [fallbackDefaultConfig] rather than the native
     * Rust [defaultConfig]. If this test fails it means the .so was loaded in JVM
     * context (should never happen) and the test suite would need updating.
     */
    @Test
    fun `native library is not loaded in JVM unit tests — fallback is exercised`() {
        assertFalse(
            "isNativeLibraryLoaded must be false in JVM unit tests — " +
                "defaultConfig() must be delegating to fallbackDefaultConfig()",
            isNativeLibraryLoaded,
        )
    }
}
