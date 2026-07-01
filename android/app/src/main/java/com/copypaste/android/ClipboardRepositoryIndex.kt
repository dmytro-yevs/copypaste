package com.copypaste.android

/**
 * Id-index and source-id set helpers for [ClipboardRepository].
 *
 * Extracted from [ClipboardRepository] (CopyPaste-vp63.33). These extension functions
 * access [ClipboardRepository]'s internal fields via the extension receiver. Mirrors the
 * existing extraction pattern from [ClipboardRepositoryPin], [ClipboardRepositoryPrune],
 * and [ClipboardRepositorySync] (CopyPaste-ra15.4).
 */

/**
 * The ordered id index, read back canonical: blanks removed and any
 * duplicate ids collapsed to their FIRST (oldest) occurrence. Persisting a
 * dup-free index is the invariant every writer relies on; reading it
 * de-duplicated also heals any index that an older build may have corrupted,
 * so the history LazyColumn never sees a repeated key.
 */
internal fun ClipboardRepository.storedIds(): List<String> {
    // Fast path: in-memory snapshot populated by every writer.
    ClipboardItemCache.cachedIds?.let { return it }
    // Cold start: parse from SharedPreferences and cache.
    val ids = prefs.getString(ClipboardRepository.KEY_ITEM_IDS, "")
        ?.split(",")
        ?.filter { it.isNotBlank() }
        ?.distinct()
        ?: emptyList()
    ClipboardItemCache.cachedIds = ids
    return ids
}

/**
 * Append [id] to [current], guaranteeing it appears exactly once and at the
 * end (most-recent position). Any prior occurrence is removed first so the
 * index can never hold the same id twice — the root invariant that keeps the
 * history LazyColumn's `key = { it.id }` from crashing on a duplicate.
 */
internal fun ClipboardRepository.appendUniqueId(current: List<String>, id: String): List<String> {
    val next = current.toMutableList()
    next.remove(id)
    next.add(id)
    return next
}

/** Ordered list of pinned ids — position 0 is displayed at the top of the pinned section. */
internal fun ClipboardRepository.storedPinnedList(): List<String> =
    prefs.getString(ClipboardRepository.KEY_PINNED_IDS, "")
        ?.split(",")
        ?.filter { it.isNotBlank() }
        ?: emptyList()

internal fun ClipboardRepository.storedPinnedIds(): Set<String> = storedPinnedList().toHashSet()

internal fun ClipboardRepository.storedSourceIds(): LinkedHashSet<String> =
    LinkedHashSet(
        prefs.getString(ClipboardRepository.KEY_SYNCED_SOURCE_IDS, "")
            ?.split(",")
            ?.filter { it.isNotBlank() }
            ?: emptyList()
    )

internal fun ClipboardRepository.recordSourceId(sourceId: String, seen: LinkedHashSet<String>) {
    seen.add(sourceId)
    while (seen.size > ClipboardRepository.MAX_SEEN_SOURCE_IDS) {
        val oldest = seen.iterator().next()
        seen.remove(oldest)
    }
    prefs.edit().putString(ClipboardRepository.KEY_SYNCED_SOURCE_IDS, seen.joinToString(",")).apply()
}
