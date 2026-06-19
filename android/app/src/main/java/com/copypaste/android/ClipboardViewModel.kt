package com.copypaste.android

import android.app.Application
import android.content.SharedPreferences
import android.util.Log
import androidx.lifecycle.AndroidViewModel
import androidx.lifecycle.LiveData
import androidx.lifecycle.MutableLiveData
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/**
 * ViewModel for clipboard history UI.
 * Extends [AndroidViewModel] to obtain [Application] context required by
 * [ClipboardRepository] for SharedPreferences access.
 *
 * ## Lazy pagination
 *
 * Items are loaded in pages of [ClipboardRepository.PAGE_SIZE] unpinned rows.
 * Pinned items always appear on every page (at the top). The UI calls [loadMore]
 * when the user scrolls near the end; [loadItems] resets to page 0.
 *
 * [hasMore] is false once we have loaded at least as many unpinned rows as exist
 * in the store — the UI hides the "load more" trigger at that point.
 */
class ClipboardViewModel(app: Application) : AndroidViewModel(app) {

    private val repository = ClipboardRepository(app)
    private val settings = Settings(app)

    private val _items = MutableLiveData<List<ClipboardItem>>(emptyList())
    val items: LiveData<List<ClipboardItem>> = _items

    private val _loading = MutableLiveData(false)
    val loading: LiveData<Boolean> = _loading

    private val _loadingMore = MutableLiveData(false)
    val loadingMore: LiveData<Boolean> = _loadingMore

    /** True when there are more unpinned pages to load. Starts false to avoid
     *  a spinner flash before the first loadItems() call completes. */
    private val _hasMore = MutableLiveData(false)
    val hasMore: LiveData<Boolean> = _hasMore

    private val _errors = MutableLiveData<String?>(null)
    val errors: LiveData<String?> = _errors

    /** Current pagination offset (count of unpinned rows already loaded). */
    private var unpinnedOffset = 0

    /**
     * Total number of stored items (pinned + unpinned) in the repository.
     * Updated after each [loadItems] / [loadMore] call.
     * Used by the history header to show the real total rather than the
     * number of items loaded so far.
     */
    private val _totalCount = MutableLiveData(0)
    val totalCount: LiveData<Int> = _totalCount

    /**
     * Debounce job for the store-change listener. Rapid bursts of prefs writes
     * (e.g. a sync catch-up) are collapsed into a single reload call.
     */
    private var storeDebounceJob: Job? = null

    /**
     * Track which item IDs are currently visible in the list. Used by the
     * store-change listener to distinguish additive updates (new items arrived
     * while the user is scrolled down) from structural changes (deletes, pin
     * toggles, reorders) that require a full reset.
     */
    private var knownItemIds: Set<String> = emptySet()

    /**
     * Auto-refresh whenever the backing store changes.
     * Watches [ClipboardRepository.KEY_ITEM_IDS] / [KEY_PINNED_IDS].
     * Retained as a field — SharedPreferences holds a weak reference.
     *
     * Incremental strategy:
     * - KEY_ITEM_IDS changed → check whether items were only ADDED (sync
     *   catch-up) or also removed/reordered.  Additive changes are merged at
     *   the top of the existing list without resetting [unpinnedOffset] so the
     *   user's scroll position is preserved.  Any removal or structural change
     *   falls back to a full [loadItems] reset.
     * - KEY_PINNED_IDS changed → always full reload (pin order is structural).
     */
    private val storeListener =
        SharedPreferences.OnSharedPreferenceChangeListener { _, key ->
            when (key) {
                ClipboardRepository.KEY_ITEM_IDS -> {
                    storeDebounceJob?.cancel()
                    storeDebounceJob = viewModelScope.launch {
                        delay(STORE_DEBOUNCE_MS)
                        refreshItems()
                    }
                }
                ClipboardRepository.KEY_PINNED_IDS -> {
                    storeDebounceJob?.cancel()
                    storeDebounceJob = viewModelScope.launch {
                        delay(STORE_DEBOUNCE_MS)
                        loadItems()
                    }
                }
            }
        }

    init {
        repository.observe(storeListener)
    }

    /**
     * Incremental refresh: fetch the first page and merge any NEW items at the
     * top of the existing list without resetting [unpinnedOffset].
     *
     * Called when KEY_ITEM_IDS fires (item added or removed).  If the refresh
     * detects that existing IDs have been removed or pin state has changed
     * structurally, it falls through to a full [loadItems] reset so the list
     * stays consistent.
     *
     * Preserves the user's scroll position when the only change was new items
     * arriving at the top (the common sync/capture case).
     */
    private fun refreshItems() {
        viewModelScope.launch {
            try {
                // Fetch the first page — this always contains pinned items plus
                // the newest unpinned items (offset 0).
                val freshPage = repository.getItems(
                    key    = settings.encryptionKey,
                    limit  = ClipboardRepository.PAGE_SIZE,
                    offset = 0,
                )
                val freshIds = freshPage.mapTo(HashSet()) { it.id }
                val existing = _items.value ?: emptyList()
                val existingIds = existing.mapTo(HashSet()) { it.id }

                // Determine whether this is purely additive (new items at top)
                // or structural (items removed / reordered).
                val anyRemoved = existingIds.any { it !in freshIds } &&
                    existing.any { !it.pinned && it.id !in freshIds }

                if (anyRemoved || existing.isEmpty()) {
                    // Structural change — full reset is required for correctness.
                    loadItems()
                    return@launch
                }

                // Additive: some IDs in freshPage are not yet in existing.
                // Prepend them to the current list without touching unpinnedOffset
                // (the offset still correctly addresses the same "next page" the
                // user would load by scrolling — existing loaded rows are intact).
                val newItems = freshPage.filter { it.id !in existingIds }
                if (newItems.isEmpty()) {
                    // No visible change (e.g. a tombstone write or metadata update).
                    // Refresh counts but keep the list intact.
                    _totalCount.value = repository.totalItemCount()
                    _hasMore.value = repository.unpinnedItemCount() > unpinnedOffset
                    return@launch
                }

                // Merge: new unpinned items go to the top of the unpinned section;
                // fresh pinned items replace stale pinned entries (pin state may have
                // changed for an already-loaded item).
                val freshPinned = freshPage.filter { it.pinned }
                val existingUnpinned = existing.filter { !it.pinned }
                val newUnpinned = newItems.filter { !it.pinned }
                val merged = (freshPinned + newUnpinned + existingUnpinned).distinctBy { it.id }

                if (merged != existing) {
                    _items.value = merged
                }
                // Offset grows by the number of new unpinned items prepended.
                unpinnedOffset += newUnpinned.size
                knownItemIds = merged.mapTo(HashSet()) { it.id }
                _totalCount.value = repository.totalItemCount()
                _hasMore.value = repository.unpinnedItemCount() > unpinnedOffset
            } catch (e: Exception) {
                Log.w(TAG, "refreshItems failed — falling back to full reload", e)
                loadItems()
            }
        }
    }

    /**
     * Load (or reload) from page 0. Resets pagination state and replaces the
     * current item list. Called on initial open and after any mutation.
     */
    fun loadItems() {
        viewModelScope.launch {
            _loading.value = true
            unpinnedOffset = 0
            try {
                val page = repository.getItems(
                    key    = settings.encryptionKey,
                    limit  = ClipboardRepository.PAGE_SIZE,
                    offset = 0,
                )
                val next = page.distinctBy { it.id }
                if (next != _items.value) {
                    _items.value = next
                }
                unpinnedOffset = next.count { !it.pinned }
                knownItemIds = next.mapTo(HashSet()) { it.id }
                _totalCount.value = repository.totalItemCount()
                // Check whether there are more unpinned rows beyond this page.
                _hasMore.value = repository.unpinnedItemCount() > unpinnedOffset
            } catch (e: Exception) {
                Log.w(TAG, "loadItems failed", e)
                _errors.value = e.message ?: e.javaClass.simpleName
            } finally {
                _loading.value = false
            }
        }
    }

    /**
     * Append the next page of unpinned items. No-op when [hasMore] is false or a
     * load is already in flight. Pinned items already present are deduplicated so
     * they never appear twice in the combined list.
     */
    fun loadMore() {
        if (_loadingMore.value == true || _loading.value == true) return
        if (_hasMore.value == false) return
        viewModelScope.launch {
            _loadingMore.value = true
            try {
                val nextPage = repository.getItems(
                    key    = settings.encryptionKey,
                    limit  = ClipboardRepository.PAGE_SIZE,
                    offset = unpinnedOffset,
                )
                // Pinned items are returned on every page — deduplicate by id.
                val existing = _items.value ?: emptyList()
                val existingIds = existing.mapTo(HashSet()) { it.id }
                val newUnpinned = nextPage.filter { !it.pinned && it.id !in existingIds }
                // Also refresh pinned items in case pin state changed.
                val freshPinned = nextPage.filter { it.pinned }
                val existingUnpinned = existing.filter { !it.pinned }
                val merged = (freshPinned + existingUnpinned + newUnpinned).distinctBy { it.id }
                _items.value = merged
                unpinnedOffset = merged.count { !it.pinned }
                _totalCount.value = repository.totalItemCount()
                _hasMore.value = repository.unpinnedItemCount() > unpinnedOffset
            } catch (e: Exception) {
                Log.w(TAG, "loadMore failed", e)
                _errors.value = e.message ?: e.javaClass.simpleName
            } finally {
                _loadingMore.value = false
            }
        }
    }

    fun deleteItem(id: String) {
        viewModelScope.launch {
            try {
                repository.deleteItem(id)
                loadItems()
            } catch (e: Exception) {
                Log.w(TAG, "deleteItem($id) failed", e)
                _errors.value = e.message ?: e.javaClass.simpleName
            }
        }
    }

    fun deleteItems(ids: List<String>) {
        viewModelScope.launch {
            try {
                repository.deleteItems(ids)
                loadItems()
            } catch (e: Exception) {
                Log.w(TAG, "deleteItems(${ids.size}) failed", e)
                _errors.value = e.message ?: e.javaClass.simpleName
            }
        }
    }

    fun clearAll() {
        viewModelScope.launch {
            try {
                repository.clearAll()
                loadItems()
            } catch (e: Exception) {
                Log.w(TAG, "clearAll failed", e)
                _errors.value = e.message ?: e.javaClass.simpleName
            }
        }
    }

    fun clearUnpinned() {
        viewModelScope.launch {
            try {
                repository.clearUnpinned()
                loadItems()
            } catch (e: Exception) {
                Log.w(TAG, "clearUnpinned failed", e)
                _errors.value = e.message ?: e.javaClass.simpleName
            }
        }
    }

    fun setPinned(id: String, pinned: Boolean) {
        // setPinned now uses a blocking commit() — run off the main thread.
        viewModelScope.launch(Dispatchers.IO) {
            try {
                repository.setPinned(id, pinned)
                loadItems()
            } catch (e: Exception) {
                Log.w(TAG, "setPinned($id, $pinned) failed", e)
                // postValue: this coroutine runs on Dispatchers.IO, not the main thread.
                _errors.postValue(e.message ?: e.javaClass.simpleName)
            }
        }
    }

    /**
     * Persist a new user-defined order for pinned items.
     * [ids] is the full ordered list of pinned item IDs (first = top of pinned section).
     */
    fun reorderPinned(ids: List<String>) {
        // reorderPinned now uses a blocking commit() — run off the main thread.
        viewModelScope.launch(Dispatchers.IO) {
            try {
                repository.reorderPinned(ids)
                loadItems()
            } catch (e: Exception) {
                Log.w(TAG, "reorderPinned failed", e)
                // postValue: this coroutine runs on Dispatchers.IO, not the main thread.
                _errors.postValue(e.message ?: e.javaClass.simpleName)
            }
        }
    }

    /**
     * Move the just-copied item [id] to the top of the recency (non-pinned)
     * section, then refresh. Pinned items are left in place by
     * [ClipboardRepository.bumpToTop]. Mirrors macOS `bump_item_recency`.
     */
    /**
     * cvns (PG-18): hook set by [HistoryActivity] (or any other copy-back host) so
     * the ViewModel can enqueue an immediate sync push after bumping the lamport.
     * Receives the item id and new lamport timestamp.  No-op when null (tests,
     * screens that don't need sync).
     *
     * The hook runs on [Dispatchers.IO] inside [copyItem]'s coroutine, so
     * implementations may perform I/O (relay push, etc.) without blocking the main
     * thread.
     */
    var onCopyBackSync: (suspend (itemId: String, newLamport: Long) -> Unit)? = null

    fun copyItem(id: String) {
        viewModelScope.launch(Dispatchers.IO) {
            try {
                val newLamport = repository.bumpToTop(id)
                loadItems()
                // cvns: if the bump produced a new lamport and a sync hook is wired,
                // trigger an immediate push so peers see the re-copy with updated recency.
                if (newLamport > 0L) {
                    onCopyBackSync?.invoke(id, newLamport)
                }
            } catch (e: Exception) {
                Log.w(TAG, "copyItem($id) failed", e)
                // postValue: this coroutine runs on Dispatchers.IO, not the main thread.
                _errors.postValue(e.message ?: e.javaClass.simpleName)
            }
        }
    }

    /** Call from UI after the current error has been displayed to the user. */
    fun clearError() {
        _errors.value = null
    }

    override fun onCleared() {
        repository.stopObserving(storeListener)
        super.onCleared()
    }

    companion object {
        private const val TAG = "ClipboardViewModel"

        /**
         * Quiet period after the last prefs change before [loadItems] fires.
         * 300 ms absorbs rapid sync bursts while feeling instant for single edits.
         */
        private const val STORE_DEBOUNCE_MS = 300L
    }
}
