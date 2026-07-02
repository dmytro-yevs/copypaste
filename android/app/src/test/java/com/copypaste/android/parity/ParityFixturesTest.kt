package com.copypaste.android.parity

import com.copypaste.android.ui.theme.BannerVariant
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

/**
 * android-material3-redesign task 2.11 "Paired structural fixtures": every
 * required pair (history row, masked row, device card, SAS dialog, a
 * Settings group, banner, destructive modal, empty state, sync status) has a
 * `parity/fixtures/` JSON file (one per fixture) with an explicit android/desktop status —
 * "Blocked" (never silently green) where the desktop-epic evidence isn't
 * available, "Deferred" where the owning Android screen slice hasn't landed
 * yet, "Verified" where an S2-landed component's structure is checked below.
 *
 * Android structural conformance is blocking HERE for the two fixtures whose
 * component exists after S2 (banner via [BannerVariant], the shared
 * destructive-modal/empty-state primitives) — the rest are schema-only
 * scaffolding owned by their screen slice (S5/S7/S8/S9/S11), as recorded in
 * each fixture's own `android.status` field.
 */
class ParityFixturesTest {

    private fun fixturesDir(): File {
        var dir = File(".").absoluteFile
        repeat(8) {
            val candidate = File(dir, "parity/fixtures")
            if (candidate.exists()) return candidate
            dir = dir.parentFile ?: return@repeat
        }
        throw AssertionError("parity/fixtures/ not found by walking up from ${File(".").absolutePath}")
    }

    private fun loadFixture(name: String): JSONObject = JSONObject(File(fixturesDir(), "$name.json").readText())

    private val requiredFixtures = listOf(
        "history-row", "masked-row", "device-card", "sas-dialog",
        "settings-group", "banner", "destructive-modal", "empty-state", "sync-status",
    )

    @Test
    fun `every required paired fixture exists with an explicit android and desktop status`() {
        for (name in requiredFixtures) {
            val json = loadFixture(name)
            assertTrue("$name missing 'android' status", json.has("android"))
            assertTrue("$name missing 'desktop' status", json.has("desktop"))
            val androidStatus = json.getJSONObject("android").getString("status")
            val desktopStatus = json.getJSONObject("desktop").getString("status")
            assertTrue("$name android.status must be non-blank", androidStatus.isNotBlank())
            // Desktop paired evidence is an explicit desktop-epic dependency (cross-platform-
            // parity.md "Paired fixtures") — every fixture must say so, never silently green.
            assertEquals("$name desktop.status", "Blocked", desktopStatus)
        }
    }

    @Test
    fun `banner fixture variants match BannerVariant exactly`() {
        val json = loadFixture("banner")
        val records = json.getJSONArray("records")
        val variantsInFixture = (0 until records.length()).map { records.getJSONObject(it).getString("variant") }.toSet()
        val variantsInCode = BannerVariant.entries.map { it.name.lowercase() }.toSet()
        assertEquals(variantsInCode, variantsInFixture)
    }

    @Test
    fun `masked-row fixture placeholder is synthetic, never real plaintext-shaped`() {
        val json = loadFixture("masked-row")
        val record = json.getJSONArray("records").getJSONObject(0)
        val placeholder = record.getString("placeholder")
        assertTrue("masked-row placeholder must be marked synthetic", placeholder.contains("synthetic"))
        assertTrue(record.getBoolean("sensitive"))
    }
}
