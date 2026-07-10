package com.copypaste.android

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

/**
 * CopyPaste-myh8.9 wave 4 (§I "Non-tab user-controlled state"): `capture_enabled`
 * (Immediate — notification pause/resume action), `sort_by_device` (Immediate —
 * overflow menu), `recent_searches` (Immediate — history search bar; 5-entry
 * cap, NUL-delimiter round-trip, blank filtering; logic at
 * Settings.kt ~803-814).
 *
 * `capture_enabled`'s consumer is [ClipboardCapturePipeline]'s early-return
 * gate (`if (!settings.captureEnabled) return`), reached BEFORE any
 * keystore/native access — same reachability class as the private_mode gate
 * noted in [GeneralTabSettingsTest], but exercising the full pipeline still
 * requires a live [SyncManager] + [ClipboardRepository], so persistence-only
 * here (matches [RepairedSettingsConsumersTest]'s rationale for not calling
 * `captureClip` directly). `sort_by_device`'s consumer
 * (`HistoryScreenState` sort) is a Compose recomposition read — deferred to
 * goldens/connected.
 */
@RunWith(RobolectricTestRunner::class)
@Config(sdk = [34])
class NonTabSettingsStateTest {

    private fun context(): Context = ApplicationProvider.getApplicationContext()

    private fun rawPrefs() =
        context().getSharedPreferences("copypaste", Context.MODE_PRIVATE)

    // ── capture_enabled (Immediate) ──────────────────────────────────────────

    @Test
    fun `capture_enabled defaults to true`() {
        assertTrue(Settings(context()).captureEnabled)
    }

    @Test
    fun `capture_enabled is readable immediately after a bare setter write — the notification Pause action pattern`() {
        val ctx = context()
        Settings(ctx).captureEnabled = false
        assertFalse("Immediate mode: a fresh instance must see the write without a Save", Settings(ctx).captureEnabled)
    }

    @Test
    fun `capture_enabled upgrade fixture — pre-redesign raw key survives`() {
        val ctx = context()
        rawPrefs().edit().putBoolean("capture_enabled", false).apply()
        assertFalse(Settings(ctx).captureEnabled)
    }

    // ── sort_by_device (Immediate) ───────────────────────────────────────────

    @Test
    fun `sort_by_device defaults to false`() {
        assertFalse(Settings(context()).sortByDevice)
    }

    @Test
    fun `sort_by_device is readable immediately after a bare setter write — the overflow-menu toggle pattern`() {
        val ctx = context()
        Settings(ctx).sortByDevice = true
        assertTrue("Immediate mode: a fresh instance must see the write without a Save", Settings(ctx).sortByDevice)
    }

    @Test
    fun `sort_by_device upgrade fixture — pre-redesign raw key survives`() {
        val ctx = context()
        rawPrefs().edit().putBoolean("sort_by_device", true).apply()
        assertTrue(Settings(ctx).sortByDevice)
    }

    // ── recent_searches ───────────────────────────────────────────────────────

    @Test
    fun `recent_searches defaults to an empty list`() {
        assertEquals(emptyList<String>(), Settings(context()).recentSearches)
    }

    @Test
    fun `recent_searches round-trips through the NUL-delimiter encoding — commas and pipes preserved`() {
        val ctx = context()
        val queries = listOf("hello, world", "a|b|c", "plain query")
        Settings(ctx).recentSearches = queries

        assertEquals(queries, Settings(ctx).recentSearches)
    }

    @Test
    fun `recent_searches caps at 5 entries on write`() {
        val ctx = context()
        val eightQueries = (1..8).map { "query-$it" }
        Settings(ctx).recentSearches = eightQueries

        assertEquals(eightQueries.take(5), Settings(ctx).recentSearches)
    }

    @Test
    fun `recent_searches caps at 5 entries on read even if a legacy write stored more`() {
        val ctx = context()
        // Simulate a raw prefs value with more than 5 entries (e.g. a future/legacy
        // writer that did not respect the cap) — the getter must still cap on read.
        val nul = 0.toChar().toString()
        val raw = (1..9).map { "legacy-$it" }.joinToString(nul)
        rawPrefs().edit().putString("recent_searches", raw).apply()

        assertEquals((1..5).map { "legacy-$it" }, Settings(ctx).recentSearches)
    }

    @Test
    fun `recent_searches filters out blank entries on write and on read`() {
        val ctx = context()
        Settings(ctx).recentSearches = listOf("real query", "", "   ", "another real one")

        assertEquals(listOf("real query", "another real one"), Settings(ctx).recentSearches)
    }

    @Test
    fun `recent_searches filters blanks from a raw legacy value containing empty NUL-joined segments`() {
        val ctx = context()
        val nul = 0.toChar().toString()
        rawPrefs().edit().putString("recent_searches", "first$nul${nul}second$nul   ").apply()

        assertEquals(listOf("first", "second"), Settings(ctx).recentSearches)
    }

    @Test
    fun `recent_searches empty list round-trips to an empty list`() {
        val ctx = context()
        Settings(ctx).recentSearches = listOf("something")
        Settings(ctx).recentSearches = emptyList()

        assertEquals(emptyList<String>(), Settings(ctx).recentSearches)
    }
}
