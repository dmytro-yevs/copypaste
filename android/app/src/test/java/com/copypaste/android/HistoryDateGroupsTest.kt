package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Test
import java.util.TimeZone

/**
 * CopyPaste-myh8.5 (S5 5.2) — unit tests for the date-group header pure logic:
 * [dateGroupFor] and [buildHistoryListEntries]. Uses a fixed UTC [TimeZone] so
 * the test is independent of the machine's local timezone.
 */
class HistoryDateGroupsTest {

    private val utc = TimeZone.getTimeZone("UTC")

    /** 2026-07-02 12:00:00 UTC — an arbitrary fixed "now" for every test below. */
    private val nowMs = 1783166400000L

    private val oneDayMs = 86_400_000L

    private fun item(id: String, wallTimeMs: Long, pinned: Boolean = false) = ClipboardItem(
        id = id,
        contentType = "text",
        isSensitive = false,
        wallTimeMs = wallTimeMs,
        pinned = pinned,
    )

    // ── dateGroupFor ──────────────────────────────────────────────────────

    @Test
    fun `pinned rows always group as PINNED regardless of their timestamp`() {
        assertEquals(HistoryDateGroup.PINNED, dateGroupFor(pinned = true, wallTimeMs = 0L, nowMs = nowMs, zone = utc))
        assertEquals(HistoryDateGroup.PINNED, dateGroupFor(pinned = true, wallTimeMs = nowMs, nowMs = nowMs, zone = utc))
    }

    @Test
    fun `a timestamp from today groups as TODAY`() {
        assertEquals(HistoryDateGroup.TODAY, dateGroupFor(pinned = false, wallTimeMs = nowMs, nowMs = nowMs, zone = utc))
        // Still today, just earlier in the day (00:00:01 UTC on the same calendar day).
        val earlyToday = nowMs - (nowMs % oneDayMs) + 1_000L
        assertEquals(HistoryDateGroup.TODAY, dateGroupFor(pinned = false, wallTimeMs = earlyToday, nowMs = nowMs, zone = utc))
    }

    @Test
    fun `a timestamp from yesterday groups as YESTERDAY`() {
        val yesterday = nowMs - oneDayMs
        assertEquals(HistoryDateGroup.YESTERDAY, dateGroupFor(pinned = false, wallTimeMs = yesterday, nowMs = nowMs, zone = utc))
    }

    @Test
    fun `a timestamp from two or more days ago groups as EARLIER`() {
        val twoDaysAgo = nowMs - (2 * oneDayMs)
        assertEquals(HistoryDateGroup.EARLIER, dateGroupFor(pinned = false, wallTimeMs = twoDaysAgo, nowMs = nowMs, zone = utc))
        assertEquals(HistoryDateGroup.EARLIER, dateGroupFor(pinned = false, wallTimeMs = 0L, nowMs = nowMs, zone = utc))
    }

    // ── buildHistoryListEntries ──────────────────────────────────────────

    @Test
    fun `emits exactly one header per contiguous date-group run, in list order`() {
        val items = listOf(
            item("pinned-1", wallTimeMs = 0L, pinned = true),
            item("today-1", wallTimeMs = nowMs),
            item("today-2", wallTimeMs = nowMs - 1_000L),
            item("yesterday-1", wallTimeMs = nowMs - oneDayMs),
            item("earlier-1", wallTimeMs = nowMs - (3 * oneDayMs)),
        )
        // dateGroupFor is exercised with the default (system) zone here to match
        // production usage of `buildHistoryListEntries`; the boundary math itself
        // is already pinned to UTC above, so this only asserts fold STRUCTURE.
        val entries = buildHistoryListEntries(items, nowMs)

        val groupSequence = entries.filterIsInstance<HistoryListEntry.Header>().map { it.group }
        assertEquals(listOf(HistoryDateGroup.PINNED, HistoryDateGroup.TODAY, HistoryDateGroup.YESTERDAY, HistoryDateGroup.EARLIER), groupSequence)

        val rowIds = entries.filterIsInstance<HistoryListEntry.Row>().map { it.item.id }
        assertEquals(items.map { it.id }, rowIds)
    }

    @Test
    fun `an empty item list produces no entries at all`() {
        assertEquals(emptyList<HistoryListEntry>(), buildHistoryListEntries(emptyList(), nowMs))
    }

    @Test
    fun `consecutive rows in the same group share a single header`() {
        val items = listOf(
            item("today-1", wallTimeMs = nowMs),
            item("today-2", wallTimeMs = nowMs - 500L),
            item("today-3", wallTimeMs = nowMs - 1_000L),
        )
        val entries = buildHistoryListEntries(items, nowMs)
        val headerCount = entries.count { it is HistoryListEntry.Header }
        assertEquals(1, headerCount)
        assertEquals(4, entries.size) // 1 header + 3 rows
    }
}
