package com.copypaste.android

import android.content.Context
import android.content.ContextWrapper
import android.content.SharedPreferences
import com.copypaste.android.ui.theme.AccentColor
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Pure-JVM tests for the two-axis theme system (Phase 7, CopyPaste-2hfj.8).
 *
 * Covers:
 *  1. AccentColor.fromName() round-trip for all six hues.
 *  2. fromName() fallback for null / unknown / old-skin names.
 *  3. migrateThemeForTwoAxis() removes stale Liquid-Glass keys via FakePrefs.
 *
 * Uses a HashMap-backed FakeSharedPreferences so no Robolectric / Mockito needed.
 */
class AccentRoundTripTest {

    // ── Pure Kotlin: AccentColor.fromName round-trip ──────────────────────────

    @Test
    fun `fromName round-trips all six accent variants`() {
        for (accent in AccentColor.entries) {
            val result = AccentColor.fromName(accent.name)
            assertEquals(
                "AccentColor.fromName(\"${accent.name}\") should return ${accent.name}",
                accent,
                result,
            )
        }
    }

    @Test
    fun `fromName returns DEFAULT for null`() {
        assertEquals(
            "fromName(null) must return DEFAULT (${AccentColor.DEFAULT})",
            AccentColor.DEFAULT,
            AccentColor.fromName(null),
        )
    }

    @Test
    fun `fromName returns DEFAULT for unknown string`() {
        assertEquals(
            "fromName(\"vapor\") must return DEFAULT (old skin name — not a valid accent)",
            AccentColor.DEFAULT,
            AccentColor.fromName("vapor"),
        )
        assertEquals(
            "fromName(\"graphite-mist\") must return DEFAULT (old palette name)",
            AccentColor.DEFAULT,
            AccentColor.fromName("graphite-mist"),
        )
    }

    @Test
    fun `fromName is case-sensitive — lowercase does not match`() {
        // Enum names are stored upper-case ("INDIGO"); a lowercase value
        // from pre-migration storage must fall back to DEFAULT, not crash.
        assertEquals(
            "fromName(\"indigo\") (lowercase) must fall back to DEFAULT",
            AccentColor.DEFAULT,
            AccentColor.fromName("indigo"),
        )
    }

    @Test
    fun `DEFAULT accent is INDIGO per STYLEGUIDE section 2`() {
        assertEquals(
            "DEFAULT accent must be INDIGO per STYLEGUIDE §2",
            AccentColor.INDIGO,
            AccentColor.DEFAULT,
        )
    }

    @Test
    fun `all six expected AccentColor entries exist`() {
        val names = AccentColor.entries.map { it.name }.toSet()
        listOf("INDIGO", "BLUE", "TEAL", "GREEN", "AMBER", "ROSE").forEach { expected ->
            assertTrue(
                "AccentColor must contain variant $expected (STYLEGUIDE §2); found: $names",
                names.contains(expected),
            )
        }
        assertEquals("AccentColor must have exactly 6 variants", 6, AccentColor.entries.size)
    }

    // ── migrateThemeForTwoAxis via FakePrefs (no Robolectric / Mockito) ────────

    /**
     * A minimal HashMap-backed implementation of SharedPreferences and its Editor.
     * The Android framework classes are available as stubs on the unit-test classpath;
     * implementing the interface allows testing migration logic without Robolectric.
     */
    private class FakePrefs : SharedPreferences {
        private val map: MutableMap<String, Any?> = mutableMapOf()

        inner class FakeEditor : SharedPreferences.Editor {
            private val ops: MutableMap<String, Any?> = mutableMapOf()
            private val removes: MutableSet<String> = mutableSetOf()
            private var clearAll = false

            override fun putString(key: String, value: String?) = apply { ops[key] = value }
            override fun putStringSet(key: String, values: MutableSet<String>?) = apply { ops[key] = values }
            override fun putInt(key: String, value: Int) = apply { ops[key] = value }
            override fun putLong(key: String, value: Long) = apply { ops[key] = value }
            override fun putFloat(key: String, value: Float) = apply { ops[key] = value }
            override fun putBoolean(key: String, value: Boolean) = apply { ops[key] = value }
            override fun remove(key: String) = apply { removes.add(key) }
            override fun clear() = apply { clearAll = true }

            override fun commit(): Boolean { flush(); return true }
            override fun apply() { flush() }

            private fun flush() {
                if (clearAll) map.clear()
                for (k in removes) map.remove(k)
                map.putAll(ops)
            }
        }

        override fun edit(): SharedPreferences.Editor = FakeEditor()
        override fun contains(key: String): Boolean = map.containsKey(key)
        override fun getAll(): MutableMap<String, *> = map.toMutableMap()
        override fun getString(key: String, defValue: String?): String? = map[key] as? String ?: defValue
        override fun getStringSet(key: String, defValues: MutableSet<String>?): MutableSet<String>? =
            @Suppress("UNCHECKED_CAST") (map[key] as? MutableSet<String>) ?: defValues
        override fun getInt(key: String, defValue: Int): Int = (map[key] as? Int) ?: defValue
        override fun getLong(key: String, defValue: Long): Long = (map[key] as? Long) ?: defValue
        override fun getFloat(key: String, defValue: Float): Float = (map[key] as? Float) ?: defValue
        override fun getBoolean(key: String, defValue: Boolean): Boolean = (map[key] as? Boolean) ?: defValue
        override fun registerOnSharedPreferenceChangeListener(l: SharedPreferences.OnSharedPreferenceChangeListener) {}
        override fun unregisterOnSharedPreferenceChangeListener(l: SharedPreferences.OnSharedPreferenceChangeListener) {}
    }

    /**
     * Minimal Context stub that returns our FakePrefs for any prefs name.
     *
     * Extends ContextWrapper(null) rather than Context directly so we only
     * need to override the two methods that Settings actually calls — the rest
     * of the Context API is provided by ContextWrapper's concrete stubs.
     */
    private class FakeContext(private val prefs: FakePrefs) : ContextWrapper(null) {
        override fun getApplicationContext(): Context = this
        override fun getSharedPreferences(name: String, mode: Int): SharedPreferences = prefs
    }

    @Test
    fun `migrateThemeForTwoAxis removes palette skin density motion contrast keys`() {
        val prefs = FakePrefs()
        val ctx = FakeContext(prefs)

        // Seed stale Liquid-Glass keys
        prefs.edit()
            .putString("palette", "graphite-mist")
            .putString("skin", "classic")
            .putString("density", "comfortable")
            .putBoolean("motion_reduced", false)
            .putString("contrast", "balanced")
            .apply()

        val settings = Settings(ctx)
        settings.migrateThemeForTwoAxis()

        assertFalse("palette key must be removed after migration", prefs.contains("palette"))
        assertFalse("skin key must be removed after migration",    prefs.contains("skin"))
        assertFalse("density key must be removed after migration", prefs.contains("density"))
        assertFalse("motion_reduced key must be removed",          prefs.contains("motion_reduced"))
        assertFalse("contrast key must be removed after migration", prefs.contains("contrast"))

        assertTrue(
            "theme_migrated_2axis latch must be set so migration does not run twice",
            prefs.getBoolean("theme_migrated_2axis", false),
        )
    }

    @Test
    fun `migrateThemeForTwoAxis is idempotent when already migrated`() {
        val prefs = FakePrefs()
        val ctx = FakeContext(prefs)

        // Pre-set the latch so migration should be a no-op
        prefs.edit().putBoolean("theme_migrated_2axis", true).apply()
        prefs.edit().putString("palette", "old-value").apply()

        Settings(ctx).migrateThemeForTwoAxis()

        // palette must NOT be removed because migration was already done
        assertTrue(
            "palette key must remain when migration was already applied",
            prefs.contains("palette"),
        )
    }
}
