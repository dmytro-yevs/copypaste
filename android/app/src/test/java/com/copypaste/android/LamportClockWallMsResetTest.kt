package com.copypaste.android

import android.content.SharedPreferences
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Regression tests for CopyPaste-nv7t: LamportClock must reset stored wall-millis
 * values on startup instead of loading them as the logical clock value.
 *
 * Old Android builds stored System.currentTimeMillis() (~1.7 × 10^12) as the
 * lamport_ts. When the app restarts it loaded that huge value, making every
 * subsequent tick() produce 1.7e12+1, 1.7e12+2, ... which always beats macOS
 * Lamport clocks in the thousands range — biasing all LWW races in favour of
 * Android permanently.
 *
 * Fix: if the persisted value is >= WALL_MS_THRESHOLD (1_000_000_000L ≈ Jan 2001
 * epoch), it was written by an old build and must be discarded (reset to 0) on
 * construction.
 */
class LamportClockWallMsResetTest {

    // ── Helpers ──────────────────────────────────────────────────────────────

    /**
     * Minimal in-memory SharedPreferences stub — stores only Long values, which
     * is all LamportClock needs for its persistence calls.
     */
    private class FakePrefs(private val initial: Map<String, Long> = emptyMap()) :
        SharedPreferences {
        private val map = initial.toMutableMap()

        override fun getLong(key: String, defValue: Long): Long = map[key] ?: defValue
        override fun edit(): SharedPreferences.Editor = Editor()

        inner class Editor : SharedPreferences.Editor {
            private val pending = mutableMapOf<String, Long>()
            override fun putLong(key: String, value: Long) = apply { pending[key] = value }
            override fun apply() { map.putAll(pending) }
            override fun commit(): Boolean { map.putAll(pending); return true }
            // Unused stubs:
            override fun putString(k: String, v: String?) = this
            override fun putStringSet(k: String, v: MutableSet<String>?) = this
            override fun putInt(k: String, v: Int) = this
            override fun putFloat(k: String, v: Float) = this
            override fun putBoolean(k: String, v: Boolean) = this
            override fun remove(k: String) = this
            override fun clear() = this
        }

        // Remaining SharedPreferences methods — not used by LamportClock.
        override fun getAll() = emptyMap<String, Any>()
        override fun getString(k: String, d: String?) = d
        override fun getStringSet(k: String, d: MutableSet<String>?) = d
        override fun getInt(k: String, d: Int) = d
        override fun getFloat(k: String, d: Float) = d
        override fun getBoolean(k: String, d: Boolean) = d
        override fun contains(k: String) = false
        override fun registerOnSharedPreferenceChangeListener(l: SharedPreferences.OnSharedPreferenceChangeListener) {}
        override fun unregisterOnSharedPreferenceChangeListener(l: SharedPreferences.OnSharedPreferenceChangeListener) {}
    }

    private fun clockWithStoredValue(stored: Long): LamportClock {
        val prefs = FakePrefs(mapOf(LamportClock.PREF_KEY_LAMPORT_CLOCK to stored))
        return LamportClock(prefs)
    }

    // ── Core regression: wall-millis reset on construction ───────────────────

    @Test
    fun wallMsValue_isResetToZeroOnConstruction() {
        // ~2024 wall time in milliseconds — typical value stored by old builds.
        val wallMs = 1_718_000_000_000L // approx June 2024 epoch ms
        val clock = clockWithStoredValue(wallMs)
        assertEquals(
            "LamportClock must reset stored wall-millis to 0 on construction",
            0L, clock.get(),
        )
    }

    @Test
    fun thresholdBoundary_exactlyAtThreshold_isReset() {
        // Exactly at the wall-millis detection threshold — must be reset.
        val clock = clockWithStoredValue(LamportClock.WALL_MS_THRESHOLD)
        assertEquals(0L, clock.get())
    }

    @Test
    fun justBelowThreshold_isPreserved() {
        // A purely logical counter just below the threshold: must be preserved.
        val logical = LamportClock.WALL_MS_THRESHOLD - 1L
        val clock = clockWithStoredValue(logical)
        assertEquals(
            "Logical Lamport values below WALL_MS_THRESHOLD must be loaded as-is",
            logical, clock.get(),
        )
    }

    @Test
    fun zero_isPreserved() {
        val clock = clockWithStoredValue(0L)
        assertEquals(0L, clock.get())
    }

    @Test
    fun largeLogicalValue_justBelowThreshold_isPreserved() {
        val logical = LamportClock.WALL_MS_THRESHOLD - 1000L
        val clock = clockWithStoredValue(logical)
        assertEquals(logical, clock.get())
    }

    // ── After reset, tick/observe produce small logical values ───────────────

    @Test
    fun afterWallMsReset_tickProducesOne() {
        val clock = clockWithStoredValue(1_700_000_000_000L)
        val ts = clock.tick()
        assertEquals("After wall-ms reset, first tick must return 1", 1L, ts)
    }

    @Test
    fun afterWallMsReset_observeSmallIncoming_producesIncomingPlusOne() {
        val clock = clockWithStoredValue(1_700_000_000_000L)
        val ts = clock.observe(5L)
        assertEquals("After reset, observe(5) must return 6", 6L, ts)
    }

    @Test
    fun afterWallMsReset_clockCompetesWithMacOsValues() {
        // macOS logical Lamport values are typically in the low thousands.
        // After reset, Android must produce values in the same range, not 1.7e12+N.
        val clock = clockWithStoredValue(1_718_000_000_000L)
        repeat(10) { clock.tick() }
        assertTrue(
            "After 10 ticks from reset, clock must be << 1_000_000 (not wall-ms-biased)",
            clock.get() < 1_000_000L,
        )
    }

    // ── Clean install: value starts at 0 ─────────────────────────────────────

    @Test
    fun freshClock_noStoredValue_startsAtZero() {
        val prefs = FakePrefs() // no stored value → getLong returns 0
        val clock = LamportClock(prefs)
        assertEquals(0L, clock.get())
    }

    // ── Monotonicity is preserved for valid logical values ────────────────────

    @Test
    fun validLogicalValue_tickIsMonotonic() {
        val clock = clockWithStoredValue(42L)
        val t1 = clock.tick()
        val t2 = clock.tick()
        assertTrue("tick must be strictly monotonic", t2 > t1)
        assertEquals(43L, t1)
        assertEquals(44L, t2)
    }
}
