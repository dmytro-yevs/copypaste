package com.copypaste.android

import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * CopyPaste-crh3.34: Unit tests for the "Revoke all" feature on the Android
 * Devices screen — parity with macOS DevicesView "Revoke all" button.
 *
 * These are pure JVM tests — no Android SDK, Compose runtime, or emulator needed.
 */
class RevokeAllDevicesTest {

    // ── revokeAllEnabled ──────────────────────────────────────────────────────

    @Test
    fun `revokeAllEnabled returns false when peer count is zero`() {
        assertFalse(
            "Button must be disabled when there are no paired peers",
            revokeAllEnabled(peerCount = 0),
        )
    }

    @Test
    fun `revokeAllEnabled returns true when at least one peer exists`() {
        assertTrue(
            "Button must be enabled when there is one peer",
            revokeAllEnabled(peerCount = 1),
        )
    }

    @Test
    fun `revokeAllEnabled returns true for multiple peers`() {
        assertTrue(
            "Button must be enabled when there are multiple peers",
            revokeAllEnabled(peerCount = 5),
        )
    }

    // ── revokeAllConfirmBody ──────────────────────────────────────────────────

    @Test
    fun `revokeAllConfirmBody contains parity trust-break copy`() {
        val body = revokeAllConfirmBody()
        assertTrue(
            "Confirm body must mention breaking trust with paired devices",
            body.contains("break trust") || body.contains("immediately"),
        )
    }

    @Test
    fun `revokeAllConfirmBody mentions re-pair requirement`() {
        val body = revokeAllConfirmBody()
        assertTrue(
            "Confirm body must tell the user devices need to re-pair",
            body.contains("re-pair"),
        )
    }

    // ── Source-level guard: DevicesActivity must wire up the Revoke-all button ─

    @Test
    fun `DevicesActivity source contains Revoke all action`() {
        val moduleRoot = locateModuleRoot()
        val source = java.io.File(
            moduleRoot,
            "src/main/java/com/copypaste/android/DevicesActivity.kt",
        ).readText()

        assertTrue(
            "DevicesActivity must have a 'Revoke all' button (crh3.34)",
            source.contains("Revoke all"),
        )
    }

    @Test
    fun `DevicesActivity source calls revokeAllEnabled`() {
        val moduleRoot = locateModuleRoot()
        val source = java.io.File(
            moduleRoot,
            "src/main/java/com/copypaste/android/DevicesActivity.kt",
        ).readText()

        assertTrue(
            "DevicesActivity must use revokeAllEnabled() to gate the button (crh3.34)",
            source.contains("revokeAllEnabled"),
        )
    }

    @Test
    fun `DevicesUtils source contains revokeAllEnabled helper`() {
        val moduleRoot = locateModuleRoot()
        val source = java.io.File(
            moduleRoot,
            "src/main/java/com/copypaste/android/DevicesUtils.kt",
        ).readText()

        assertTrue(
            "DevicesUtils must define revokeAllEnabled() (crh3.34)",
            source.contains("fun revokeAllEnabled"),
        )
    }

    @Test
    fun `DevicesUtils source contains revokeAllConfirmBody helper`() {
        val moduleRoot = locateModuleRoot()
        val source = java.io.File(
            moduleRoot,
            "src/main/java/com/copypaste/android/DevicesUtils.kt",
        ).readText()

        assertTrue(
            "DevicesUtils must define revokeAllConfirmBody() (crh3.34)",
            source.contains("fun revokeAllConfirmBody"),
        )
    }

    // ── helper ────────────────────────────────────────────────────────────────

    private fun locateModuleRoot(): java.io.File {
        val anchor = RevokeAllDevicesTest::class.java
            .protectionDomain?.codeSource?.location?.toURI()
            ?.let { java.io.File(it) }
        var dir: java.io.File? = anchor
        while (dir != null) {
            if (java.io.File(dir, "src/main").exists()) return dir
            dir = dir.parentFile
        }
        error("Could not locate module root from $anchor")
    }
}
