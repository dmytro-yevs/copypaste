package com.copypaste.android

import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * CopyPaste-vp63.37 — unit tests for the pure item-pipeline derivations
 * extracted from HistoryScreen into HistoryScreenState.kt.
 */
class HistoryScreenStateTest {

    private fun item(
        id: String,
        wallTimeMs: Long,
        pinned: Boolean = false,
        pinnedSortIndex: Int = -1,
        originDeviceId: String? = null,
        snippet: String = "",
    ) = ClipboardItem(
        id = id,
        contentType = "text",
        isSensitive = false,
        wallTimeMs = wallTimeMs,
        snippet = snippet,
        pinned = pinned,
        pinnedSortIndex = pinnedSortIndex,
        originDeviceId = originDeviceId,
    )

    private fun peer(fingerprint: String, name: String, peerDeviceId: String? = null) = PairedPeer(
        fingerprint = fingerprint,
        syncAddr = "10.0.0.1:9000",
        name = name,
        sessionKeyWrappedB64 = "",
        sessionKeyIvB64 = "",
        peerDeviceId = peerDeviceId,
    )

    // ── sortHistoryItems ──────────────────────────────────────────────────────

    @Test
    fun `sortHistoryItems dedupes duplicate ids before sorting`() {
        val items = listOf(
            item("a", wallTimeMs = 100),
            item("a", wallTimeMs = 200), // duplicate id — must not crash the LazyColumn key
            item("b", wallTimeMs = 50),
        )
        val result = sortHistoryItems(items, sortByDevice = false, ownDeviceId = "own", pairedPeers = emptyList())
        assertEquals(listOf("a", "b"), result.map { it.id })
    }

    @Test
    fun `sortHistoryItems puts pinned items first in pinnedSortIndex order, then unpinned by recency`() {
        val items = listOf(
            item("unpinned-old", wallTimeMs = 100),
            item("pinned-second", wallTimeMs = 300, pinned = true, pinnedSortIndex = 1),
            item("unpinned-new", wallTimeMs = 200),
            item("pinned-first", wallTimeMs = 50, pinned = true, pinnedSortIndex = 0),
        )
        val result = sortHistoryItems(items, sortByDevice = false, ownDeviceId = "own", pairedPeers = emptyList())
        assertEquals(
            listOf("pinned-first", "pinned-second", "unpinned-new", "unpinned-old"),
            result.map { it.id },
        )
    }

    @Test
    fun `sortHistoryItems with sortByDevice groups own device first then peers alphabetically, pinned unaffected`() {
        val items = listOf(
            item("peerB-item", wallTimeMs = 400, originDeviceId = "dev-b"),
            item("own-item-old", wallTimeMs = 100, originDeviceId = "own"),
            item("peerA-item", wallTimeMs = 300, originDeviceId = "dev-a"),
            item("own-item-new", wallTimeMs = 200, originDeviceId = "own"),
            item("pinned-item", wallTimeMs = 1, pinned = true, pinnedSortIndex = 0, originDeviceId = "dev-a"),
        )
        val peers = listOf(
            peer("dev-a", name = "Zed's Phone"),
            peer("dev-b", name = "Amy's Laptop"),
        )
        val result = sortHistoryItems(items, sortByDevice = true, ownDeviceId = "own", pairedPeers = peers)
        // Pinned first regardless of device, then own device (newest first),
        // then peers alphabetically by display name ("Amy's Laptop" < "Zed's Phone").
        assertEquals(
            listOf("pinned-item", "own-item-new", "own-item-old", "peerB-item", "peerA-item"),
            result.map { it.id },
        )
    }

    // ── filterHistoryItemsBySearch ────────────────────────────────────────────

    @Test
    fun `filterHistoryItemsBySearch returns all items when query is blank`() {
        val items = listOf(item("a", 1, snippet = "hello"), item("b", 2, snippet = "world"))
        val result = filterHistoryItemsBySearch(items, query = "   ", fullMatchIds = emptySet(), fullMatchQuery = "")
        assertEquals(items, result)
    }

    @Test
    fun `filterHistoryItemsBySearch matches snippet case-insensitively`() {
        val items = listOf(item("a", 1, snippet = "Hello World"), item("b", 2, snippet = "goodbye"))
        val result = filterHistoryItemsBySearch(items, query = "hello", fullMatchIds = emptySet(), fullMatchQuery = "")
        assertEquals(listOf("a"), result.map { it.id })
    }

    @Test
    fun `filterHistoryItemsBySearch unions in full-content matches for the current query`() {
        val items = listOf(item("a", 1, snippet = "short preview…"), item("b", 2, snippet = "unrelated"))
        val result = filterHistoryItemsBySearch(
            items,
            query = "deep match",
            fullMatchIds = setOf("a"),
            fullMatchQuery = "deep match",
        )
        assertEquals(listOf("a"), result.map { it.id })
    }

    @Test
    fun `filterHistoryItemsBySearch ignores stale fullMatchIds computed for a different query`() {
        val items = listOf(item("a", 1, snippet = "unrelated"), item("b", 2, snippet = "unrelated"))
        // fullMatchIds was computed for a PREVIOUS query — must not leak into this result.
        val result = filterHistoryItemsBySearch(
            items,
            query = "new query",
            fullMatchIds = setOf("a"),
            fullMatchQuery = "old query",
        )
        assertEquals(emptyList<String>(), result.map { it.id })
    }
}
