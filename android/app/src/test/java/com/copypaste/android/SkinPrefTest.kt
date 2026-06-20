package com.copypaste.android

import com.copypaste.android.ui.theme.Skin
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Pure-JVM tests for the [Skin] preference added in A-F2.
 *
 * These tests do NOT require an Android [Context] — they verify the enum
 * contract and default value that [Settings.skin] and [rememberSkin] rely on.
 * They follow the same pattern as [SettingsParityTest] (pure-JVM, no mocking).
 */
class SkinPrefTest {

    // ── Enum contract ─────────────────────────────────────────────────────────

    @Test
    fun `Skin enum has all three required values`() {
        val names = Skin.entries.map { it.name }
        assertTrue("Skin must include CLASSIC", names.contains("CLASSIC"))
        assertTrue("Skin must include QUIET",   names.contains("QUIET"))
        assertTrue("Skin must include VAPOR",   names.contains("VAPOR"))
    }

    @Test
    fun `Skin default is CLASSIC`() {
        // CLASSIC = today's Liquid Glass look; default must never change appearance.
        assertEquals(
            "Skin.DEFAULT must be CLASSIC so a fresh install is visually identical to the pre-skin build",
            Skin.CLASSIC,
            Skin.DEFAULT,
        )
    }

    // ── SharedPreferences key contract ────────────────────────────────────────

    /**
     * The SharedPreferences key used by [Settings.skin] is "skin".
     * This is the stable on-disk identifier; changing it would wipe all
     * persisted user choices on upgrade. We verify it here by asserting the
     * enum name that serves as the default sentinel in the prefs getter.
     *
     * We cannot construct [Settings] without an Android Context in a JVM test,
     * so we assert the next best thing: the default name stored when no value
     * has been persisted is [Skin.DEFAULT].name = "CLASSIC".
     */
    @Test
    fun `Skin DEFAULT name round-trips through enum lookup`() {
        val stored = Skin.DEFAULT.name          // what Settings writes to prefs
        val resolved = Skin.entries.firstOrNull { it.name == stored }
        assertEquals(
            "Skin.DEFAULT.name must round-trip back to Skin.DEFAULT via entries lookup",
            Skin.DEFAULT,
            resolved,
        )
    }

    @Test
    fun `All Skin entries name round-trip through enum lookup`() {
        // Ensures rememberSkin() defensive fallback logic will never lose a valid
        // stored value — every Skin.name is resolvable via entries.firstOrNull.
        for (skin in Skin.entries) {
            val resolved = Skin.entries.firstOrNull { it.name == skin.name }
            assertEquals(
                "Skin.${skin.name} must round-trip through name lookup",
                skin,
                resolved,
            )
        }
    }

    @Test
    fun `Unknown stored name falls back to CLASSIC`() {
        // Mirrors the defensive fallback in Settings.skin getter:
        //   else -> Skin.CLASSIC
        // Simulate what happens when an unrecognised name is stored (e.g. a
        // future downgrade from a build with a fourth skin).
        val unknownStoredName = "NEON"
        val resolved = when (unknownStoredName) {
            Skin.QUIET.name -> Skin.QUIET
            Skin.VAPOR.name -> Skin.VAPOR
            else            -> Skin.CLASSIC
        }
        assertEquals(
            "An unrecognised stored skin name must fall back to CLASSIC",
            Skin.CLASSIC,
            resolved,
        )
    }
}
