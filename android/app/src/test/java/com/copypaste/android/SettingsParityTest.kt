package com.copypaste.android

import com.copypaste.android.ui.theme.FILE_SIZE_STEP_VALUES
import com.copypaste.android.ui.theme.FILE_SIZE_STEP_LABELS
import com.copypaste.android.ui.theme.MAX_ITEMS_STEP_VALUES
import com.copypaste.android.ui.theme.MAX_ITEMS_STEP_LABELS
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * Pure-JVM tests that verify §6/§10 Settings parity constants and invariants
 * (CopyPaste-7ar).
 *
 * Tests are intentionally written BEFORE the implementation; they will fail
 * until the corresponding production code is in place.
 */
class SettingsParityTest {

    // ── File-size slider (§1) ─────────────────────────────────────────────────

    /**
     * The Rust core clamps max_file_size_bytes to MAX_FILE_BYTES = 100 MiB.
     * All step values in FILE_SIZE_STEP_VALUES MUST stay at or below that cap
     * so clampConfig never silently snaps the user's chosen value to a
     * different step. Values must be in ascending order.
     */
    @Test
    fun `FILE_SIZE_STEP_VALUES all values within 100 MiB core ceiling`() {
        val coreHardCap = 100L * 1024 * 1024
        FILE_SIZE_STEP_VALUES.forEach { v ->
            assertTrue(
                "FILE_SIZE_STEP value $v exceeds core hard cap of 100 MiB ($coreHardCap bytes)",
                v <= coreHardCap,
            )
        }
    }

    @Test
    fun `FILE_SIZE_STEP_VALUES top step equals exactly 100 MiB`() {
        val expected = 100L * 1024 * 1024
        assertEquals(
            "Top step must be 100 MiB (core hard cap)",
            expected,
            FILE_SIZE_STEP_VALUES.last(),
        )
    }

    @Test
    fun `FILE_SIZE_STEP_LABELS top label ends with (max)`() {
        assertTrue(
            "Last FILE_SIZE label must end with (max); got: ${FILE_SIZE_STEP_LABELS.last()}",
            FILE_SIZE_STEP_LABELS.last().contains("(max)", ignoreCase = true),
        )
    }

    @Test
    fun `FILE_SIZE_STEP_VALUES has at least 4 steps`() {
        assertTrue(
            "FILE_SIZE_STEP_VALUES should have ≥ 4 steps for useful granularity; got ${FILE_SIZE_STEP_VALUES.size}",
            FILE_SIZE_STEP_VALUES.size >= 4,
        )
    }

    @Test
    fun `FILE_SIZE arrays are same length`() {
        assertEquals(
            "FILE_SIZE_STEP_VALUES and FILE_SIZE_STEP_LABELS must be same length",
            FILE_SIZE_STEP_VALUES.size,
            FILE_SIZE_STEP_LABELS.size,
        )
    }

    // ── Max-items slider (§2) ─────────────────────────────────────────────────

    /**
     * Spec: [100, 250, 500, 1000, 2500, 5000, 10000, 100000] — last entry is the
     * Unlimited sentinel matching HISTORY_LIMIT in defaults.rs (100_000).
     */
    @Test
    fun `MAX_ITEMS_STEP_VALUES matches spec array`() {
        val expected = longArrayOf(100, 250, 500, 1_000, 2_500, 5_000, 10_000, 100_000)
        assertEquals(
            "MAX_ITEMS_STEP_VALUES must match spec [100,250,500,1000,2500,5000,10000,100000]",
            expected.toList(),
            MAX_ITEMS_STEP_VALUES.toList(),
        )
    }

    @Test
    fun `MAX_ITEMS_STEP_VALUES sentinel is 100000`() {
        assertEquals(
            "Unlimited sentinel must be 100000 (matches HISTORY_LIMIT in defaults.rs)",
            100_000L,
            MAX_ITEMS_STEP_VALUES.last(),
        )
    }

    @Test
    fun `MAX_ITEMS_STEP_LABELS last entry is Unlimited`() {
        assertEquals(
            "Last MAX_ITEMS label must be Unlimited",
            "Unlimited",
            MAX_ITEMS_STEP_LABELS.last(),
        )
    }

    @Test
    fun `MAX_ITEMS arrays are same length`() {
        assertEquals(
            "MAX_ITEMS_STEP_VALUES and MAX_ITEMS_STEP_LABELS must be same length",
            MAX_ITEMS_STEP_VALUES.size,
            MAX_ITEMS_STEP_LABELS.size,
        )
    }

    @Test
    fun `MAX_ITEMS_STEP_VALUES are ascending`() {
        for (i in 1 until MAX_ITEMS_STEP_VALUES.size) {
            assertTrue(
                "MAX_ITEMS_STEP_VALUES must be strictly ascending at index $i",
                MAX_ITEMS_STEP_VALUES[i] > MAX_ITEMS_STEP_VALUES[i - 1],
            )
        }
    }

    // ── theme axis (§2) ───────────────────────────────────────────────────────
    // The density pref/enum was removed (CopyPaste-xruv, §2/§12): there is exactly
    // one fixed §5 spacing scale, no density modes. The theme axis is dark/light
    // only — no "system" — and defaults to dark.

    @Test
    fun `ThemeMode is dark-light only with no system case`() {
        val values = ThemeMode.entries.map { it.name }
        assertEquals(
            "ThemeMode must be exactly LIGHT + DARK (no SYSTEM) — STYLEGUIDE §2",
            listOf("LIGHT", "DARK"),
            values,
        )
    }

    @Test
    fun `ThemeMode default is DARK`() {
        assertEquals(
            "Default theme must be DARK (dark-first, matches web store.ts) — §2",
            ThemeMode.DARK,
            ThemeMode.DEFAULT,
        )
    }
}
