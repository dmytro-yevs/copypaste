package com.copypaste.android

/**
 * Process-wide in-memory cache for parsed (decrypted) clipboard items.
 *
 * getItems() previously called parseItem()/decryptForPreview() for EVERY id
 * on EVERY reload — a full AEAD decrypt + native isSensitive() per row.
 * On a 200-item list this saturates Dispatchers.IO and produces Davey frames.
 *
 * We cache the parsed ClipboardItem keyed by storage id, invalidated only
 * when the raw blob string changes (i.e. the item was actually written).
 * getItems() reads the cheap prefs.getString("item_$id") and only decrypts
 * when the raw blob differs from the cached entry; otherwise reuses the
 * cached item. On a quiescent list (no new items since last load) this
 * reduces decryptions from N→0.
 *
 * The cache stores (rawBlob, ClipboardItem) without imagePng (that field
 * is removed). getItems() always applies cheap pinned/pinnedSortIndex/
 * tooLargeToSync overrides via .copy() after the cache lookup.
 *
 * Process-wide so multiple ClipboardRepository instances
 * (VM + searchRepository + filePickLauncher) share the same warm cache.
 *
 * Extracted from ClipboardRepository companion object (CopyPaste-g06m.20).
 */
object ClipboardItemCache {

    internal data class ParsedEntry(val rawBlob: String, val item: ClipboardItem)

    /**
     * In-memory snapshot of KEY_ITEM_IDS. Populated on first read and kept
     * in sync by every writer that holds idsWriteLock.  Avoids re-parsing
     * the SharedPreferences XML string on every 30-second P2P tick.
     *
     * @Volatile makes reads safe without a lock (single-writer pattern: all
     * writes happen inside a synchronized(idsWriteLock) block; a stale read
     * merely causes a harmless cache-miss that falls back to prefs).
     */
    @Volatile
    var cachedIds: List<String>? = null

    /** Guards [parseCache] for concurrent IO reads. */
    internal val parseCacheLock = Any()

    /**
     * Maps storage id → (rawBlob, ClipboardItem).
     */
    internal val parseCache = HashMap<String, ParsedEntry>()

    /**
     * Evict a single id from the parse cache.
     * Call on delete / LWW-replace paths alongside evictImageCaches.
     */
    fun evictParseCache(id: String) {
        synchronized(parseCacheLock) { parseCache.remove(id) }
    }

    /** Evict ALL entries — call on clearAll / clearUnpinned. */
    fun evictAllParseCache() {
        synchronized(parseCacheLock) { parseCache.clear() }
    }
}
